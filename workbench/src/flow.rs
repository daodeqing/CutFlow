use crate::{Error, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub base_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub description: String,
    pub kind: String,
    pub max_references: usize,
    pub duration: Option<f64>,
    pub aspect: Option<String>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub aspects: Option<Vec<String>>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub image_sizes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub durations: Option<Vec<f64>>,
}

pub fn describe(id: &str, description: &str) -> Model {
    let normalized = id.to_ascii_lowercase();
    let image = normalized.contains("image") || normalized.contains("imagen") || normalized.contains("banana");
    let text = normalized == "codex-auto-review" || !image && ["gpt-", "api-6", "gemini-", "claude-", "deepseek", "qwen"].iter().any(|prefix| normalized.starts_with(prefix));
    let video = ["veo", "video", "t2v", "i2v", "r2v", "sora", "kling", "omni-1.1"].iter().any(|word| normalized.contains(word));
    let references = if image {
        3
    } else if id.contains("r2v") || id.contains("omni") {
        3
    } else if id.contains("i2v") || id.contains("interpolation") {
        2
    } else if id.contains("t2v") {
        0
    } else {
        3
    };
    let duration = regex::Regex::new(r"(?:^|[-_])(4|6|8|10)s(?:[-_]|$)")
        .unwrap()
        .captures(id)
        .and_then(|captures| captures[1].parse().ok());
    Model {
        id: id.into(),
        description: description.into(),
        kind: if image { "image" } else if video { "video" } else if text { "text" } else { "unknown" }.into(),
        max_references: references,
        duration,
        aspects: None,
        image_sizes: None,
        durations: None,
        aspect: if id.contains("portrait") {
            Some("9:16".into())
        } else if id.contains("landscape") {
            Some("16:9".into())
        } else if id.contains("square") {
            Some("1:1".into())
        } else {
            None
        },
    }
}

pub fn base_url(value: &str) -> Result<String> {
    let value = value.trim().trim_end_matches('/');
    let parsed =
        url::Url::parse(value).map_err(|_| Error::new("invalid_request", "服务地址无效"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || value.contains('@')
        || value.contains('?')
        || value.contains('#')
    {
        return Err(Error::new(
            "invalid_request",
            "请填写 HTTP(S) 服务地址，不含凭据、查询参数或片段",
        ));
    }
    Ok(if value.ends_with("/v1") {
        value.into()
    } else {
        format!("{value}/v1")
    })
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .timeout(Duration::from_secs(1800))
        .build()
}

fn http_error(error: ureq::Error) -> Error {
    match error {
        ureq::Error::Status(status, _) => Error::new(
            match status {
                401 | 403 => "provider_unauthorized",
                402 => "insufficient_credit",
                429 => "rate_limited",
                _ => "provider_error",
            },
            format!("生成服务返回 HTTP {status}，请检查服务账号、权限或额度"),
        ),
        _ => Error::new(
            "result_unknown",
            "连接中断或超时；生成结果需核查，请勿直接重发付费任务",
        ),
    }
}

pub fn models(connection: &Connection, key: &str) -> Result<Vec<Model>> {
    let response = agent()
        .get(&format!("{}/models", base_url(&connection.base_url)?))
        .set("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(http_error)?;
    let payload: Value = serde_json::from_reader(response.into_reader().take(2_000_000))?;
    let rows = payload["data"]
        .as_array()
        .ok_or_else(|| Error::new("provider_error", "模型列表格式无效"))?;
    Ok(rows
        .iter()
        .filter_map(|item| {
            let id = item["id"].as_str()?;
            if id == "gflow-project-control" { return None; }
            let mut model = describe(id, item["description"].as_str().unwrap_or(""));
            apply_model_metadata(&mut model, item);
            Some(model)
        })
        .collect())
}

pub fn apply_model_metadata(model: &mut Model, value: &Value) {
    if let Some(kind) = value["kind"].as_str().filter(|s| matches!(*s, "image" | "video" | "text" | "unknown")) { model.kind = kind.into(); }
    if let Some(max) = value["maxReferences"].as_u64() { model.max_references = max.min(32) as usize; }
    model.aspects = value["aspects"].as_array().map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect()).or(model.aspects.take());
    model.image_sizes = value["imageSizes"].as_array().map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect()).or(model.image_sizes.take());
    model.durations = value["durations"].as_array().map(|items| items.iter().filter_map(Value::as_f64).filter(|n| n.is_finite() && *n > 0.0).collect()).or(model.durations.take());
}

fn control_http_error(error: ureq::Error) -> Error {
    if let ureq::Error::Status(status, response) = error {
        if let Ok(payload) = serde_json::from_reader::<_, Value>(response.into_reader().take(1_000_000)) {
            if payload["error"].is_object() {
                return Error::new(payload["error"]["code"].as_str().unwrap_or("provider_error"), provider_message(&payload));
            }
        }
        Error::new(match status { 401 | 403 => "provider_unauthorized", 402 => "insufficient_credit", 429 => "rate_limited", _ => "provider_error" },
            format!("项目控制服务返回 HTTP {status}，请检查插件与账号状态"))
    } else { http_error(error) }
}

/// Dedicated, non-media Chat Completions control request. It never uses generation parsing.
pub fn project_control(connection: &Connection, key: &str, control: &Value) -> Result<Value> {
    let response = ureq::AgentBuilder::new().redirects(0)
        .timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(120))
        .timeout(Duration::from_secs(180)).build()
        .post(&format!("{}/chat/completions", base_url(&connection.base_url)?))
        .set("Authorization", &format!("Bearer {key}"))
        .send_json(json!({"model":"gflow-project-control","stream":false,
            "messages":[{"role":"user","content":control.to_string()}]}))
        .map_err(control_http_error)?;
    let payload: Value = serde_json::from_reader(response.into_reader().take(2_000_000))
        .map_err(|_| Error::new("result_unknown", "项目控制响应无效，请核查原操作结果"))?;
    if payload.get("error").is_some() { return Err(Error::new(
        payload["error"]["code"].as_str().unwrap_or("provider_error"), provider_message(&payload))); }
    let content = payload["choices"][0]["message"]["content"].as_str()
        .ok_or_else(|| Error::new("project_binding_unavailable", "服务不支持项目控制协议，请更新 GFlow 插件"))?;
    let value: Value = serde_json::from_str(content)
        .map_err(|_| Error::new("result_unknown", "项目控制内容无效，请核查原操作结果"))?;
    if value["version"] != 1 { return Err(Error::new("project_binding_unavailable", "项目控制协议版本不兼容")); }
    if value.get("error").is_some() { return Err(Error::new(
        value["error"]["code"].as_str().unwrap_or("provider_error"), provider_message(&value))); }
    Ok(value)
}

pub fn reference_data(path: &Path) -> Result<String> {
    let metadata = path.metadata()?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err(Error::new("invalid_request", "参考图不能超过 20 MiB"));
    }
    let bytes = std::fs::read(path)?;
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        return Err(Error::new(
            "invalid_request",
            "参考图只支持 PNG、JPEG 或 WebP",
        ));
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdate {
    pub stage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    pub basis: String,
}

impl ProgressUpdate {
    pub fn stage(stage: impl Into<String>, percent: Option<u8>) -> Self {
        Self { stage: stage.into(), percent, basis: "stage".into() }
    }
}

fn safe_progress_text(value: &Value) -> Option<String> {
    let raw = value.as_str()?.trim();
    if raw.is_empty() || raw.len() > 512 { return None; }
    let lower = raw.to_ascii_lowercase();
    // Only short status text may be persisted; media and credentials are not progress.
    if lower.contains("data:") || lower.contains("base64") || lower.contains("bearer ") || lower.contains("sk-")
        || lower.contains("http://") || lower.contains("https://")
        || ["api_key", "apikey", "access_token", "authorization", "cookie", "password", "credential"].iter().any(|word| lower.contains(*word))
        || raw.split_whitespace().any(|word| word.len() > 100 && word.bytes().all(|c| c.is_ascii_alphanumeric() || b"+/=_-".contains(&c))) {
        return None;
    }
    let text: String = raw.chars().filter(|character| !character.is_control()).take(120).collect();
    (!text.is_empty()).then_some(text)
}

fn progress_stage_text(value: &Value) -> Option<String> {
    let stage = safe_progress_text(value)?;
    Some(match stage.as_str() {
        "queued" => "等待生成".into(),
        "started" => "准备生成".into(),
        "token_selected" => "已选择项目账号".into(),
        "token_ready" => "项目账号已就绪".into(),
        "project_ready" => "Flow 项目已就绪".into(),
        "preparing_video" => "正在准备视频".into(),
        "uploading_images" => "正在上传参考图".into(),
        "submitting_image" => "正在提交图片生成".into(),
        "solving_image_captcha" => "正在完成图片验证".into(),
        "video_submitted" => "视频任务已提交".into(),
        "video_polling" => "等待视频生成".into(),
        "image_generated" => "图片已生成".into(),
        "upsampling_2k" => "正在放大至 2K".into(),
        "upsampling_4k" => "正在放大至 4K".into(),
        "caching_image" => "正在缓存原图".into(),
        "caching_video" => "正在缓存视频".into(),
        "submitting" => "正在提交生成请求".into(),
        "generating" => "正在生成".into(),
        "upscaling" => "正在放大图片".into(),
        "downloading" => "正在下载原图".into(),
        "completed" => "生成结果已返回".into(),
        _ => stage,
    })
}

fn progress_from_value(value: &Value) -> Option<ProgressUpdate> {
    if value.get("basis").and_then(Value::as_str) != Some("stage") { return None; }
    let stage = progress_stage_text(value.get("stage")?)?;
    let percent = value.get("percent").and_then(Value::as_u64)
        .filter(|value| *value <= 100).map(|value| value as u8);
    Some(ProgressUpdate::stage(stage, percent))
}

fn progress_from_payload(payload: &Value) -> Option<ProgressUpdate> {
    let choice = payload["choices"].as_array().and_then(|choices| choices.first());
    let message = choice.and_then(|choice| choice.get("delta"))
        .or_else(|| choice.and_then(|choice| choice.get("message")));
    payload.get("gflow_progress").and_then(progress_from_value)
        .or_else(|| message.and_then(|message| message.get("gflow_progress")).and_then(progress_from_value))
        .or_else(|| message.and_then(|message| message.get("reasoning_content"))
            .and_then(|text| progress_stage_text(text).map(|stage| ProgressUpdate::stage(stage, None))))
}

#[derive(Default)]
pub struct StreamResult {
    pub content: String,
    pub url: Option<String>,
    pub done: bool,
    pub control: Option<Value>,
    pub progress: Option<ProgressUpdate>,
    media_id: Option<String>,
    media_mime: Option<String>,
    media_base64: String,
}

impl StreamResult {
    pub fn event(&mut self, data: &str) -> Result<()> {
        let mut data = data.trim();
        while let Some(inner) = data.strip_prefix("data:") { data = inner.trim(); }
        if data.is_empty() { return Ok(()); }
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let payload: Value = serde_json::from_str(data)
            .map_err(|_| Error::new("result_unknown", "生成服务返回无效消息，结果待核查"))?;
        if let Some(media) = payload.get("gflow_media") {
            self.media_chunk(media)?;
            return Ok(());
        }
        if let Some(control) = payload.get("gflow_control") { self.control = Some(control.clone()); }
        if let Some(progress) = progress_from_payload(&payload) { self.progress = Some(progress); }
        if payload.get("error").is_some() {
            return Err(Error::new(
                match payload["error"]["code"].as_str().unwrap_or("") {
                    "insufficient_quota" | "insufficient_credit" => "insufficient_credit",
                    "rate_limit_exceeded" => "rate_limited",
                    "cancelled" => "cancelled",
                    _ => "provider_error",
                },
                provider_message(&payload),
            ));
        }
        if payload["choices"][0]["finish_reason"].as_str().is_some_and(|reason| !reason.is_empty()) {
            self.done = true;
        }
        if let Some(url) = payload["data"][0]["url"].as_str() { self.url = Some(url.into()); }
        if let Some(data) = payload["data"][0]["b64_json"].as_str() { self.url = Some(format!("data:image/png;base64,{data}")); }
        if let Some(url) = payload["url"].as_str() {
            self.url = Some(url.into());
        }
        let message = payload["choices"][0].get("delta").or_else(|| payload["choices"][0].get("message"));
        let content = message.and_then(|message| message.get("content"));
        let part = content.and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| {
            content.and_then(Value::as_array).map(|parts| parts.iter().filter_map(|part| part["text"].as_str()).collect::<String>()).unwrap_or_default()
        });
        if let Some(parts) = content.and_then(Value::as_array) {
            for part in parts {
                if let Some(url) = part["image_url"]["url"].as_str().or_else(|| part["video_url"]["url"].as_str()) {
                    self.url = Some(url.into());
                }
            }
        }
        if let Some(url) = message.and_then(|message| message["images"][0]["image_url"]["url"].as_str()) {
            self.url = Some(url.into());
        }
        if self.content.len() + part.len() > 32 * 1024 * 1024 {
            return Err(Error::new("provider_error", "生成服务响应超过安全长度"));
        }
        self.content.push_str(&part);
        Ok(())
    }

    fn media_chunk(&mut self, media: &Value) -> Result<()> {
        let id = media.get("id").and_then(Value::as_str).unwrap_or("");
        let mime = media.get("mime").and_then(Value::as_str).unwrap_or("");
        if media.get("encoding").and_then(Value::as_str) != Some("base64") {
            return Err(Error::new("invalid_media", "生成服务返回了不支持的媒体编码"));
        }
        let valid_id = valid_cached_media_id(id);
        if !valid_id || !matches!(mime, "image/png" | "image/jpeg" | "image/webp" | "video/mp4" | "video/webm" | "video/quicktime") {
            return Err(Error::new("invalid_media", "生成服务返回了无效的媒体分片"));
        }
        if self.media_id.as_deref().is_some_and(|current| current != id) {
            return Err(Error::new("invalid_media", "生成服务返回了多个媒体分片"));
        }
        if self.media_mime.as_deref().is_some_and(|current| current != mime) {
            return Err(Error::new("invalid_media", "生成服务返回了不一致的媒体类型"));
        }
        let chunk = media.get("chunk").and_then(Value::as_str).unwrap_or("");
        if chunk.len() > 100_000 || !chunk.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=')) {
            return Err(Error::new("invalid_media", "生成服务返回了无效的媒体编码"));
        }
        if self.media_base64.len() + chunk.len() > 180 * 1024 * 1024 {
            return Err(Error::new("provider_error", "生成媒体超过安全长度"));
        }
        self.media_id = Some(id.to_owned());
        self.media_mime = Some(mime.to_owned());
        self.media_base64.push_str(chunk);
        if media.get("done").and_then(Value::as_bool) == Some(true) {
            if self.media_base64.is_empty()
                || base64::engine::general_purpose::STANDARD.decode(&self.media_base64).is_err()
            {
                return Err(Error::new("invalid_media", "生成服务返回了不完整的媒体分片"));
            }
            self.url = Some(format!("data:{};base64,{}", mime, self.media_base64));
        }
        Ok(())
    }

    pub fn media_url(&self) -> Result<String> {
        if let Some(url) = &self.url {
            return Ok(url.clone());
        }
        for pattern in [
            r#"!\[[^\]]*\]\(([^\s)]+)\)"#,
            r#"<video\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
            r#"<img\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
            r#"(?:https?://[^\s<>"')]+\.(?:mp4|webm|mov|png|jpe?g|webp)(?:\?[^\s<>"')]+)?)"#,
        ] {
            if let Some(found) = regex::Regex::new(pattern).unwrap().captures(&self.content) {
                return Ok(found.get(1).unwrap_or_else(|| found.get(0).unwrap()).as_str().replace("&amp;", "&"));
            }
        }
        Err(Error::new(
            "result_unknown",
            "未收到最终媒体地址，请先核查服务端生成历史",
        ))
    }
}

fn valid_cached_media_id(id: &str) -> bool {
    let Some((stem, extension)) = id.rsplit_once('.') else { return false; };
    if !matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "mp4" | "webm" | "mov") { return false; }
    let base = stem.strip_suffix("_2K").or_else(|| stem.strip_suffix("_4K")).unwrap_or(stem);
    base.len() == 32 && base.bytes().all(|c| c.is_ascii_hexdigit())
}

async fn cancellable<F: std::future::Future>(future: F, cancel: &AtomicBool) -> Result<F::Output> {
    let mut future = std::pin::pin!(future);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(Error::new("cancelled", "已停止生成请求；已提交云端的任务是否扣费以服务端为准"));
        }
        if let Ok(result) = tokio::time::timeout(Duration::from_millis(100), future.as_mut()).await {
            return Ok(result);
        }
    }
}

fn provider_message(payload: &Value) -> String {
    let text = payload["error"]["message"].as_str()
        .or_else(|| payload["detail"].as_str()).unwrap_or("服务未返回错误详情");
    let secret = regex::Regex::new(r"(?i)(?:bearer\s+|sk-)[a-z0-9_.-]+|(?:https?://)[^\s]+[?][^\s]+").unwrap();
    secret.replace_all(text, "[REDACTED_SECRET]").chars().take(600).collect()
}

pub fn check_response(connection: &Connection, key: &str, model: &str) -> Result<Value> {
    if describe(model, "").kind != "text" {
        return Err(Error::new("invalid_request", "请选择普通文本模型测试回复；图片和视频请在创作页测试"));
    }
    let response = agent().post(&format!("{}/chat/completions", base_url(&connection.base_url)?))
        .set("Authorization", &format!("Bearer {key}"))
        .send_json(json!({"model":model,"messages":[{"role":"user","content":"只回复 OK"}],"stream":false,"max_tokens":16}))
        .map_err(http_error)?;
    let payload: Value = serde_json::from_reader(response.into_reader().take(1_000_000))?;
    let answer = payload["choices"][0]["message"]["content"].as_str().unwrap_or("");
    if answer.trim().is_empty() { return Err(Error::new("provider_error", "模型列表可读，但没有收到文本回复")); }
    Ok(json!({"model":model,"answer":answer.chars().take(200).collect::<String>(),"verified":true}))
}

pub fn generate(
    connection: &Connection, key: &str, request: &Value, cancel: &AtomicBool,
    mut progress: impl FnMut(ProgressUpdate),
) -> Result<String> {
    let model = crate::required(request, "model")?;
    let prompt = crate::required(request, "prompt")?;
    let capability = describe(model, "");
    if capability.kind == "text" || model == "gflow-project-control" {
        return Err(Error::new("invalid_request", "这是文本模型，请选择图片或视频生成模型"));
    }
    let references = request["references"].as_array().cloned().unwrap_or_default();
    if references.len() > capability.max_references {
        return Err(Error::new("invalid_request", "所选模型不支持此数量的参考图"));
    }
    let reference_paths = references.clone();
    let mut content = vec![json!({"type":"text","text":prompt})];
    for reference in references {
        content.push(json!({"type":"image_url","image_url":{"url":reference_data(Path::new(reference.as_str().ok_or_else(|| Error::new("invalid_request", "参考图必须是本地路径"))?))?}}));
    }
    let mut payload = json!({"model":model,"messages":[{"role":"user","content":content}],"stream":request["stream"].as_bool().unwrap_or(true)});
    if model.starts_with("gflow-") && url::Url::parse(&connection.base_url).is_ok_and(|url| matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))) { payload["client_process_id"] = json!(std::process::id()); }
    for field in ["aspect_ratio", "duration", "quality", "image_size", "client_request_id", "gflow_workspace_id", "gflow_project_id", "gflow_instance_id"] {
        if let Some(value) = request.get(field).filter(|value| !value.is_null() && value.as_str() != Some("auto")) { payload[field] = value.clone(); }
    }
    if model.starts_with("gflow-") || model.contains("gemini") || model.contains("banana") {
        let mut image_config = json!({});
        if let Some(aspect) = request["aspect_ratio"].as_str().filter(|s| *s != "auto") { image_config["aspectRatio"] = json!(aspect); }
        if let Some(size) = request["image_size"].as_str().filter(|s| *s != "auto") { image_config["imageSize"] = json!(size); }
        payload["generationConfig"] = json!({"imageConfig":image_config});
    }
    let image_api = model.starts_with("gpt-image-");
    let mut endpoint = "chat/completions";
    if image_api {
        endpoint = if reference_paths.is_empty() { "images/generations" } else { "images/edits" };
        payload = json!({"model":model,"prompt":prompt,"n":1});
        let size = match request["aspect_ratio"].as_str().unwrap_or("auto") {"1:1"=>"1024x1024", "3:2"=>"1536x1024", "2:3"=>"1024x1536", "auto"=>"auto", _=>return Err(Error::new("invalid_request", "此图片模型支持 1:1、3:2、2:3 比例"))};
        payload["size"] = json!(size);
        let quality = match request["image_size"].as_str().unwrap_or("auto") {"1K"|"low"=>"low", "2K"|"medium"=>"medium", "4K"|"high"=>"high", _=>"auto"};
        payload["quality"] = json!(quality);
    }
    let runtime = tokio::runtime::Runtime::new().map_err(|e| Error::new("io_error", e.to_string()))?;
    let mut control: Option<Value> = None;
    let outcome = runtime.block_on(async {
        let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(1800)).build()
            .map_err(|_| Error::new("io_error", "HTTP 客户端初始化失败"))?;
        let mut outgoing = client.post(format!("{}/{endpoint}", base_url(&connection.base_url)?)).bearer_auth(key);
        if image_api && !reference_paths.is_empty() {
            let mut form = reqwest::multipart::Form::new().text("model", model.to_string()).text("prompt", prompt.to_string()).text("n", "1");
            for field in ["size", "quality"] { form = form.text(field, payload[field].as_str().unwrap_or("auto").to_string()); }
            for (index, value) in reference_paths.iter().enumerate() {
                let path = Path::new(value.as_str().ok_or_else(|| Error::new("invalid_request", "参考图路径无效"))?);
                let data = reference_data(path)?;
                let mime = data.split(';').next().unwrap_or("data:image/png").trim_start_matches("data:");
                let extension = match mime {"image/jpeg"=>"jpg", "image/webp"=>"webp", _=>"png"};
                let part = reqwest::multipart::Part::bytes(std::fs::read(path)?).file_name(format!("reference-{index}.{extension}")).mime_str(mime).map_err(|_| Error::new("invalid_request", "参考图格式无效"))?;
                form = form.part("image[]", part);
            }
            outgoing = outgoing.multipart(form);
        } else { outgoing = outgoing.json(&payload); }
        let mut response = cancellable(outgoing.send(), cancel).await?
            .map_err(|_| Error::new("result_unknown", "连接中断或超时；请核查服务端历史，不会自动重发"))?;
        let status = response.status().as_u16();
        let streamed = response.headers().get("content-type").and_then(|h| h.to_str().ok()).unwrap_or("").contains("text/event-stream");
        let mut buffer = Vec::new();
        let mut total = 0usize;
        let mut result = StreamResult::default();
        while let Some(chunk) = cancellable(response.chunk(), cancel).await?
            .map_err(|_| Error::new("result_unknown", "响应中断，生成结果待核查"))? {
            total += chunk.len();
            if total > 40 * 1024 * 1024 { return Err(Error::new("provider_error", "响应超过 40 MiB")); }
            buffer.extend_from_slice(&chunk);
            if streamed && status < 400 {
                while let Some((end, separator)) = buffer.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2))
                    .or_else(|| buffer.windows(4).position(|w| w == b"\r\n\r\n").map(|i| (i, 4))) {
                    let event = String::from_utf8_lossy(&buffer[..end]);
                    let data = event.lines().filter_map(|line| line.strip_prefix("data:").map(str::trim_start)).collect::<Vec<_>>().join("\n");
                    if !data.is_empty() {
                        let previous_progress = result.progress.clone();
                        result.event(&data).map_err(|mut error| { error.message = error.message.replace(key, "[REDACTED_SECRET]"); error })?;
                        if result.control.is_some() { control = result.control.clone(); }
                        if result.progress != previous_progress {
                            if let Some(mut update) = result.progress.clone() {
                                update.stage = update.stage.replace(key, "[REDACTED_SECRET]");
                                progress(update);
                            }
                        }
                    }
                    buffer.drain(..end + separator);
                    if result.done { break; }
                }
                if result.done { break; }
            }
        }
        if status >= 400 {
            let body = serde_json::from_slice::<Value>(&buffer).unwrap_or_default();
            return Err(Error::new(match status {401|403=>"provider_unauthorized",402=>"insufficient_credit",429=>"rate_limited",_=>"provider_error"},
                format!("HTTP {status}：{}", provider_message(&body).replace(key, "[REDACTED_SECRET]"))));
        }
        if streamed {
            if !result.done { return Err(Error::new("result_unknown", "流式响应提前结束；请核查服务端历史，不会自动重发")); }
        } else {
            let body: Value = serde_json::from_slice(&buffer).map_err(|_| Error::new("result_unknown", "响应格式无效，结果待核查"))?;
            result.event(&body.to_string())?;
            if let Some(mut update) = result.progress.clone() {
                update.stage = update.stage.replace(key, "[REDACTED_SECRET]");
                progress(update);
            }
        }
        result.media_url()
    });
    if cancel.load(Ordering::SeqCst) {
        let mut pipe_ack = false;
        #[cfg(windows)]
        if model.starts_with("gflow-") && url::Url::parse(&connection.base_url).is_ok_and(|url| matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))) {
            if let Some(id) = request["client_request_id"].as_str().filter(|id| id.len() <= 40 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')) {
                let name = format!(r"\\.\pipe\gflow-cancel-{id}");
                if let Ok(mut pipe) = std::fs::OpenOptions::new().read(true).write(true).open(name) {
                    pipe_ack = pipe.write_all(b"cancel\n").is_ok();
                }
            }
        }
        if !pipe_ack && control.is_none() && model.starts_with("gflow-") {
            if let Some(id) = request["client_request_id"].as_str().filter(|id| id.len() <= 40 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')) {
                if let Ok(base) = url::Url::parse(&connection.base_url) {
                    if matches!(base.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
                        if let Ok(address) = base.join(&format!("/v0/resource/plugins/gflow/tmp/control-{id}.json")) {
                            if let Ok(response) = ureq::get(address.as_str()).timeout(Duration::from_secs(2)).call() {
                                control = serde_json::from_reader(response.into_reader().take(4096)).ok();
                            }
                        }
                    }
                }
            }
        }
        if let Some(control) = control.filter(|_| !pipe_ack).filter(|_| url::Url::parse(&connection.base_url).is_ok_and(|url| matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))) {
            if let (Some(address), Some(token)) = (control["url"].as_str(), control["token"].as_str()) {
                if url::Url::parse(address).is_ok_and(|url| url.scheme() == "http" && url.host_str() == Some("127.0.0.1") && url.path() == "/cancel" && url.username().is_empty() && url.password().is_none()) {
                    let _ = ureq::post(address).timeout(Duration::from_secs(3)).set("Authorization", &format!("Bearer {token}")).send_string("");
                }
            }
        }
    }
    outcome.map_err(|mut error| { error.message = error.message.replace(key, "[REDACTED_SECRET]"); error })
}

pub fn download(
    connection: &Connection,
    url: &str,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    if url.starts_with("data:image/") {
        let (header, encoded) = url.split_once(",").ok_or_else(|| Error::new("invalid_media", "内嵌图片格式无效"))?;
        let extension = match header { "data:image/png;base64"=>"png", "data:image/jpeg;base64"=>"jpg", "data:image/webp;base64"=>"webp", _=>return Err(Error::new("invalid_media", "内嵌图片类型不支持")) };
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_| Error::new("invalid_media", "内嵌图片编码无效"))?;
        if bytes.is_empty() || bytes.len() > 32*1024*1024 { return Err(Error::new("invalid_media", "图片为空或超过 32 MiB")); }
        if cancel.load(Ordering::SeqCst) { return Err(Error::new("cancelled", "图片保存已取消")); }
        reference_magic(&bytes)?;
        let target = destination.with_extension(extension);
        std::fs::create_dir_all(target.parent().ok_or_else(|| Error::new("invalid_media", "目标路径无效"))?)?;
        std::fs::write(&target, bytes)?;
        return Ok(target);
    }
    let base = base_url(&connection.base_url)?;
    let base_uri =
        url::Url::parse(&base).map_err(|_| Error::new("invalid_request", "服务地址无效"))?;
    let mut target = base_uri
        .join(url)
        .map_err(|_| Error::new("invalid_media", "媒体地址无效"))?
        .to_string();
    for _attempt in 0..6 {
        let parsed =
            url::Url::parse(&target).map_err(|_| Error::new("invalid_media", "媒体地址无效"))?;
        let host = parsed.host_str().unwrap_or("");
        let same_host = parsed.origin() == base_uri.origin();
        let private = host == "localhost"
            || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                std::net::IpAddr::V4(ip) => {
                    ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
                }
                std::net::IpAddr::V6(ip) => {
                    ip.is_loopback()
                        || ip.is_unspecified()
                        || ip.is_unique_local()
                        || ip.is_unicast_link_local()
                }
            });
        if !matches!(parsed.scheme(), "http" | "https")
            || host.is_empty()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || (private && !same_host)
        {
            return Err(Error::new("invalid_media", "拒绝不安全的媒体地址"));
        }
        use std::net::ToSocketAddrs;
        let resolved = (
            host.trim_matches(['[', ']']),
            parsed.port_or_known_default().unwrap_or(443),
        )
            .to_socket_addrs()
            .map_err(|_| Error::new("download_error", "媒体服务器地址无法解析"))?
            .collect::<Vec<_>>();
        if !same_host && resolved.iter().any(|address| private_ip(address.ip())) {
            return Err(Error::new("invalid_media", "拒绝指向本地网络的媒体地址"));
        }
        let media_agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .timeout(Duration::from_secs(1800))
            .resolver(move |_netloc: &str| Ok(resolved.clone()))
            .build();
        let response = media_agent.get(&target).call().map_err(|error| {
            let detail = match error {
                ureq::Error::Status(status, _) => format!("HTTP {status}"),
                _ => "连接中断或超时".into(),
            };
            Error::new("download_error", format!("生成已成功，但原图下载失败（{detail}）；可只重试下载，无需重新生成"))
        })?;
        if (300..400).contains(&response.status()) {
            target = parsed
                .join(
                    response
                        .header("Location")
                        .ok_or_else(|| Error::new("invalid_media", "重定向无目标"))?,
                )
                .map_err(|_| Error::new("invalid_media", "重定向无效"))?
                .to_string();
            continue;
        }
        if response
            .header("Content-Type")
            .is_some_and(|value| value.contains("text/") || value.contains("json"))
        {
            return Err(Error::new("invalid_media", "媒体地址返回了文字或错误页"));
        }
        let destination = match response.header("Content-Type").unwrap_or("").split(';').next().unwrap_or("").trim() {
            "image/png" => destination.with_extension("png"),
            "image/jpeg" => destination.with_extension("jpg"),
            "image/webp" => destination.with_extension("webp"),
            "video/webm" => destination.with_extension("webm"),
            "video/mp4" => destination.with_extension("mp4"),
            "video/quicktime" => destination.with_extension("mov"),
            _ => destination.to_path_buf(),
        };
        std::fs::create_dir_all(
            destination
                .parent()
                .ok_or_else(|| Error::new("invalid_media", "目标路径无效"))?,
        )?;
        let partial = destination.with_extension("part");
        let mut output = File::create(&partial)?;
        let mut input = response.into_reader().take(2 * 1024 * 1024 * 1024 + 1);
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(Error::new(
                    "cancelled",
                    "已取消媒体下载，生成结果未自动重发",
                ));
            }
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > 2 * 1024 * 1024 * 1024 {
                return Err(Error::new("invalid_media", "生成媒体超过 2 GiB"));
            }
            output.write_all(&buffer[..count])?;
        }
        if total == 0 {
            return Err(Error::new("invalid_media", "媒体文件为空"));
        }
        output.sync_all()?;
        drop(output);
        std::fs::rename(partial, &destination)?;
        return Ok(destination.to_path_buf());
    }
    Err(Error::new("invalid_media", "媒体重定向过多"))
}

fn private_ip(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_multicast()
        }
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || ip.to_ipv4_mapped().is_some_and(|ip| private_ip(ip.into()))
        }
    }
}

fn reference_magic(bytes: &[u8]) -> Result<()> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(b"\xff\xd8\xff") || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")) { Ok(()) }
    else { Err(Error::new("invalid_media", "返回的内容不是有效图片")) }
}
