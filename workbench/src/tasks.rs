use crate::{Error, Result, atomic_json, flow, required, secret};
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

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub version: u32,
    pub connection: flow::Connection,
    pub models: Vec<flow::Model>,
    pub references: Vec<Reference>,
    pub jobs: Vec<Job>,
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
                Ok(store) if store.version == 1 => (store, None),
                _ => (
                    Store::default(),
                    Some("任务存储损坏或版本不兼容，禁止覆盖；请先备份核查".into()),
                ),
            }
        } else {
            (
                Store {
                    version: 1,
                    ..Store::default()
                },
                None,
            )
        };
        for job in &mut store.jobs {
            if matches!(job.status.as_str(), "queued" | "running") {
                job.status = if job.stage.starts_with("generating") {
                    "result_unknown"
                } else {
                    "interrupted"
                }
                .into();
                job.stage = "程序已重启；请检查已完成素材，不会自动重发生成".into();
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
        atomic_json(
            &self.root.join("workbench.json"),
            &*self.store.lock().unwrap(),
        )
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

    fn job_view(&self, job: &Job) -> Value {
        let mut value = json!(job);
        value["canRetryDownload"] = json!(self.can_retry_download(job));
        value
    }

    pub fn snapshot(&self) -> Value {
        let store = self.store.lock().unwrap();
        json!({"connection":store.connection,"models":store.models,"references":store.references,
            "jobs":store.jobs.iter().map(|job| self.job_view(job)).collect::<Vec<_>>(),"fault":self.fault})
    }

    pub fn update(&self, id: &str, stage: &str) {
        {
            let mut store = self.store.lock().unwrap();
            if let Some(job) = store.jobs.iter_mut().find(|job| job.id == id) {
                job.stage = stage.into();
            }
        }
        if let Err(error) = self.persist() {
            if let Some(cancel) = self.cancels.lock().unwrap().get(id) {
                cancel.store(true, Ordering::SeqCst);
            }
            if let Some(job) = self
                .store
                .lock()
                .unwrap()
                .jobs
                .iter_mut()
                .find(|job| job.id == id)
            {
                job.stage = format!("任务检查点保存失败：{}", error.message);
            }
        }
    }

    pub fn finish(&self, id: &str, result: Result<Value>) {
        {
            let mut store = self.store.lock().unwrap();
            if let Some(job) = store.jobs.iter_mut().find(|job| job.id == id) {
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
            if let Some(job) = self
                .store
                .lock()
                .unwrap()
                .jobs
                .iter_mut()
                .find(|job| job.id == id)
            {
                job.status = "result_unknown".into();
                job.stage = format!("结果未持久化：{}；重启前请保存素材", error.message);
                job.error = Some(error);
            }
        }
    }

    pub fn reserve(&self, method: &str, params: Value) -> Result<(Job, bool, Arc<AtomicBool>)> {
        if self.fault.is_some() {
            self.persist()?;
        }
        let operation = required(&params, "operationId")?.to_string();
        if operation.len() > 128 {
            return Err(Error::new("invalid_request", "operationId 过长"));
        }
        let mut store = self.store.lock().unwrap();
        if let Some(previous) = store.jobs.iter().find(|job| job.operation_id == operation) {
            if previous.params != params || previous.method != method {
                return Err(Error::new(
                    "operation_conflict",
                    "operationId 已被其他请求使用",
                ));
            }
            return Ok((previous.clone(), false, Arc::new(AtomicBool::new(false))));
        }
        if self.cancels.lock().unwrap().len() >= 4 {
            return Err(Error::new("busy", "同时最多执行四项后台任务"));
        }
        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            operation_id: operation,
            method: method.into(),
            params,
            status: "running".into(),
            stage: "queued".into(),
            result: None,
            error: None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels
            .lock()
            .unwrap()
            .insert(job.id.clone(), cancel.clone());
        store.jobs.push(job.clone());
        if let Err(error) = atomic_json(&self.root.join("workbench.json"), &*store) {
            store.jobs.retain(|entry| entry.id != job.id);
            self.cancels.lock().unwrap().remove(&job.id);
            return Err(error);
        }
        drop(store);
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
                self.store.lock().unwrap().connection.base_url = base_url;
                self.persist()?;
                Ok(json!({"saved":true}))
            }
            "cutflow.models.list" => {
                let connection = self.store.lock().unwrap().connection.clone();
                let key = self.api_key()?;
                let models = flow::models(&connection, &key)?;
                self.store.lock().unwrap().models = models.clone();
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
                job.status = "running".into();
                job.stage = "downloading".into();
                job.error = None;
                *self.store.lock().unwrap().jobs.iter_mut().find(|item| item.id == id).unwrap() = job.clone();
                self.persist()?;
                let runtime = self.clone();
                std::thread::spawn(move || { let result = runtime.download_generation(&job, &folder, &cancel); runtime.finish(&job.id, result); });
                Ok(json!({"jobId":id,"status":"running"}))
            }
            "cutflow.jobs.list" => Ok(json!({"jobs":self.store.lock().unwrap().jobs.iter().map(|job| self.job_view(job)).collect::<Vec<_>>()})),
            "cutflow.jobs.get" => {
                let store = self.store.lock().unwrap();
                let job = store
                    .jobs
                    .iter()
                    .find(|job| Some(job.id.as_str()) == params["jobId"].as_str())
                    .ok_or_else(|| Error::new("not_found", "任务不存在"))?;
                Ok(self.job_view(job))
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

    fn api_key(&self) -> Result<String> {
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
        if let Some(path) = job.params["projectPath"].as_str().filter(|path| !path.is_empty()) {
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
            let asset = self.probe(&path, cancel)?;
            if (kind == "video" && (asset["duration"].as_f64().is_none() || asset["width"].as_u64().unwrap_or(0) == 0)) || (kind == "image" && !asset["stillImage"].as_bool().unwrap_or(false)) {
                return Err(Error::new("invalid_media", "生成结果类型与请求不符，文件已保留供核查"));
            }
            if thumbnail.is_none() { thumbnail = self.thumbnail(&path, folder, cancel); }
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
                        if let Some(current) = store.jobs.iter_mut().find(|current| current.id == job.id) { current.stage = format!("generating: 第 {}/{} 张；{message}", index + 1, count); }
                    })?;
                    urls.push(url);
                    atomic_json(&folder.join("generation-result.json"), &json!({"urls":urls,"url":urls[0],"model":job.params["model"],"connection":connection}))?;
                    let partial = self.download_generation(job, &folder, cancel)?;
                    if let Some(current) = self.store.lock().unwrap().jobs.iter_mut().find(|current| current.id == job.id) { current.result = Some(partial); }
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
        let still_image =
            matches!(codec, "png" | "mjpeg" | "webp" | "bmp" | "gif") && duration.is_none();
        Ok(
            json!({"path":path,"duration":duration,"stillImage":still_image,"width":video.map(|video| &video["width"]),
            "height":video.map(|video| &video["height"]),"hasAudio":audio,"bytes":path.metadata()?.len()}),
        )
    }

    fn thumbnail(&self, source: &Path, folder: &Path, cancel: &AtomicBool) -> Option<PathBuf> {
        let target = folder.join("thumbnail.jpg");
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
