use bdl_tauri::{commands, secure_store::CredentialBackend, state::AppState};
use cutflow_workbench::{
    Error, Result, atomic_json, protocol::Call, required, server, tasks::Runtime,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

pub struct Hub {
    pub runtime: Runtime,
    server: Mutex<Option<server::Server>>,
    launch: Mutex<()>,
    downloads: Mutex<()>,
    quitting: std::sync::atomic::AtomicBool,
}

pub struct PortableCredentials(pub PathBuf);

impl CredentialBackend for PortableCredentials {
    fn load_cookie(&self) -> bdl_core::BdlResult<Option<String>> {
        cutflow_workbench::secret(&self.0, "bilibili-cookie", None).map_err(credential_error)
    }

    fn save_cookie(&self, cookie: &str) -> bdl_core::BdlResult<()> {
        cutflow_workbench::secret(&self.0, "bilibili-cookie", Some(cookie))
            .map(|_| ())
            .map_err(credential_error)
    }

    fn clear_cookie(&self) -> bdl_core::BdlResult<()> {
        let path = self.0.join("bilibili-cookie.dpapi");
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn credential_error(error: Error) -> bdl_core::BdlError {
    bdl_core::BdlError::Account {
        message: error.message,
    }
}

fn bdl_error(error: commands::CommandError) -> Error {
    Error::new(&error.code, error.message)
}

pub fn install(app: &mut tauri::App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = cutflow_workbench::portable_root()?;
    let tools = root.parent().unwrap().join("tools");
    let runtime = Runtime::open(root.clone(), tools.clone())?;
    let data = root.join("data");
    std::fs::create_dir_all(&data)?;
    let settings_path = data.join("settings.json");
    if !settings_path.exists() {
        atomic_json(
            &settings_path,
            &json!({"download_dir":root.join("downloads"),"auto_check_updates":false,"ffmpeg_path":tools.join("ffmpeg.exe")}),
        )?;
    }
    let credentials = bdl_tauri::secure_store::SecureStore::from_backend(PortableCredentials(
        root.join("credentials"),
    ));
    let state = AppState::new_with_secure_store(&data, root.join("downloads"), credentials)?;
    let mut settings = state.settings()?;
    settings.auto_check_updates = false;
    if tools.join("ffmpeg.exe").is_file() {
        settings.ffmpeg_path = Some(tools.join("ffmpeg.exe").to_string_lossy().into());
    }
    state.update_settings(settings)?;
    app.manage(state);
    app.manage(Hub {
        runtime,
        server: Mutex::new(None),
        launch: Mutex::new(()),
        downloads: Mutex::new(()),
        quitting: std::sync::atomic::AtomicBool::new(false),
    });
    let handle = app.handle().clone();
    let listener = server::Server::start(&root, Arc::new(move |call| dispatch(&handle, call)))?;
    *app.state::<Hub>().server.lock().unwrap() = Some(listener);
    app.asset_protocol_scope().allow_directory(&root, true)?;
    commands::start_queue_processing(app.handle());
    commands::start_account_startup_verification(app.handle());
    let show = tauri::menu::MenuItem::with_id(app, "show", "打开 CutFlow", true, None::<&str>)?;
    let exit =
        tauri::menu::MenuItem::with_id(app, "exit", "退出并停止本地任务", true, None::<&str>)?;
    let menu = tauri::menu::Menu::with_items(app, &[&show, &exit])?;
    let mut tray = tauri::tray::TrayIconBuilder::new()
        .tooltip("CutFlow · 素材 / AI 创作 / 剪辑")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "exit" => quit(app.clone()),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn active(app: &AppHandle) -> bool {
    let generating = app
        .state::<Hub>()
        .runtime
        .store
        .lock()
        .unwrap()
        .jobs
        .iter()
        .any(|job| matches!(job.status.as_str(), "queued" | "running"));
    let downloading = app
        .state::<AppState>()
        .queue_snapshot()
        .map(|tasks| {
            tasks.iter().any(|task| {
                matches!(
                    task.status,
                    bdl_core::queue::TaskStatus::Waiting
                        | bdl_core::queue::TaskStatus::Parsing
                        | bdl_core::queue::TaskStatus::Downloading
                        | bdl_core::queue::TaskStatus::Muxing
                )
            })
        })
        .unwrap_or(false);
    generating || downloading
}

pub fn quit(app: AppHandle) {
    if app.state::<Hub>().quitting.swap(true, std::sync::atomic::Ordering::SeqCst) { return; }
    std::thread::spawn(move || {
        let server = app.state::<Hub>().server.lock().unwrap().take();
        drop(server);
        if let Err(error) = app.state::<Hub>().runtime.persist() { eprintln!("CutFlow checkpoint: {error}"); }
        app.exit(0);
    });
}

fn editor_call(root: &Path, method: &str, params: Value) -> Result<Value> {
    server::call(
        &root.join("editor"),
        &Call {
            method: method.into(),
            params,
        },
    )
}

fn editor_status(root: &Path) -> Value {
    match editor_call(root, "cutflow.editor.status", json!({})) {
        Ok(status) => status,
        Err(error) => json!({"running":false,"error":error}),
    }
}

fn open_editor(hub: &Hub, params: Value) -> Result<Value> {
    let _launch = hub.launch.lock().unwrap();
    if editor_status(&hub.runtime.root)["running"] == true {
        return editor_call(&hub.runtime.root, "cutflow.editor.open", params);
    }
    let executable = hub
        .runtime
        .root
        .parent()
        .unwrap()
        .join("CutFlow-editor.exe");
    if !executable.is_file() {
        return Err(Error::new(
            "not_found",
            "剪辑程序尚未安装，请使用完整 CutFlow 安装包",
        ));
    }
    let mut command = std::process::Command::new(executable);
    if let Some(project) = params["projectPath"]
        .as_str()
        .filter(|path| !path.is_empty())
    {
        command.arg(project);
    }
    command.current_dir(hub.runtime.root.parent().unwrap());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn()?;
    for _attempt in 0..120 {
        if child.try_wait()?.is_some() {
            return Err(Error::new(
                "editor_error",
                "剪辑器启动失败，请检查 portable/logs",
            ));
        }
        let status = editor_status(&hub.runtime.root);
        if status["running"] == true {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(Error::new(
        "starting",
        "剪辑器仍在启动，请稍后刷新状态；无需重复打开",
    ))
}

fn bdl_dispatch(app: &AppHandle, method: &str, params: &Value) -> Result<Value> {
    let state = app.state::<AppState>();
    let converted = |error: bdl_core::BdlError| bdl_error(error.into());
    match method {
        "cutflow.hub.focus" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            Ok(json!({"running":true}))
        }
        "cutflow.account.start" => {
            let session = tauri::async_runtime::block_on(commands::account_login_qr_start())
                .map_err(bdl_error)?;
            let value = serde_json::to_value(session)?;
            Ok(json!({"sessionId":value["qrcode_key"],"svg":value["qr_image_svg"],"session":value}))
        }
        "cutflow.account.poll" => {
            let response = tauri::async_runtime::block_on(commands::account_login_qr_poll(
                app.clone(),
                state,
                commands::AccountLoginQrPollRequest {
                    qrcode_key: required(params, "sessionId")?.into(),
                },
            ))
            .map_err(bdl_error)?;
            Ok(serde_json::to_value(response)?)
        }
        "cutflow.account.status" => Ok(serde_json::to_value(state.account().map_err(converted)?)?),
        "cutflow.account.logout" => Ok(serde_json::to_value(
            commands::account_logout(app.clone(), state).map_err(bdl_error)?,
        )?),
        "cutflow.bilibili.favorites" => {
            let folder = params["folderId"].as_u64();
            if let Some(folder) = folder {
                let tree = tauri::async_runtime::block_on(state.parse_source(
                    &format!("https://www.bilibili.com/medialist/detail/ml{folder}"),
                    false,
                ))
                .map_err(converted)?;
                Ok(serde_json::to_value(tree)?)
            } else {
                let kind = bdl_core::account::AccountLibraryFolderKind::CreatedFavorite;
                let page = tauri::async_runtime::block_on(state.account_library(
                    kind,
                    params["page"].as_u64().unwrap_or(1) as u32,
                    20,
                ))
                .map_err(converted)?;
                Ok(serde_json::to_value(page)?)
            }
        }
        "cutflow.bilibili.parse" => {
            let tree =
                tauri::async_runtime::block_on(state.parse_source(required(params, "url")?, true))
                    .map_err(converted)?;
            Ok(serde_json::to_value(tree)?)
        }
        "cutflow.bilibili.download" => {
            let hub = app.state::<Hub>();
            let _serialize = hub.downloads.lock().unwrap();
            let operation = required(params, "operationId")?;
            if operation.len() > 128 {
                return Err(Error::new("invalid_request", "operationId 过长"));
            }
            required(params, "rights")?;
            let ledger_path = hub.runtime.root.join("bdl-operations.json");
            let mut ledger: Value = if ledger_path.exists() {
                serde_json::from_slice(&std::fs::read(&ledger_path)?)?
            } else {
                json!({})
            };
            if let Some(previous) = ledger.get(operation) {
                if previous["params"] != *params {
                    return Err(Error::new(
                        "operation_conflict",
                        "operationId 已用于其他请求",
                    ));
                }
                return previous.get("result").cloned().ok_or_else(|| {
                    Error::new("result_unknown", "上次下载创建中断，请核查 BDL 队列")
                });
            }
            let tree =
                tauri::async_runtime::block_on(state.parse_source(required(params, "url")?, true))
                    .map_err(converted)?;
            let all_parts = tree
                .groups
                .iter()
                .flat_map(|group| &group.items)
                .flat_map(|item| &item.parts)
                .collect::<Vec<_>>();
            let selection = params["parts"].as_str().unwrap_or("1");
            let mut indices = Vec::new();
            if selection == "all" {
                indices.extend(1..=all_parts.len());
            } else {
                for index in selection.split(',') {
                    indices.push(index.trim().parse::<usize>().map_err(|_| {
                        Error::new("invalid_request", "parts 请填写逗号分隔的分P序号或 all")
                    })?);
                }
            }
            let mut part_ids = Vec::new();
            for index in indices {
                part_ids.push(
                    all_parts
                        .get(index.wrapping_sub(1))
                        .ok_or_else(|| Error::new("invalid_request", "所选分P不存在"))?
                        .id
                        .0
                        .clone(),
                );
            }
            let output = params["projectPath"].as_str().map(|path| {
                Path::new(path)
                    .join("assets/bilibili")
                    .to_string_lossy()
                    .into_owned()
            });
            let request: commands::SelectionCreateTasksRequest = serde_json::from_value(
                json!({"source_id":tree.source.id.0,"part_ids":part_ids,
                "output_dir":output,"quality":params["quality"].as_str().unwrap_or("120"),"missing_quality_policy":if params["allowLower"]==true {"lower"} else {"skip"}}),
            )?;
            ledger[operation] = json!({"params":params});
            atomic_json(&ledger_path, &ledger)?;
            let (job, _fresh, cancel) = hub
                .runtime
                .reserve("cutflow.bilibili.download", params.clone())?;
            let result = match tauri::async_runtime::block_on(commands::selection_create_tasks(
                app.clone(),
                state,
                request,
            )) {
                Ok(result) => result,
                Err(error) => {
                    let error = bdl_error(error);
                    hub.runtime.finish(&job.id, Err(error.clone()));
                    return Err(error);
                }
            };
            let response = json!({"jobId":job.id,"status":"queued","queue":"bdl","tasks":result.created,"duplicates":result.duplicates});
            ledger[operation]["result"] = response.clone();
            atomic_json(&ledger_path, &ledger)?;
            let ids = result
                .created
                .iter()
                .map(|task| task.id.clone())
                .collect::<Vec<_>>();
            let runtime = hub.runtime.clone();
            let handle = app.clone();
            std::thread::spawn(move || {
                let outcome = (|| -> Result<Value> {
                    if ids.is_empty() {
                        return Err(Error::new(
                            "duplicate_download",
                            "未创建新下载。请在 B站下载队列确认已有任务，或处理画质与重复任务提示。",
                        ));
                    }
                    loop {
                        let state = handle.state::<AppState>();
                        let tasks = ids
                            .iter()
                            .map(|id| {
                                state
                                    .task_snapshot(id)
                                    .map_err(|error| bdl_error(error.into()))
                            })
                            .collect::<Result<Vec<_>>>()?;
                        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                            for id in &ids {
                                let _ = commands::queue_cancel(
                                    handle.clone(),
                                    handle.state::<AppState>(),
                                    id.clone(),
                                );
                            }
                            return Err(Error::new("cancelled", "下载已请求取消，已完成文件保留"));
                        }
                        if tasks.iter().any(|task| {
                            matches!(
                                task.status,
                                bdl_core::queue::TaskStatus::Failed
                                    | bdl_core::queue::TaskStatus::Cancelled
                            )
                        }) {
                            return Err(Error::new(
                                "download_error",
                                "下载失败或取消，请在 BDL 队列查看原因、刷新地址或恢复任务",
                            ));
                        }
                        if tasks
                            .iter()
                            .all(|task| task.status == bdl_core::queue::TaskStatus::Completed)
                        {
                            let assets = tasks
                                .iter()
                                .map(|task| runtime.probe(&task.output_path, &cancel))
                                .collect::<Result<Vec<_>>>()?;
                            return Ok(json!({"assets":assets,"tasks":tasks}));
                        }
                        drop(state);
                        std::thread::sleep(Duration::from_millis(500));
                    }
                })();
                runtime.finish(&job.id, outcome);
            });
            Ok(response)
        }
        _ => Err(Error::new("method_not_found", "未知 B站方法")),
    }
}

pub fn dispatch(app: &AppHandle, call: Call) -> Result<Value> {
    let hub = app.state::<Hub>();
    let root = &hub.runtime.root;
    let params = call.params;
    let method = call.method.as_str();
    if method.starts_with("cutflow.account.") || method.starts_with("cutflow.bilibili.") {
        return bdl_dispatch(app, method, &params);
    }
    match method {
        "cutflow.editor.status" => Ok(editor_status(root)),
        "cutflow.editor.open" | "cutflow.editor.focus" => open_editor(&hub, params),
        "cutflow.editor.recents" => {
            let path = root.join("recents.json");
            Ok(if path.exists() {
                serde_json::from_slice(&std::fs::read(path)?)?
            } else {
                json!([])
            })
        }
        "cutflow.hub.snapshot" => Ok(hub.runtime.snapshot()),
        "cutflow.files.open" => {
            let file = Path::new(required(&params, "path")?).canonicalize()?;
            app.opener()
                .open_path(file.to_string_lossy().as_ref(), None::<&str>)
                .map_err(|error| Error::new("io_error", error.to_string()))?;
            Ok(json!({"opened":true}))
        }
        "cutflow.assets.import"
        | "cutflow.project.current"
        | "cutflow.project.save"
        | "cutflow.edit.apply"
        | "cutflow.edit.undo"
        | "cutflow.edit.redo"
        | "cutflow.preview"
        | "cutflow.export.open"
        | "cutflow.speech.run"
        | "cutflow.captions.run"
        | "cutflow.workflow.run" => editor_call(root, method, params),
        "cutflow.jobs.list" => {
            let mut jobs = hub.runtime.store.lock().unwrap().jobs.clone();
            if let Ok(value) = editor_call(root, method, json!({})) {
                if let Ok(other) = serde_json::from_value::<Vec<cutflow_workbench::tasks::Job>>(
                    value["jobs"].clone(),
                ) {
                    jobs.extend(other);
                }
            }
            let downloads = app
                .state::<AppState>()
                .queue_snapshot()
                .map_err(|error| bdl_error(error.into()))?;
            Ok(json!({"jobs":jobs,"downloads":downloads}))
        }
        "cutflow.jobs.get" | "cutflow.jobs.cancel" => {
            if hub
                .runtime
                .store
                .lock()
                .unwrap()
                .jobs
                .iter()
                .any(|job| Some(job.id.as_str()) == params["jobId"].as_str())
            {
                hub.runtime.dispatch(method, params)
            } else {
                editor_call(root, method, params)
            }
        }
        _ => hub.runtime.dispatch(method, params),
    }
}

#[tauri::command]
pub async fn cutflow_call(app: AppHandle, method: String, params: Option<Value>) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        dispatch(
            &app,
            Call {
                method,
                params: params.unwrap_or_else(|| json!({})),
            },
        )
    })
    .await
    .map_err(|error| Error::new("internal_error", error.to_string()))?
}
