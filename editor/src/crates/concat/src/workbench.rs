use crate::{
    host::Shell,
    ui::{App, WorkbenchJobData},
};
use concat_project::Command;
use concat_project::commands::NewMedia;
use concat_project::model::{MediaKind, TextStyle, VideoSettings};
use cutflow_workbench::timeline::append_media;
use cutflow_workbench::{
    Error, Result,
    protocol::Call,
    tasks::{Job, Runtime},
    workflow::{Source, TaskSpec},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

thread_local! { static STATE: RefCell<Option<Rc<RefCell<State>>>> = const { RefCell::new(None) }; }

#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    request: Value,
    result: Value,
}

struct State {
    runtime: Runtime,
    server: Option<cutflow_workbench::server::Server>,
    ledger: BTreeMap<String, Receipt>,
    revision: u64,
    observed: Value,
    session: String,
    next_poll: std::time::Instant,
    messages: mpsc::Sender<(String, Result<Value>)>,
    replies: mpsc::Receiver<(String, Result<Value>)>,
    rows: Rc<VecModel<WorkbenchJobData>>,
    shown: String,
    busy: usize,
    ledger_fault: Option<String>,
}

fn failure(message: impl Into<String>) -> Error {
    Error::new("editor_error", message)
}

fn ui_call(call: Call) -> Result<Value> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    slint::invoke_from_event_loop(move || {
        if std::time::Instant::now() > deadline {
            return;
        }
        STATE.with(|slot| {
            let state = slot.borrow().clone();
            let result = state
                .ok_or_else(|| Error::new("not_running", "工作台尚未就绪"))
                .and_then(|state| state.borrow_mut().dispatch_ui(call));
            let _ = sender.send(result);
        });
    })
    .map_err(|_| Error::new("not_running", "窗口事件循环已关闭"))?;
    receiver
        .recv_timeout(Duration::from_secs(30))
        .map_err(|_| Error::new("result_unknown", "窗口未及时回应，请读取工程状态后核查操作"))?
}

fn route(runtime: &Runtime, call: Call) -> Result<Value> {
    if matches!(
        call.method.as_str(),
        "cutflow.generate" | "cutflow.bilibili.download"
    ) {
        if let Some(job) = runtime.existing(&call.method, &call.params)? {
            return Ok(json!({"jobId":job.id,"status":job.status}));
        }
        ui_call(Call {
            method: "cutflow.internal.check".into(),
            params: call.params.clone(),
        })?;
    }
    if matches!(
        call.method.as_str(),
        "cutflow.project.current"
            | "cutflow.project.save"
            | "cutflow.edit.apply"
            | "cutflow.edit.undo"
            | "cutflow.edit.redo"
            | "cutflow.assets.import"
            | "cutflow.preview"
            | "cutflow.export.open"
            | "cutflow.speech.run"
            | "cutflow.captions.run"
            | "cutflow.workflow.run"
    ) {
        ui_call(call)
    } else {
        runtime.dispatch(&call.method, call.params)
    }
}

impl State {
    fn current(&mut self) -> Result<Value> {
        let mut value = Err(Error::new("project_required", "请先新建或打开工程"));
        Shell::with(|shell, _app| {
            let mut studio = shell.studio.borrow_mut();
            studio.flush_commit();
            if let Some(session) = &studio.session {
                let document = session.document();
                let observed = json!({"path":session.path(),"document":document,"editRevision":studio.edit_revision()});
                if self.observed != observed {
                    self.revision += 1;
                    self.observed = observed;
                }
                value = Ok(
                    json!({"projectPath":session.path(),"revision":self.revision,"document":document,"view":session.view()}),
                );
            }
        });
        value
    }

    fn check(&mut self, params: &Value, revision: bool) -> Result<Value> {
        let current = self.current()?;
        let requested =
            Path::new(cutflow_workbench::required(params, "projectPath")?).canonicalize()?;
        let actual = Path::new(current["projectPath"].as_str().unwrap()).canonicalize()?;
        if requested != actual {
            return Err(Error::new(
                "project_changed",
                "当前工程已切换，请重新读取工程",
            ));
        }
        if revision && params["expectedRevision"].as_u64() != current["revision"].as_u64() {
            return Err(Error::new(
                "revision_conflict",
                "工程已修改，请读取最新 revision 后重试",
            ));
        }
        Ok(current)
    }

    fn submit(&mut self, app: &App, method: String, params: Value) {
        self.busy += 1;
        app.set_workbench_busy(true);
        let runtime = self.runtime.clone();
        let sender = self.messages.clone();
        std::thread::spawn(move || {
            let result = route(
                &runtime,
                Call {
                    method: method.clone(),
                    params,
                },
            );
            let _ = sender.send((method, result));
        });
    }

    fn dispatch_ui(&mut self, call: Call) -> Result<Value> {
        if call.method == "cutflow.project.current" {
            return self.current();
        }
        if call.method == "cutflow.internal.check" {
            return self.check(&call.params, false);
        }
        if call.method == "cutflow.internal.assemble" {
            self.check(&call.params, true)?;
            let commands = assemble(&call.params)?;
            let mut params = call.params;
            params["commands"] = serde_json::to_value(commands)?;
            return self.dispatch_ui(Call {
                method: "cutflow.internal.commit".into(),
                params,
            });
        }
        if call.method == "cutflow.preview" {
            self.check(&call.params, false)?;
            let time = call.params["time"].as_f64().unwrap_or(0.0);
            if !time.is_finite() || time < 0.0 {
                return Err(Error::new("invalid_request", "time 必须是非负秒数"));
            }
            Shell::with(|shell, app| {
                let mut studio = shell.studio.borrow_mut();
                studio.seek(time as f32);
                studio.publish(&app, &shell.models);
            });
            return self.current();
        }
        if call.method == "cutflow.export.open" {
            self.check(&call.params, false)?;
            Shell::with(|_shell, app| {
                app.set_workbench_opened(false);
                app.invoke_export_clicked();
            });
            return Ok(
                json!({"confirmationRequired":true,"message":"请在软件内检查导出参数并确认导出"}),
            );
        }
        if matches!(
            call.method.as_str(),
            "cutflow.workflow.run"
                | "cutflow.speech.run"
                | "cutflow.captions.run"
                | "cutflow.assets.import"
        ) {
            if let Some(job) = self.runtime.existing(&call.method, &call.params)? {
                return Ok(json!({"jobId":job.id,"status":job.status}));
            }
            self.check(&call.params, true)?;
            if call.method == "cutflow.workflow.run" {
                let spec: TaskSpec = serde_json::from_value(call.params.clone())?;
                spec.validate()?;
            }
            return self.long_task(call);
        }
        if let Some(fault) = &self.ledger_fault {
            return Err(Error::new("store_error", fault));
        }
        let operation = cutflow_workbench::required(&call.params, "operationId")?.to_string();
        let identity = json!({"method":call.method,"params":call.params});
        if let Some(receipt) = self.ledger.get(&operation) {
            if receipt.request != identity {
                return Err(Error::new(
                    "operation_conflict",
                    "operationId 已用于其他操作",
                ));
            }
            return if receipt.result["pending"] == true {
                Err(Error::new(
                    "result_unknown",
                    "上次操作中断，请检查工程后使用新 operationId",
                ))
            } else {
                Ok(receipt.result.clone())
            };
        }
        self.check(&call.params, true)?;
        self.ledger.insert(
            operation.clone(),
            Receipt {
                request: identity,
                result: json!({"pending":true}),
            },
        );
        self.save_ledger()?;
        let mut result = Err(failure("编辑窗口不可用"));
        Shell::with(|shell, app| {
            let mut studio = shell.studio.borrow_mut();
            result = (|| -> Result<Value> {
                match call.method.as_str() {
                    "cutflow.project.save" => {
                        studio
                            .session
                            .as_mut()
                            .ok_or_else(|| failure("工程未打开"))?
                            .save(None)
                            .map_err(failure)?;
                    }
                    "cutflow.edit.undo" => {
                        studio.undo();
                        if let Err(error) = studio.session.as_mut().unwrap().save(None) {
                            return Ok(json!({"applied":true,"saved":false,"warning":error}));
                        }
                    }
                    "cutflow.edit.redo" => {
                        studio.redo();
                        if let Err(error) = studio.session.as_mut().unwrap().save(None) {
                            return Ok(json!({"applied":true,"saved":false,"warning":error}));
                        }
                    }
                    "cutflow.edit.apply" | "cutflow.internal.commit" => {
                        let command = if call.params["commands"].is_array() {
                            Command::Batch {
                                commands: serde_json::from_value(call.params["commands"].clone())?,
                            }
                        } else {
                            serde_json::from_value::<Command>(call.params["command"].clone())?
                        };
                        let mut trial = studio
                            .session
                            .as_ref()
                            .ok_or_else(|| failure("工程未打开"))?
                            .sandbox();
                        let outcome = trial
                            .apply(command.clone())
                            .map_err(|error| failure(error.to_string()))?;
                        studio.apply(command);
                        studio.refresh_art();
                        if let Err(error) = studio.session.as_mut().unwrap().save(None) {
                            return Ok(
                                json!({"applied":outcome.applied,"createdId":outcome.created_id,"saved":false,"warning":error}),
                            );
                        }
                        return Ok(
                            json!({"applied":outcome.applied,"createdId":outcome.created_id,"saved":true}),
                        );
                    }
                    _ => return Err(Error::new("method_not_found", "未知编辑方法")),
                }
                Ok(json!({"applied":true,"saved":true}))
            })();
            studio.publish(&app, &shell.models);
        });
        match result {
            Ok(mut result) => {
                result["state"] = self.current()?;
                self.ledger.get_mut(&operation).unwrap().result = result.clone();
                self.save_ledger()?;
                Ok(result)
            }
            Err(error) => {
                self.ledger.remove(&operation);
                self.save_ledger()?;
                Err(error)
            }
        }
    }

    fn save_ledger(&self) -> Result<()> {
        cutflow_workbench::atomic_json(&self.runtime.root.join("operations.json"), &self.ledger)
    }

    fn long_task(&mut self, call: Call) -> Result<Value> {
        let (job, fresh, cancel) = self.runtime.reserve(&call.method, call.params)?;
        if fresh {
            let runtime = self.runtime.clone();
            let running = job.clone();
            let mut services = None;
            Shell::with(|shell, _app| {
                let studio = shell.studio.borrow();
                services = Some((studio.host.speech.clone(), studio.host.transcriber.clone()));
            });
            let (speech, transcriber) = services.ok_or_else(|| failure("媒体服务未就绪"))?;
            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare(&runtime, &running, &cancel, &speech, &transcriber)
                }))
                .unwrap_or_else(|_| Err(Error::new("interrupted", "任务异常中断，已完成素材保留")));
                runtime.finish(&running.id, result);
            });
        }
        Ok(json!({"jobId":job.id,"status":job.status}))
    }

    fn tick(&mut self, app: &App) {
        while let Ok((method, result)) = self.replies.try_recv() {
            self.busy = self.busy.saturating_sub(1);
            match result {
                Ok(value) => {
                    if method == "cutflow.account.start" {
                        self.session = value["sessionId"].as_str().unwrap_or("").into();
                        self.next_poll = std::time::Instant::now() + Duration::from_secs(3);
                        if let Some(svg) = value["svg"].as_str() {
                            app.set_workbench_qr(
                                slint::Image::load_from_svg_data(svg.as_bytes())
                                    .unwrap_or_default(),
                            );
                        }
                        app.set_workbench_message(
                            "请使用哔哩哔哩 App 扫码并确认；二维码三分钟后失效".into(),
                        );
                    } else if method == "cutflow.account.status"
                        || method == "cutflow.account.poll"
                        || method == "cutflow.account.logout"
                    {
                        let account = if value.get("account").is_some() {
                            &value["account"]
                        } else {
                            &value
                        };
                        app.set_workbench_account(
                            if account["loggedIn"] == true {
                                format!(
                                    "{} · 会员状态：{}",
                                    account["name"].as_str().unwrap_or("已登录"),
                                    account["membership"].as_str().unwrap_or("未知")
                                )
                            } else {
                                format!(
                                    "登录状态：{}",
                                    value["status"].as_str().unwrap_or("未登录或已过期")
                                )
                            }
                            .into(),
                        );
                        if value["status"] == "confirmed" {
                            self.session.clear();
                            app.set_workbench_qr(slint::Image::default());
                        }
                        if value["status"] == "expired" || method == "cutflow.account.logout" {
                            self.session.clear();
                            app.set_workbench_qr(slint::Image::default());
                        }
                        app.set_workbench_message("账号状态已更新；下载权限以视频解析为准".into());
                    } else if method == "cutflow.bilibili.favorites" {
                        let text = value["items"]
                            .as_array()
                            .map(|rows| {
                                rows.iter()
                                    .map(|row| {
                                        format!(
                                            "{} · ID {}\n{}",
                                            row["title"].as_str().unwrap_or("收藏夹"),
                                            row["media_id"].as_str().unwrap_or(""),
                                            row["source_url"].as_str().unwrap_or("")
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n\n")
                            })
                            .unwrap_or_default();
                        app.set_workbench_favorites(text.into());
                    } else {
                        app.set_workbench_message(
                            if let Some(id) = value["jobId"].as_str() {
                                format!("任务已提交：{id}")
                            } else {
                                "操作完成".into()
                            }
                            .into(),
                        );
                    }
                }
                Err(error) => app.set_workbench_message(error.to_string().into()),
            }
        }
        app.set_workbench_busy(self.busy > 0);
        if !self.session.is_empty() && self.busy == 0 && std::time::Instant::now() >= self.next_poll
        {
            self.next_poll = std::time::Instant::now() + Duration::from_secs(3);
            self.submit(
                app,
                "cutflow.account.poll".into(),
                json!({"sessionId":self.session}),
            );
        }
        let snapshot = self.runtime.snapshot();
        let model = cutflow_workbench::flow::describe(app.get_workbench_model().as_str(), "");
        app.set_workbench_capability(
            format!(
                "比例：{} · 时长：{} · 参考图上限：{}（由模型名推断，以服务实际支持为准）",
                model.aspect.as_deref().unwrap_or("服务默认"),
                model
                    .duration
                    .map(|duration| format!("{duration} 秒"))
                    .unwrap_or("服务默认".into()),
                model.max_references
            )
            .into(),
        );
        let text = format!(
            "{}:{}:{}:{}:{}",
            app.get_workbench_kind(),
            app.get_workbench_model_mode(),
            app.get_workbench_aspect(),
            app.get_workbench_duration(),
            snapshot
        );
        if text != self.shown {
            self.shown = text;
            let store = self.runtime.store.lock().unwrap();
            app.set_workbench_models(ModelRc::new(VecModel::from(
                store
                    .models
                    .iter()
                    .filter(|model| {
                        let kind = model.kind
                            == if app.get_workbench_kind() == 0 {
                                "image"
                            } else {
                                "video"
                            };
                        let mode = app.get_workbench_kind() == 0
                            || match app.get_workbench_model_mode() {
                                1 => model.id.contains("t2v"),
                                2 => !model.id.contains("t2v"),
                                _ => true,
                            };
                        let aspect = match app.get_workbench_aspect() {
                            1 => model.aspect.as_deref() == Some("16:9"),
                            2 => model.aspect.as_deref() == Some("9:16"),
                            3 => model.aspect.as_deref() == Some("1:1"),
                            _ => true,
                        };
                        let duration = app.get_workbench_kind() == 0
                            || match app.get_workbench_duration() {
                                1 => model.duration == Some(4.0),
                                2 => model.duration == Some(6.0),
                                3 => model.duration == Some(8.0),
                                4 => model.duration == Some(10.0),
                                _ => true,
                            };
                        kind && mode && aspect && duration
                    })
                    .map(|model| model.id.clone().into())
                    .collect::<Vec<_>>(),
            )));
            app.set_workbench_saved_references(ModelRc::new(VecModel::from(
                store
                    .references
                    .iter()
                    .map(|reference| format!("{} · {}", reference.kind, reference.name).into())
                    .collect::<Vec<_>>(),
            )));
            self.rows.set_vec(
                store
                    .jobs
                    .iter()
                    .rev()
                    .map(|job| {
                        let path = job
                            .result
                            .as_ref()
                            .and_then(|result| result["assets"][0]["path"].as_str())
                            .unwrap_or("");
                        let thumbnail = job
                            .result
                            .as_ref()
                            .and_then(|result| result["thumbnail"].as_str())
                            .and_then(|path| slint::Image::load_from_path(Path::new(path)).ok())
                            .unwrap_or_default();
                        WorkbenchJobData {
                            id: job.id.clone().into(),
                            title: job.method.clone().into(),
                            status: job.status.clone().into(),
                            detail: job.stage.clone().into(),
                            path: path.into(),
                            thumbnail,
                            completed: job.status == "completed" && !path.is_empty(),
                            running: job.status == "running",
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
}

pub struct Workbench {
    state: Rc<RefCell<State>>,
    timer: slint::Timer,
}

impl Workbench {
    pub fn smoke(&self) -> Result<()> {
        let runtime = self.state.borrow().runtime.clone();
        let root = runtime.root.join("smoke-checks").join(uuid());
        std::fs::create_dir_all(&root)?;
        let first =
            concat_host::projects::create(&root.to_string_lossy(), "control-a", 1920, 1080, 30, 1)
                .map_err(failure)?;
        let second =
            concat_host::projects::create(&root.to_string_lossy(), "control-b", 1920, 1080, 30, 1)
                .map_err(failure)?;
        let mut opened = Err(failure("窗口不可用"));
        Shell::with(|shell, _| {
            opened = shell
                .studio
                .borrow_mut()
                .open_project(first.clone())
                .map_err(failure);
        });
        opened.clone()?;
        let mut state = self.state.borrow_mut();
        let before = state.current()?;
        let command = json!({"op":"addTextClip","trackId":null,"above":true,"start":0,"duration":2,"style":{"content":"中文草稿测试"},"offsetY":0.3});
        let request = Call {
            method: "cutflow.edit.apply".into(),
            params: json!({"projectPath":first.path,"operationId":uuid(),"expectedRevision":before["revision"],"command":command}),
        };
        let once = state.dispatch_ui(request.clone())?;
        let twice = state.dispatch_ui(request.clone())?;
        if once != twice || once["state"]["revision"] == before["revision"] {
            return Err(failure("重复操作或修订号检查失败"));
        }
        let mut stale = request;
        stale.params["operationId"] = json!(uuid());
        if state
            .dispatch_ui(stale)
            .err()
            .is_none_or(|error| error.code != "revision_conflict")
        {
            return Err(failure("未拒绝过期修订号"));
        }
        let saved = state.current()?["document"].clone();
        let undo = Call {
            method: "cutflow.edit.undo".into(),
            params: json!({"projectPath":first.path,"operationId":uuid(),"expectedRevision":state.current()?["revision"]}),
        };
        state.dispatch_ui(undo)?;
        let redo = Call {
            method: "cutflow.edit.redo".into(),
            params: json!({"projectPath":first.path,"operationId":uuid(),"expectedRevision":state.current()?["revision"]}),
        };
        state.dispatch_ui(redo)?;
        if state.current()?["document"] != saved {
            return Err(failure("撤销重做后草稿不一致"));
        }
        Shell::with(|shell, _| {
            opened = shell
                .studio
                .borrow_mut()
                .open_project(second)
                .map_err(failure);
        });
        opened.clone()?;
        if state
            .check(&json!({"projectPath":first.path}), false)
            .err()
            .is_none_or(|error| error.code != "project_changed")
        {
            return Err(failure("工程切换检查失败"));
        }
        let reopen = concat_host::projects::open(&first.path).map_err(failure)?;
        Shell::with(|shell, _| {
            opened = shell
                .studio
                .borrow_mut()
                .open_project(reopen)
                .map_err(failure);
        });
        opened?;
        if state.current()?["document"] != saved {
            return Err(failure("保存重开后草稿不一致"));
        }
        drop(state);
        let dispatch_runtime = runtime.clone();
        let server = cutflow_workbench::server::Server::start(
            &runtime.root,
            Arc::new(move |call| route(&dispatch_runtime, call)),
        )?;
        let (completed, received) = mpsc::sync_channel(1);
        let control_root = runtime.root.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                let current = cutflow_workbench::server::call(
                    &control_root,
                    &Call {
                        method: "cutflow.project.current".into(),
                        params: json!({}),
                    },
                )?;
                let request = Call {
                    method: "cutflow.edit.apply".into(),
                    params: json!({"projectPath":current["projectPath"],"expectedRevision":current["revision"],"operationId":uuid(),"command":{"op":"addTextClip","trackId":null,"above":true,"start":3,"duration":1,"style":{"content":"回环控制通道"}}}),
                };
                let first = cutflow_workbench::server::call(&control_root, &request)?;
                let duplicate = cutflow_workbench::server::call(&control_root, &request)?;
                if first != duplicate {
                    return Err(failure("控制通道重放结果不同"));
                }
                Ok(())
            })();
            let _ = completed.send(result);
            let _ = slint::quit_event_loop();
        });
        slint::run_event_loop_until_quit().map_err(|error| failure(error.to_string()))?;
        received
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| failure("控制通道未结束"))??;
        drop(server);
        cutflow_workbench::atomic_json(
            &root.join("report.json"),
            &json!({"liveEdit":true,"duplicate":true,"revisionConflict":true,"switchProject":true,"undoRedo":true,"saveReopen":true,"loopbackUiQueue":true}),
        )
    }

    pub fn install(app: &App) -> Result<Self> {
        let root = cutflow_workbench::portable_root()?;
        let tools = root.parent().unwrap().join("tools");
        let runtime = Runtime::open(root, tools)?;
        let ledger_path = runtime.root.join("operations.json");
        let (ledger, ledger_fault) = if ledger_path.exists() {
            match std::fs::read(&ledger_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            {
                Some(ledger) => (ledger, None),
                None => (
                    BTreeMap::new(),
                    Some("编辑操作日志损坏，已禁用外部修改；请备份核查".into()),
                ),
            }
        } else {
            (BTreeMap::new(), None)
        };
        app.set_workbench_base_url(
            runtime
                .store
                .lock()
                .unwrap()
                .connection
                .base_url
                .clone()
                .into(),
        );
        app.set_workbench_automation(
            "启用后，仅当前 Windows 用户可通过 cutflow-agent.exe mcp 或 call 连接。".into(),
        );
        let (messages, replies) = mpsc::channel();
        let rows = Rc::new(VecModel::default());
        app.set_workbench_jobs(ModelRc::from(rows.clone()));
        let state = Rc::new(RefCell::new(State {
            runtime,
            server: None,
            ledger,
            revision: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            observed: Value::Null,
            session: String::new(),
            next_poll: std::time::Instant::now(),
            messages,
            replies,
            rows,
            shown: String::new(),
            busy: 0,
            ledger_fault,
        }));
        STATE.with(|slot| *slot.borrow_mut() = Some(state.clone()));
        {
            let weak = app.as_weak();
            app.on_workbench_open(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_download_opened(false);
                    app.set_workbench_opened(true);
                }
            });
        }
        {
            let weak = app.as_weak();
            app.on_workbench_close(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_workbench_opened(false);
                    app.set_workbench_api_key("".into());
                }
            });
        }
        {
            let state = state.clone();
            let weak = app.as_weak();
            app.on_workbench_action(move |action| { if let Some(app) = weak.upgrade() {
                let action = action.to_string();
                if action == "downloads" { app.set_workbench_opened(false); app.invoke_download_open(); return; }
                let mut state = state.borrow_mut();
                if action == "automation" {
                    if state.server.is_some() { state.server.take(); app.set_workbench_automation_enabled(false); }
                    else {
                        let runtime = state.runtime.clone();
                        match cutflow_workbench::server::Server::start(&runtime.root,Arc::new({let runtime=runtime.clone();move |call| route(&runtime,call)})) {
                            Ok(server) => { state.server=Some(server); app.set_workbench_automation_enabled(true); app.set_workbench_message("本地外部控制已启用".into()); }
                            Err(error) => app.set_workbench_message(error.to_string().into()),
                        }
                    }
                    return;
                }
                if action == "refresh" { state.shown.clear(); state.tick(&app); return; }
                if action == "browse" {
                    drop(state);
                    #[cfg(not(any(target_os="android",target_os="ios")))]
                    if let Some(paths) = rfd::FileDialog::new().add_filter("参考图",&["png","jpg","jpeg","webp"]).pick_files() {
                        app.set_workbench_references(paths.iter().map(|path| path.to_string_lossy()).collect::<Vec<_>>().join("\n").into());
                    }
                    return;
                }
                let (method,params) = match action.as_str() {
                    "models" => ("cutflow.models.list",json!({})),
                    "login" => ("cutflow.account.start",json!({})),
                    "poll" => ("cutflow.account.poll",json!({"sessionId":state.session})),
                    "account" => ("cutflow.account.status",json!({})),
                    "logout" => ("cutflow.account.logout",json!({})),
                    "favorites" => ("cutflow.bilibili.favorites",json!({"page":1})),
                    "favorite-items" => ("cutflow.bilibili.favorites",json!({"page":app.get_workbench_favorite_page().as_str().parse::<u32>().unwrap_or(1),"folderId":app.get_workbench_favorite_folder().as_str().parse::<u64>().ok()})),
                    "connection-save" => {
                        let params=json!({"baseUrl":app.get_workbench_base_url().as_str(),"apiKey":app.get_workbench_api_key().as_str()});
                        app.set_workbench_api_key("".into()); ("cutflow.connection.save",params)
                    }
                    "reference-save" => ("cutflow.references.save",json!({"kind":if app.get_workbench_reference_kind()==0 {"character"} else {"scene"},"name":app.get_workbench_reference_name().as_str(),
                        "path":app.get_workbench_references().as_str().lines().next().unwrap_or(""),"prompt":app.get_workbench_reference_prompt().as_str()})),
                    "generate" => {
                        match state.current() {
                            Ok(current) => ("cutflow.generate",json!({"operationId":uuid(),"projectPath":current["projectPath"],"model":app.get_workbench_model().as_str(),
                                "prompt":app.get_workbench_prompt().as_str(),"kind":if app.get_workbench_kind()==0 {"image"} else {"video"},"maxGenerationCalls":1,
                                "references":app.get_workbench_references().as_str().lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>()})),
                            Err(error) => { app.set_workbench_message(error.to_string().into()); return; }
                        }
                    }
                    _ => return,
                };
                state.submit(&app,method.into(),params);
            }});
        }
        {
            let state = state.clone();
            let weak = app.as_weak();
            app.on_workbench_use_reference(move |index| {
                if let Some(app) = weak.upgrade() {
                    let state = state.borrow();
                    let store = state.runtime.store.lock().unwrap();
                    if let Some(reference) = store.references.get(index.max(0) as usize) {
                        app.set_workbench_references(
                            reference.path.to_string_lossy().into_owned().into(),
                        );
                        app.set_workbench_prompt(reference.prompt.clone().into());
                    }
                }
            });
        }
        {
            let state = state.clone();
            let weak = app.as_weak();
            app.on_workbench_job(move |id,action| { if let Some(app) = weak.upgrade() {
                let mut state = state.borrow_mut();
                if action == "cancel" { state.submit(&app,"cutflow.jobs.cancel".into(),json!({"jobId":id.as_str()})); return; }
                let assets = {let store=state.runtime.store.lock().unwrap();store.jobs.iter().find(|job| job.id == id.as_str()).and_then(|job|job.result.as_ref()).map(|result|result["assets"].clone()).unwrap_or(Value::Null)};
                let files=assets.as_array().map(|assets|assets.iter().filter_map(|asset|asset["path"].as_str().map(str::to_owned)).collect::<Vec<_>>()).unwrap_or_default();
                match state.current() { Ok(current) => state.submit(&app,"cutflow.assets.import".into(),json!({"operationId":uuid(),"projectPath":current["projectPath"],"expectedRevision":current["revision"],"files":files,"place":action=="place"})),
                    Err(error)=>app.set_workbench_message(error.to_string().into()) }
            }});
        }
        let timer = slint::Timer::default();
        {
            let state = state.clone();
            let weak = app.as_weak();
            timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(250),
                move || {
                    if let Some(app) = weak.upgrade() {
                        state.borrow_mut().tick(&app);
                    }
                },
            );
        }
        Ok(Self { state, timer })
    }

    pub fn shutdown(&self) {
        self.timer.stop();
        self.state.borrow_mut().server.take();
    }
}

fn uuid() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn assemble(params: &Value) -> Result<Vec<Command>> {
    let mut editor = None;
    Shell::with(|shell, _app| {
        editor = shell
            .studio
            .borrow()
            .session
            .as_ref()
            .map(|session| session.sandbox());
    });
    let mut trial = editor.ok_or_else(|| failure("工程未打开"))?;
    let mut commands = Vec::new();
    if let Some(output) = params.get("output").filter(|value| !value.is_null()) {
        let video: VideoSettings = serde_json::from_value(output.clone())?;
        if video.width == 0 || video.height == 0 || video.rate_num <= 0 || video.rate_den <= 0 {
            return Err(failure("输出尺寸或帧率无效"));
        }
        let command = Command::SetTimelineVideo {
            timeline_id: trial.project().active().id.clone(),
            video,
        };
        trial
            .apply(command.clone())
            .map_err(|error| failure(error.to_string()))?;
        commands.push(command);
    }
    let frame_rate = trial.project().active().video.rate();
    let aligned = |seconds: f64| (seconds * frame_rate).round() / frame_rate;
    let mut at = trial
        .project()
        .active()
        .clips
        .iter()
        .map(|clip| clip.start + clip.duration)
        .fold(0.0, f64::max);
    let begin = at;
    let prepared = params["prepared"]
        .as_array()
        .ok_or_else(|| failure("缺少准备好的分镜"))?;
    for scene in prepared {
        let media: NewMedia = serde_json::from_value(scene["media"].clone())?;
        let duration = aligned(
            scene["duration"]
                .as_f64()
                .ok_or_else(|| failure("分镜缺少时长"))?,
        )
        .max(1.0 / frame_rate);
        let source_start = aligned(scene["sourceStart"].as_f64().unwrap_or(0.0));
        let narration = scene["narration"].as_array().cloned().unwrap_or_default();
        let place = params["place"].as_bool().unwrap_or(true);
        append_media(
            &mut trial,
            &mut commands,
            media,
            at,
            duration,
            source_start,
            place,
            !narration.is_empty(),
        )?;
        let mut voice_at = at;
        let mut spoken_seconds=0.0;
        for phrase in narration {
            let phrase_media: NewMedia = serde_json::from_value(phrase["media"].clone())?;
            spoken_seconds+=phrase["duration"].as_f64().ok_or_else(||failure("配音时长缺失"))?;
            let phrase_duration=(aligned(at+spoken_seconds)-voice_at).max(1.0/frame_rate);
            append_media(
                &mut trial,
                &mut commands,
                phrase_media,
                voice_at,
                phrase_duration,
                0.0,
                true,
                false,
            )?;
            if scene["subtitles"] == true {
                let command = Command::AddTextClip {
                    track_id: None,
                    above: true,
                    start: aligned(voice_at),
                    duration: Some(phrase_duration),
                    offset_y: Some(0.35),
                    style: Some(TextStyle {
                        content: phrase["text"].as_str().unwrap_or("").into(),
                        font_size: 0.04,
                        ..TextStyle::default()
                    }),
                };
                trial
                    .apply(command.clone())
                    .map_err(|error| failure(error.to_string()))?;
                commands.push(command);
            }
            voice_at += phrase_duration;
        }
        at = aligned(at + duration);
    }
    if let Some(bgm) = params.get("preparedBgm") {
        let duration = bgm["duration"].as_f64().unwrap_or(0.0);
        if duration < at - begin {
            return Err(Error::new(
                "insufficient_material",
                "背景音乐短于草稿，请选择完整配乐",
            ));
        }
        let clip = append_media(
            &mut trial,
            &mut commands,
            serde_json::from_value(bgm["media"].clone())?,
            begin,
            at - begin,
            0.0,
            true,
            false,
        )?;
        if let Some(clip_id) = clip {
            let command: Command = serde_json::from_value(
                json!({"op":"updateClip","clipId":clip_id,"patch":{"volume":0.12}}),
            )?;
            trial
                .apply(command.clone())
                .map_err(|error| failure(error.to_string()))?;
            commands.push(command);
        }
    }
    Ok(commands)
}

fn prepare(
    runtime: &Runtime,
    job: &Job,
    cancel: &Arc<AtomicBool>,
    speech: &Arc<concat_speech::tts::Speech>,
    transcriber: &Arc<concat_speech::transcribe::Transcriber>,
) -> Result<Value> {
    let dirs = concat_host::AppDirs::under(&runtime.root);
    let project =
        Path::new(cutflow_workbench::required(&job.params, "projectPath")?).canonicalize()?;
    let project_text = project.to_string_lossy().into_owned();
    let folder = project.join("assets/cutflow").join(&job.id);
    std::fs::create_dir_all(&folder)?;
    let speak = |text: &str| -> Result<concat_speech::tts::SpeakResult> {
        let model = job.params["modelId"]
            .as_str()
            .unwrap_or("kokoro-int8-multi-lang-v1_0");
        let status = concat_speech::tts::Speech::status(&dirs).map_err(failure)?;
        if !status
            .models
            .iter()
            .any(|entry| entry.id == model && entry.downloaded)
        {
            runtime.update(&job.id, "downloading_voice_model");
            speech
                .download_model(&dirs, model, |_| {})
                .map_err(failure)?;
        }
        runtime.update(&job.id, "narration");
        speech
            .speak(
                &dirs,
                &concat_speech::tts::SpeakRequest {
                    model_id: model.into(),
                    voice: job.params["voice"].as_i64().unwrap_or(45) as i32,
                    text: text.into(),
                    speed: job.params["speed"].as_f64().unwrap_or(1.0) as f32,
                    pauses: None,
                    steps: None,
                    reference: None,
                    project: project_text.clone(),
                },
                |_| {},
            )
            .map_err(failure)
    };
    if job.method == "cutflow.speech.run" {
        let result = speak(cutflow_workbench::required(&job.params, "text")?)?;
        return Ok(json!({"assets":[{"path":result.path,"duration":result.duration}]}));
    }
    if job.method == "cutflow.captions.run" {
        let model = job.params["modelId"].as_str().unwrap_or("base");
        let status = concat_speech::transcribe::Transcriber::status(&dirs).map_err(failure)?;
        if !status
            .models
            .iter()
            .any(|entry| entry.id == model && entry.downloaded)
        {
            runtime.update(&job.id, "downloading_caption_model");
            transcriber
                .download_model(&dirs, model, |_| {})
                .map_err(failure)?;
        }
        runtime.update(&job.id, "transcribing");
        let file = cutflow_workbench::required(&job.params, "file")?;
        let media = concat_host::media::probe(file).map_err(failure)?;
        let segments = transcriber
            .transcribe(
                &dirs,
                &concat_speech::transcribe::TranscribeRequest {
                    path: file.into(),
                    audio_stream: None,
                    source_start: 0.0,
                    window: media.duration.ok_or_else(|| failure("无法确认音频时长"))?,
                    model_id: job.params["modelId"].as_str().unwrap_or("base").into(),
                },
                |_| {},
            )
            .map_err(failure)?;
        return Ok(
            json!({"segments":segments.iter().map(|segment|json!({"start":segment.start,"end":segment.end,"text":segment.text})).collect::<Vec<_>>()}),
        );
    }
    let mut scenes = Vec::<Value>::new();
    if job.method == "cutflow.assets.import" {
        let files = job.params["files"]
            .as_array()
            .filter(|files| !files.is_empty())
            .ok_or_else(|| Error::new("invalid_request", "files 不能为空"))?;
        for file in files {
            let path = owned_media(
                &project,
                &folder,
                Path::new(file.as_str().ok_or_else(|| failure("文件路径无效"))?),
            )?;
            let media = concat_host::media::probe(&path.to_string_lossy()).map_err(failure)?;
            scenes.push(json!({"media":media.to_new_media(),"duration":media.duration.unwrap_or(5.0),"sourceStart":0,"narration":[],"subtitles":false}));
        }
    } else {
        let spec: TaskSpec = serde_json::from_value(job.params.clone())?;
        spec.validate()?;
        for scene in &spec.scenes {
            match &scene.source {
                Source::Local { path } => {
                    concat_host::media::probe(path).map_err(failure)?;
                }
                Source::Generate {
                    model, references, ..
                } => {
                    if references.len()
                        > cutflow_workbench::flow::describe(model, "").max_references
                    {
                        return Err(failure("分镜参考图数量超过模型能力"));
                    }
                    for path in references {
                        cutflow_workbench::flow::reference_data(Path::new(path))?;
                    }
                }
                Source::Bilibili {
                    url, rights, parts, ..
                } => {
                    if url.trim().is_empty() || rights.trim().is_empty() {
                        return Err(failure("B站分镜缺少链接或素材授权说明"));
                    }
                    cutflow_download::validate_parts(parts).map_err(failure)?;
                    if parts.contains(',') {
                        return Err(failure("每个分镜请选择一个分 P，多 P 请拆成多个分镜"));
                    }
                }
            }
        }
        for (index, scene) in spec.scenes.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(Error::new("cancelled", "流程已取消，已完成素材保留"));
            }
            let checkpoint = folder.join(format!("scene-{index}.json"));
            if checkpoint.exists() {
                scenes.push(serde_json::from_slice(&std::fs::read(checkpoint)?)?);
                continue;
            }
            runtime.update(
                &job.id,
                &format!("准备分镜 {} / {}", index + 1, spec.scenes.len()),
            );
            let path = match &scene.source {
                Source::Local { path } => owned_media(&project, &folder, Path::new(path))?,
                source => {
                    let (method, mut params) = match source {
                        Source::Generate {
                            model,
                            prompt,
                            media_kind,
                            references,
                        } => (
                            "cutflow.generate",
                            json!({"model":model,"prompt":prompt,"kind":media_kind,"references":references,"maxGenerationCalls":1}),
                        ),
                        Source::Bilibili {
                            url,
                            rights,
                            parts,
                            quality,
                        } => (
                            "cutflow.bilibili.download",
                            json!({"url":url,"rights":rights,"parts":parts,"quality":quality,"allowLower":false}),
                        ),
                        _ => unreachable!(),
                    };
                    params["projectPath"] = json!(project_text);
                    params["operationId"] = json!(format!("{}:scene:{index}", job.operation_id));
                    let (child, fresh, _child_cancel) = runtime.reserve(method, params)?;
                    let result = if fresh {
                        runtime.link_cancel(&child.id, cancel.clone());
                        let result = runtime.execute(&child, cancel);
                        runtime.finish(&child.id, result.clone());
                        result?
                    } else {
                        child.result.ok_or_else(|| {
                            Error::new(
                                "result_unknown",
                                "分镜上次生成中断；请检查任务历史并引用已生成素材，不会自动重发",
                            )
                        })?
                    };
                    PathBuf::from(
                        result["assets"][0]["path"]
                            .as_str()
                            .ok_or_else(|| failure("素材任务没有输出文件"))?,
                    )
                }
            };
            let media = concat_host::media::probe(&path.to_string_lossy()).map_err(failure)?;
            let mut narration = Vec::new();
            let mut spoken = 0.0;
            for sentence in cutflow_workbench::workflow::sentences(&scene.narration) {
                if cancel.load(Ordering::SeqCst) {
                    return Err(Error::new("cancelled", "配音已取消"));
                }
                let result = speak(&sentence)?;
                let summary = concat_host::media::probe(&result.path).map_err(failure)?;
                let actual = summary
                    .duration
                    .ok_or_else(|| failure("配音缺少真实时长"))?;
                narration.push(json!({"path":result.path,"media":summary.to_new_media(),"duration":actual,"text":sentence}));
                spoken += actual;
            }
            let duration = if spoken > 0.0 {
                spoken
            } else {
                scene
                    .duration
                    .unwrap_or(media.duration.unwrap_or(5.0) - scene.source_start)
            };
            if duration <= 0.0
                || media.kind != MediaKind::Image
                    && media
                        .duration
                        .is_none_or(|available| available + 0.04 < scene.source_start + duration)
            {
                return Err(Error::new(
                    "insufficient_material",
                    format!(
                        "分镜 {} 的素材短于需要的 {duration:.2} 秒，请补充或缩短旁白",
                        scene.id
                    ),
                ));
            }
            let prepared = json!({"media":media.to_new_media(),"duration":duration,"sourceStart":scene.source_start,"narration":narration,"subtitles":spec.subtitles});
            cutflow_workbench::atomic_json(&checkpoint, &prepared)?;
            scenes.push(prepared);
        }
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(Error::new("cancelled", "提交草稿前已取消"));
    }
    runtime.update(&job.id, "提交草稿：核对工程版本");
    let mut params = job.params.clone();
    params["prepared"] = json!(scenes);
    params["operationId"] = json!(format!("{}:commit", job.operation_id));
    if let Some(bgm) = job.params["bgm"].as_str().filter(|path| !path.is_empty()) {
        let path = owned_media(&project, &folder, Path::new(bgm))?;
        let summary = concat_host::media::probe(&path.to_string_lossy()).map_err(failure)?;
        if summary.audio.is_none() {
            return Err(failure("配乐文件没有音轨"));
        }
        params["preparedBgm"] = json!({"media":summary.to_new_media(),"duration":summary.duration});
    }
    ui_call(Call {
        method: "cutflow.internal.assemble".into(),
        params,
    })
}

fn owned_media(project: &Path, folder: &Path, source: &Path) -> Result<PathBuf> {
    cutflow_workbench::media::copy_into_project(project, folder, source)
}
