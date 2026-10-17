use crate::{Error, Result, atomic_json, flow, required, secret, creation::{self, CreationProject, CreationSession, CreationTurn}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub operation_id: String,
    pub method: String,
    pub params: Value,
    pub status: String,
    pub stage: String,
    pub result: Option<Value>,
    pub error: Option<Error>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: PathBuf,
    pub prompt: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub version: u32,
    pub connection: flow::Connection,
    pub models: Vec<flow::Model>,
    pub references: Vec<Reference>,
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub projects: Vec<CreationProject>,
    #[serde(default)]
    pub sessions: Vec<CreationSession>,
    #[serde(default)]
    pub turns: Vec<CreationTurn>,
    #[serde(default="creation::workspace")]
    pub workspace_id: String,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub project_binding: bool,
    #[serde(default)]
    pub project_binding_fault: Option<String>,
    #[serde(default)]
    pub revision: u64,
}

#[derive(Clone)]
pub struct Runtime {
    _lock: Arc<std::fs::File>,
    pub root: PathBuf,
    pub tools: PathBuf,
    pub account_root: PathBuf,
    pub store: Arc<Mutex<Store>>,
    cancels: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    pub fault: Option<String>,
    pub(crate) project_creation_gate: Arc<Mutex<()>>,
    persist_gate: Arc<Mutex<()>>,
    reservation_gate: Arc<Mutex<()>>,
    retry_cache: Arc<Mutex<BTreeMap<String, (u64, bool)>>>,
}

impl Runtime {
    pub fn open(root: PathBuf, tools: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("workbench.lock"))?;
        lock.try_lock()
            .map_err(|_| Error::new("busy", "此软件目录已有工作台运行，请使用已打开的窗口"))?;
        let path = root.join("workbench.json");
        let (mut store, fault) = if path.exists() {
            match serde_json::from_slice::<Store>(&std::fs::read(&path)?) {
                Ok(store) if matches!(store.version, 1 | 2) => (store, None),
                _ => (
                    Store::default(),
                    Some("任务存储损坏或版本不兼容，禁止覆盖；请先备份核查".into()),
                ),
            }
        } else {
            (
                Store {
                    version: 2,
                    workspace_id: creation::workspace(),
                    ..Store::default()
                },
                None,
            )
        };
        if store.version == 1 && fault.is_none() {
            // Keep the original bytes before migrating any state or asset references.
            let backup = root.join("backups").join("workbench.v1.json");
            if !backup.exists() {
                std::fs::create_dir_all(backup.parent().unwrap())?;
                std::fs::copy(&path, &backup)?;
            }
            creation::migrate(&mut store);
        }
        for session in &mut store.sessions {
            session.drafts = creation::normalize_drafts(&session.drafts);
        }
        for project in &mut store.projects {
            if project.state == "creating" {
                project.state = "creation_unknown".into();
                project.error = Some("应用重启，创建结果待核查；请查询原请求，禁止自动重建".into());
            }
        }
        store.revision = store.revision.saturating_add(1);
        let revision = store.revision;
        for job in &mut store.jobs {
            if matches!(job.status.as_str(), "queued" | "running") {
                job.status = if job.stage.starts_with("generating") {
                    "result_unknown"
                } else {
                    "interrupted"
                }
                .into();
                job.stage = "程序已重启；请检查已完成素材，不会自动重发生成".into();
                job.revision = revision;
                job.updated_at = creation::now();
            }
        }
        let account_root = root.join("localappdata/CutFlow");
        let runtime = Self {
            _lock: Arc::new(lock),
            root,
            tools,
            account_root,
            store: Arc::new(Mutex::new(store)),
            cancels: Arc::new(Mutex::new(BTreeMap::new())),
            fault,
            project_creation_gate: Arc::new(Mutex::new(())),
            persist_gate: Arc::new(Mutex::new(())),
            reservation_gate: Arc::new(Mutex::new(())),
            retry_cache: Arc::new(Mutex::new(BTreeMap::new())),
        };
        if runtime.fault.is_none() {
            runtime.persist()?;
        }
        Ok(runtime)
    }

    pub fn persist(&self) -> Result<()> {
        if let Some(fault) = &self.fault {
            return Err(Error::new("store_error", fault));
        }
        let _serial = self.persist_gate.lock().unwrap();
        let snapshot = self.store.lock().unwrap().clone();
        atomic_json(&self.root.join("workbench.json"), &snapshot)
    }

    pub fn existing(&self, method: &str, params: &Value) -> Result<Option<Job>> {
        let operation = required(params, "operationId")?;
        let store = self.store.lock().unwrap();
        if let Some(job) = store.jobs.iter().find(|job| job.operation_id == operation) {
            if job.method != method || job.params != *params {
                return Err(Error::new(
                    "operation_conflict",
                    "operationId 已被其他请求使用",
                ));
            }
            return Ok(Some(job.clone()));
        }
        Ok(None)
    }

    pub fn link_cancel(&self, id: &str, cancel: Arc<AtomicBool>) {
        self.cancels.lock().unwrap().insert(id.to_string(), cancel);
    }

    fn can_retry_download(&self, job: &Job) -> bool {
        if job.method != "cutflow.generate"
            || !matches!(job.status.as_str(), "failed" | "cancelled" | "interrupted" | "result_unknown") {
            return false;
        }
        let receipt = self.output_folder(job).ok()
            .and_then(|folder| std::fs::read(folder.join("generation-result.json")).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        let Some(receipt) = receipt else { return false; };
        let urls = receipt["urls"].as_array().cloned().unwrap_or_else(|| vec![receipt["url"].clone()]);
        let saved = job.result.as_ref().and_then(|result| result["assets"].as_array());
        urls.iter().enumerate().any(|(index, url)| {
            url.as_str().is_some_and(|url| !url.is_empty())
                && !saved.and_then(|assets| assets.get(index))
                    .and_then(|asset| asset["path"].as_str())
                    .is_some_and(|path| Path::new(path).is_file())
        })
    }

    pub(crate) fn job_view(&self, job: &Job) -> Value {
        let cached = self.retry_cache.lock().unwrap().get(&job.id).copied();
        let retry = if let Some((revision, retry)) = cached.filter(|(revision, _)| *revision == job.revision) {
            let _ = revision;
            retry
        } else {
            let retry = self.can_retry_download(job);
            self.retry_cache.lock().unwrap().insert(job.id.clone(), (job.revision, retry));
            retry
        };
        let mut value = json!(job);
        value["canRetryDownload"] = json!(retry);
        value
    }

    pub fn snapshot(&self) -> Value {
        // Compatibility snapshot: clone before any receipt/file inspection.
        let store = self.store.lock().unwrap().clone();
        let jobs = store.jobs.iter().map(|job| {
            let mut value = json!(job);
            value["canRetryDownload"] = json!(self.can_retry_download(job));
            value
        }).collect::<Vec<_>>();
        json!({"connection":store.connection,"models":store.models,"references":store.references,
            "jobs":jobs,"fault":self.fault})
    }

    pub fn jobs_list(&self, params: &Value) -> Value {
        let (jobs, revision) = {
            let store = self.store.lock().unwrap();
            let since = params["sinceRevision"].as_u64().unwrap_or(0);
            let jobs = store.jobs.iter().filter(|job| {
                (!params["activeOnly"].as_bool().unwrap_or(false) || matches!(job.status.as_str(), "running" | "queued"))
                && params["sessionId"].as_str().is_none_or(|id| job.params["sessionId"].as_str() == Some(id))
                && (since == 0 || job.revision > since)
            }).cloned().collect::<Vec<_>>();
            (jobs, store.revision)
        };
        json!({"jobs":jobs.iter().map(|job| self.job_view(job)).collect::<Vec<_>>(),"revision":revision})
    }

    pub fn update(&self, id: &str, stage: &str) {
        {
            let mut store = self.store.lock().unwrap();
            store.revision = store.revision.saturating_add(1);
            let revision = store.revision;
            if let Some(job) = store.jobs.iter_mut().find(|job| job.id == id) {
                job.revision = revision;
                job.updated_at = creation::now();
                job.stage = stage.into();
            }
        }
        if let Err(error) = self.persist() {
            if let Some(cancel) = self.cancels.lock().unwrap().get(id) { cancel.store(true, Ordering::SeqCst); }
            let mut store = self.store.lock().unwrap();
            store.revision = store.revision.saturating_add(1);
            let revision = store.revision;
            if let Some(job) = store.jobs.iter_mut().find(|job| job.id == id) {
                job.revision = revision;
                job.updated_at = creation::now();
                job.stage = format!("任务检查点保存失败：{}", error.message);
            }
        }
    }

    pub fn finish(&self, id: &str, result: Result<Value>) {
        {
            let mut store = self.store.lock().unwrap();
            store.revision = store.revision.saturating_add(1);
            let revision = store.revision;
            if let Some(job) = store.jobs.iter_mut().find(|job| job.id == id) {
                job.revision = revision;
                job.updated_at = creation::now();
                match result {
                    Ok(result) => {
                        job.status = "completed".into();
                        job.stage = "已完成".into();
                        job.result = Some(result);
                        job.error = None;
                    }
                    Err(error) => {
                        job.status = match error.code.as_str() {
                            "result_unknown" => "result_unknown",
                            "cancelled" => "cancelled",
                            "revision_conflict" | "project_changed" => "conflict",
                            _ => "failed",
                        }
                        .into();
                        job.stage = error.message.clone();
                        job.error = Some(error);
                    }
                }
            }
        }
        self.cancels.lock().unwrap().remove(id);
        if let Err(error) = self.persist() {
            let mut store = self.store.lock().unwrap();
            store.revision = store.revision.saturating_add(1);
            let revision = store.revision;
            if let Some(job) = store.jobs.iter_mut().find(|job| job.id == id) {
                job.revision = revision;
                job.updated_at = creation::now();
                job.status = "result_unknown".into();
                job.stage = format!("结果未持久化：{}。退出前请保留素材", error.message);
                job.error = Some(error);
            }
        }
    }

    pub fn reserve(&self, method: &str, mut params: Value) -> Result<(Job, bool, Arc<AtomicBool>)> {
        let _serial = self.reservation_gate.lock().unwrap();
        if self.fault.is_some() { self.persist()?; }
        let operation = required(&params, "operationId")?.to_string();
        if operation.len() > 128 { return Err(Error::new("invalid_request", "operationId 过长")); }
        if let Some(previous) = self.store.lock().unwrap().jobs.iter().find(|job| job.operation_id == operation).cloned() {
            let mut original = previous.params.clone();
            let mut incoming = params.clone();
            if previous.params["creationProjectId"].is_string() {
                for field in ["gflow_workspace_id", "gflow_project_id", "gflow_instance_id"] {
                    original.as_object_mut().unwrap().remove(field);
                    incoming.as_object_mut().unwrap().remove(field);
                }
            }
            if previous.method != method || original != incoming {
                return Err(Error::new("operation_conflict", "operationId 已被其他请求使用"));
            }
            return Ok((previous, false, Arc::new(AtomicBool::new(false))));
        }
        if method == "cutflow.generate" { self.bind_generation(&mut params)?; }
        let mut store = self.store.lock().unwrap();
        if self.cancels.lock().unwrap().len() >= 4 { return Err(Error::new("busy", "同时最多执行四个后台任务")); }
        if method == "cutflow.generate" {
            if let Some(session_id) = params["sessionId"].as_str() {
                if store.jobs.iter().any(|job| job.params["sessionId"].as_str() == Some(session_id)
                    && matches!(job.status.as_str(), "running" | "queued")) {
                    return Err(Error::new("session_busy", "当前聊天已有一轮生成，请等待完成或停止后继续"));
                }
            }
            if let Some(turn_id) = params["turnId"].as_str() {
                if store.turns.iter().any(|turn| turn.id == turn_id) {
                    return Err(Error::new("turn_conflict", "turnId 已被另一轮生成使用"));
                }
            }
        }
        store.revision = store.revision.saturating_add(1);
        let timestamp = creation::now();
        let job = Job {
            revision:store.revision, created_at:timestamp, updated_at:timestamp,
            id:uuid::Uuid::new_v4().to_string(), operation_id:operation, method:method.into(), params,
            status:"running".into(), stage:"queued".into(), result:None, error:None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.lock().unwrap().insert(job.id.clone(), cancel.clone());
        store.jobs.push(job.clone());
        if let (Some(session_id), Some(turn_id), Some(project_id)) =
            (job.params["sessionId"].as_str(), job.params["turnId"].as_str(), job.params["creationProjectId"].as_str()) {
            store.turns.push(CreationTurn { id:turn_id.into(), session_id:session_id.into(), job_id:job.id.clone(),
                prompt:job.params["prompt"].as_str().unwrap_or("").into(), params:job.params.clone(), created_at:timestamp });
            if let Some(session) = store.sessions.iter_mut().find(|s| s.id == session_id) {
                session.updated_at = timestamp;
                if session.title == "新聊天" { session.title = job.params["prompt"].as_str().unwrap_or("新聊天").chars().take(24).collect(); }
            }
            if let Some(project) = store.projects.iter_mut().find(|p| p.id == project_id) { project.updated_at = timestamp; }
        }
        drop(store);
        if let Err(error) = self.persist() {
            let mut store = self.store.lock().unwrap();
            store.jobs.retain(|entry| entry.id != job.id);
            store.turns.retain(|turn| turn.job_id != job.id);
            self.cancels.lock().unwrap().remove(&job.id);
            return Err(error);
        }
        Ok((job, true, cancel))
    }

    pub fn background(&self, method: &str, params: Value) -> Result<Value> {
        let (job, fresh, cancel) = self.reserve(method, params)?;
        if fresh {
            let runtime = self.clone();
            let running = job.clone();
            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    runtime.execute(&running, &cancel)
                }))
                .unwrap_or_else(|_| Err(Error::new("interrupted", "后台任务异常结束，请核查结果")));
                runtime.finish(&running.id, result);
            });
        }
        Ok(json!({"jobId":job.id,"status":job.status}))
    }

    pub fn dispatch(&self, method: &str, params: Value) -> Result<Value> {
        if method.starts_with("cutflow.creation.") { return self.creation_dispatch(method, params); }
        match method {
            "cutflow.connection.get" => Ok(json!(self.store.lock().unwrap().connection)),
            "cutflow.connection.save" => {
                let base_url = flow::base_url(required(&params, "baseUrl")?)?;
                if let Some(key) = params["apiKey"].as_str().filter(|key| !key.is_empty()) {
                    if key.contains(['\r', '\n']) || key.len() > 4096 {
                        return Err(Error::new("invalid_request", "API Key 格式无效"));
                    }
                    secret(&self.root, "flow-key", Some(key))?;
                }
                {
                    let mut store = self.store.lock().unwrap();
                    store.connection.base_url = base_url;
                    store.project_binding = false;
                    store.instance_id = None;
                    store.revision = store.revision.saturating_add(1);
                }
                self.persist()?;
                Ok(json!({"saved":true}))
            }
            "cutflow.models.list" => {
                let connection = self.store.lock().unwrap().connection.clone();
                let key = self.api_key()?;
                let mut models = flow::models(&connection, &key)?;
                {
                    let mut store = self.store.lock().unwrap();
                    if store.connection.base_url != connection.base_url {
                        return Err(Error::new("connection_changed", "模型响应来自旧服务连接，请重新刷新"));
                    }
                    for model in &mut models {
                        if let Some(previous) = store.models.iter().find(|previous| previous.id == model.id) {
                            model.aspects = model.aspects.take().or_else(|| previous.aspects.clone());
                            model.image_sizes = model.image_sizes.take().or_else(|| previous.image_sizes.clone());
                            model.durations = model.durations.take().or_else(|| previous.durations.clone());
                        }
                    }
                    store.models = models.clone();
                    store.revision = store.revision.saturating_add(1);
                }
                self.persist()?;
                Ok(json!({"models":models}))
            }
            "cutflow.connection.test" => {
                let connection = self.store.lock().unwrap().connection.clone();
                flow::check_response(&connection, &self.api_key()?, required(&params, "model")?)
            }
            "cutflow.account.start" => crate::accounts::start(),
            "cutflow.account.poll" => {
                crate::accounts::poll(&self.account_root, required(&params, "sessionId")?)
            }
            "cutflow.account.status" => crate::accounts::status(&self.account_root),
            "cutflow.account.logout" => {
                cutflow_download::credentials::clear(&self.account_root)
                    .map_err(|message| Error::new("credential_error", message))?;
                Ok(json!({"loggedIn":false}))
            }
            "cutflow.bilibili.favorites" => crate::accounts::favorites(
                &self.account_root,
                params["page"].as_u64().unwrap_or(1).clamp(1, 10000) as u32,
                params["folderId"].as_u64(),
            ),
            "cutflow.bilibili.parse" => {
                let engine = self.download_engine();
                let text = engine
                    .parse(required(&params, "url")?, &AtomicBool::new(false))
                    .map_err(|message| Error::new("download_error", message))?;
                Ok(json!({"formats":text}))
            }
            "cutflow.generate" | "cutflow.bilibili.download" => self.background(method, params),
            "cutflow.jobs.retryDownload" => {
                let id = required(&params, "jobId")?;
                let mut job = self.store.lock().unwrap().jobs.iter().find(|job| job.id == id).cloned()
                    .ok_or_else(|| Error::new("not_found", "任务不存在"))?;
                if job.method != "cutflow.generate" || matches!(job.status.as_str(), "running" | "queued" | "completed") {
                    return Err(Error::new("invalid_request", "只有已返回媒体地址的失败生成任务可以重试下载"));
                }
                let folder = self.output_folder(&job)?;
                if !self.can_retry_download(&job) {
                    return Err(Error::new("result_unknown", "尚未收到媒体地址，请先核查服务端历史"));
                }
                let cancel = Arc::new(AtomicBool::new(false));
                {
                    let mut cancels = self.cancels.lock().unwrap();
                    if cancels.len() >= 4 || cancels.contains_key(id) { return Err(Error::new("busy", "任务正在执行")); }
                    cancels.insert(id.into(), cancel.clone());
                }
                let previous = job.clone();
                job.status = "running".into();
                job.stage = "downloading".into();
                job.error = None;
                {
                    let mut store = self.store.lock().unwrap();
                    store.revision = store.revision.saturating_add(1);
                    job.revision = store.revision;
                    job.updated_at = creation::now();
                    *store.jobs.iter_mut().find(|item| item.id == id).unwrap() = job.clone();
                }
                if let Err(error) = self.persist() {
                    self.cancels.lock().unwrap().remove(id);
                    let mut store = self.store.lock().unwrap();
                    store.revision = store.revision.saturating_add(1);
                    let revision = store.revision;
                    if let Some(current) = store.jobs.iter_mut().find(|item| item.id == id) {
                        *current = previous;
                        current.revision = revision;
                        current.updated_at = creation::now();
                    }
                    return Err(error);
                }
                let runtime = self.clone();
                std::thread::spawn(move || { let result = runtime.download_generation(&job, &folder, &cancel); runtime.finish(&job.id, result); });
                Ok(json!({"jobId":id,"status":"running"}))
            }
            "cutflow.jobs.list" => Ok(self.jobs_list(&params)),
            "cutflow.jobs.get" => {
                let job = self.store.lock().unwrap().jobs.iter()
                    .find(|job| Some(job.id.as_str()) == params["jobId"].as_str()).cloned()
                    .ok_or_else(|| Error::new("not_found", "任务不存在"))?;
                Ok(self.job_view(&job))
            }
            "cutflow.jobs.cancel" => {
                let id = required(&params, "jobId")?;
                {
                    let cancels = self.cancels.lock().unwrap();
                    let cancel = cancels.get(id).ok_or_else(|| Error::new("not_running", "任务已结束或中断"))?;
                    cancel.store(true, Ordering::SeqCst);
                }
                self.update(id, "正在停止生成请求…");
                Ok(
                    json!({"requested":true,"message":"本地取消已请求；服务端生成可能仍在继续并计费"}),
                )
            }
            "cutflow.references.list" => {
                Ok(json!({"references":self.store.lock().unwrap().references}))
            }
            "cutflow.references.save" => {
                let kind = required(&params, "kind")?;
                if !["character", "scene"].contains(&kind) {
                    return Err(Error::new(
                        "invalid_request",
                        "kind 应为 character 或 scene",
                    ));
                }
                let source = Path::new(required(&params, "path")?).canonicalize()?;
                flow::reference_data(&source)?;
                let id = uuid::Uuid::new_v4().to_string();
                let folder = self.root.join("references");
                std::fs::create_dir_all(&folder)?;
                let path = folder.join(format!(
                    "{id}.{}",
                    source
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or("png")
                ));
                std::fs::copy(source, &path)?;
                let reference = Reference {
                    id,
                    kind: kind.into(),
                    name: required(&params, "name")?.into(),
                    path,
                    prompt: params["prompt"].as_str().unwrap_or("").into(),
                };
                self.store
                    .lock()
                    .unwrap()
                    .references
                    .push(reference.clone());
                self.persist()?;
                Ok(json!(reference))
            }
            _ => Err(Error::new("method_not_found", format!("未知方法 {method}"))),
        }
    }

    pub(crate) fn api_key(&self) -> Result<String> {
        secret(&self.root, "flow-key", None)?
            .filter(|key| !key.is_empty())
            .ok_or_else(|| Error::new("configuration_required", "请在 AI 创作设置中保存 API Key"))
    }

    pub fn download_engine(&self) -> cutflow_download::Engine {
        cutflow_download::Engine {
            tools: self.tools.clone(),
            data: self.account_root.clone(),
        }
    }

    fn output_folder(&self, job: &Job) -> Result<PathBuf> {
        if let (Some(project_id), Some(session_id)) = (job.params["creationProjectId"].as_str(), job.params["sessionId"].as_str()) {
            // IDs come from our persisted UUID records, never from arbitrary user paths.
            if ![project_id, session_id, &job.id].iter().all(|id| id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')) {
                return Err(Error::new("invalid_request", "创作目录标识无效"));
            }
            Ok(self.root.join("generated").join(project_id).join(session_id).join(&job.id))
        } else if let Some(path) = job.params["projectPath"].as_str().filter(|path| !path.is_empty()) {
            let project = Path::new(path).canonicalize()?;
            if !project.is_dir() { return Err(Error::new("invalid_request", "工程目录不存在")); }
            Ok(project.join("assets").join("cutflow").join(&job.id))
        } else if job.method == "cutflow.generate" {
            Ok(self.root.join("generated").join(&job.id))
        } else {
            Err(Error::new("project_required", "此操作需要 projectPath"))
        }
    }

    fn download_generation(&self, job: &Job, folder: &Path, cancel: &AtomicBool) -> Result<Value> {
        let receipt: Value = serde_json::from_slice(&std::fs::read(folder.join("generation-result.json"))?)?;
        let connection: flow::Connection = receipt.get("connection").cloned().map(serde_json::from_value).transpose()?
            .unwrap_or_else(|| self.store.lock().unwrap().connection.clone());
        let kind = required(&job.params, "kind")?;
        let extension = if kind == "image" { "png" } else { "mp4" };
        let urls = receipt["urls"].as_array().cloned().unwrap_or_else(|| vec![receipt["url"].clone()]);
        let mut assets = Vec::new();
        let mut thumbnail = None;
        for (index, url) in urls.iter().enumerate() {
            let destination = folder.join(format!("generated-{index}.{extension}"));
            let existing = ["png", "jpg", "webp", "mp4", "webm", "mov"].iter().map(|ext| destination.with_extension(ext)).find(|path| path.is_file());
            let path = match existing { Some(path) => path, None => flow::download(&connection, url.as_str().ok_or_else(|| Error::new("invalid_media", "结果地址无效"))?, &destination, cancel)? };
            let mut asset = self.probe(&path, cancel)?;
            if (kind == "video" && (asset["duration"].as_f64().is_none() || asset["width"].as_u64().unwrap_or(0) == 0)) || (kind == "image" && !asset["stillImage"].as_bool().unwrap_or(false)) {
                return Err(Error::new("invalid_media", "生成结果类型与请求不符，文件已保留供核查"));
            }
            let asset_thumbnail = self.thumbnail_named(&path, folder, &format!("thumbnail-{index}.jpg"), cancel);
            asset["thumbnail"] = json!(asset_thumbnail);
            if thumbnail.is_none() { thumbnail = asset_thumbnail; }
            assets.push(asset);
        }
        Ok(json!({"assets":assets,"thumbnail":thumbnail,"model":job.params["model"]}))
    }

    pub fn execute(&self, job: &Job, cancel: &Arc<AtomicBool>) -> Result<Value> {
        let folder = self.output_folder(job)?;
        std::fs::create_dir_all(&folder)?;
        match job.method.as_str() {
            "cutflow.generate" => {
                if !["image", "video"].contains(&required(&job.params, "kind")?) {
                    return Err(Error::new("invalid_request", "kind 必须为 image 或 video"));
                }
                if job.params["maxGenerationCalls"].as_u64().unwrap_or(0) < 1 {
                    return Err(Error::new(
                        "budget_required",
                        "请明确允许的生成次数 maxGenerationCalls",
                    ));
                }
                if job.params["creationProjectId"].is_string() {
                    self.refresh_project_binding()?;
                    let instance = self.store.lock().unwrap().instance_id.clone();
                    if instance.as_deref() != job.params["gflow_instance_id"].as_str() {
                        return Err(Error::new("service_instance_changed", "服务实例已变化，禁止向另一套账号提交旧项目"));
                    }
                }
                let connection = self.store.lock().unwrap().connection.clone();
                let key = self.api_key()?;
                self.update(&job.id, "generating");
                self.persist()?;
                let count = job.params["n"].as_u64().unwrap_or(1);
                if !(1..=4).contains(&count) || (job.params["kind"] == "video" && count != 1) {
                    return Err(Error::new("invalid_request", "图片数量应为 1–4，视频每次一项"));
                }
                if job.params["maxGenerationCalls"].as_u64().unwrap_or(0) < count { return Err(Error::new("budget_required", "生成次数不足，请确认图片数量")); }
                let mut urls = Vec::new();
                for index in 0..count {
                    if cancel.load(Ordering::SeqCst) { return Err(Error::new("cancelled", "已停止生成，已完成的素材保留")); }
                    let mut request = job.params.clone();
                    request["client_request_id"] = json!(format!("{}-{index}", job.id));
                    let url = flow::generate(&connection, &key, &request, cancel, |message| {
                        let mut store = self.store.lock().unwrap();
                        store.revision = store.revision.saturating_add(1);
                        let revision = store.revision;
                        if let Some(current) = store.jobs.iter_mut().find(|current| current.id == job.id) { current.revision = revision; current.updated_at = creation::now(); current.stage = format!("generating: 第 {}/{} 张；{message}", index + 1, count); }
                    })?;
                    urls.push(url);
                    atomic_json(&folder.join("generation-result.json"), &json!({"urls":urls,"url":urls[0],"model":job.params["model"],"connection":connection}))?;
                    let partial = self.download_generation(job, &folder, cancel)?;
                    {
                        let mut store = self.store.lock().unwrap();
                        store.revision = store.revision.saturating_add(1);
                        let revision = store.revision;
                        if let Some(current) = store.jobs.iter_mut().find(|current| current.id == job.id) { current.result = Some(partial); current.revision = revision; current.updated_at = creation::now(); }
                    }
                    self.persist()?;
                }
                self.update(&job.id, "downloading");
                self.download_generation(job, &folder, cancel)
            }
            "cutflow.bilibili.download" => {
                self.update(&job.id, "downloading");
                let mut queue = cutflow_download::Queue::default();
                let id = queue
                    .enqueue(
                        required(&job.params, "url")?,
                        job.params["parts"].as_str().unwrap_or("1"),
                        job.params["quality"].as_str().unwrap_or("120"),
                        job.params["allowLower"].as_bool().unwrap_or(false),
                        PathBuf::from(required(&job.params, "projectPath")?),
                        required(&job.params, "rights")?,
                    )
                    .map_err(|message| Error::new("download_error", message))?;
                let download = queue
                    .jobs
                    .iter()
                    .find(|job| job.id == id)
                    .ok_or_else(|| Error::new("download_error", "任务创建失败"))?;
                let assets = self
                    .download_engine()
                    .download(download, cancel.clone())
                    .map_err(|message| Error::new("download_error", message))?;
                let mut results = Vec::new();
                for (index, asset) in assets.iter().enumerate() {
                    let target = folder.join(format!("source-{index}.mp4"));
                    std::fs::copy(&asset.path, &target)?;
                    results.push(self.probe(&target, cancel)?);
                }
                Ok(json!({"assets":results}))
            }
            _ => Err(Error::new("method_not_found", "任务类型未实现")),
        }
    }

    pub fn probe(&self, path: &Path, cancel: &AtomicBool) -> Result<Value> {
        let mut command = cutflow_download::process::command(&self.tools.join("ffprobe.exe"));
        command
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-of",
                "json",
            ])
            .arg(path);
        let bytes = cutflow_download::process::run(command, cancel, 60)
            .map_err(|message| Error::new("invalid_media", message))?;
        let info: Value = serde_json::from_slice(&bytes)?;
        let streams = info["streams"]
            .as_array()
            .ok_or_else(|| Error::new("invalid_media", "未发现媒体流"))?;
        let video = streams
            .iter()
            .find(|stream| stream["codec_type"] == "video");
        let audio = streams.iter().any(|stream| stream["codec_type"] == "audio");
        let duration = info["format"]["duration"]
            .as_str()
            .and_then(|duration| duration.parse::<f64>().ok());
        if video.is_none() && !audio
            || duration.is_some_and(|duration| !duration.is_finite() || duration <= 0.0)
        {
            return Err(Error::new("invalid_media", "媒体轨道或时长无效"));
        }
        let codec = video
            .and_then(|stream| stream["codec_name"].as_str())
            .unwrap_or("");
        let still_image = is_still_image(&info, codec, audio, duration);
        let duration = if still_image { None } else { duration };
        Ok(
            json!({"path":path,"duration":duration,"stillImage":still_image,"width":video.map(|video| &video["width"]),
            "height":video.map(|video| &video["height"]),"hasAudio":audio,"bytes":path.metadata()?.len()}),
        )
    }

    fn thumbnail_named(&self, source: &Path, folder: &Path, filename: &str, cancel: &AtomicBool) -> Option<PathBuf> {
        let target = folder.join(filename);
        if target.is_file() { return Some(target); }
        let mut command = cutflow_download::process::command(&self.tools.join("ffmpeg.exe"));
        command
            .args(["-v", "error", "-i"])
            .arg(source)
            .args(["-frames:v", "1", "-vf", "scale=240:-2"])
            .arg(&target);
        cutflow_download::process::run(command, cancel, 60)
            .ok()
            .map(|_| target)
    }
}


// ffprobe may assign one-frame duration to the image2 demuxer (including JPEG).
// Use the container as well as the codec so MJPEG videos remain videos.
fn is_still_image(info: &Value, codec: &str, audio: bool, duration: Option<f64>) -> bool {
    let image_container = info["format"]["format_name"].as_str().unwrap_or("")
        .split(',').any(|name| matches!(name, "image2" | "image2pipe" | "png_pipe" | "jpeg_pipe" | "webp_pipe" | "bmp_pipe"));
    !audio && matches!(codec, "png" | "mjpeg" | "webp" | "bmp" | "gif")
        && (duration.is_none() || image_container)
}

#[cfg(test)]
mod probe_regressions {
    use super::*;
    #[test]
    fn a_single_jpeg_with_ffprobe_frame_duration_is_an_image() {
        let info = json!({"format":{"format_name":"image2","duration":"0.040000"}});
        assert!(is_still_image(&info, "mjpeg", false, Some(0.04)));
        let video = json!({"format":{"format_name":"avi","duration":"0.040000"}});
        assert!(!is_still_image(&video, "mjpeg", false, Some(0.04)));
        assert!(!is_still_image(&info, "mjpeg", true, Some(0.04)));
    }
}
