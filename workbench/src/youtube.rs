//! YouTube Studio handoff through the user's existing Chrome profile.
//!
//! The companion extension performs the browser-only file selection and field
//! updates. CutFlow only exposes a short-lived, loopback bridge bound to one
//! job and one random token, then waits for the extension's review-ready ack.
use crate::{Error, Result, required, tasks::{Job, Runtime}};
use serde_json::{Value, json};
use std::{io::{BufRead, BufReader, Read, Write}, net::{TcpListener, TcpStream}, path::Path, sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

struct BridgeState {
    port: u16,
    token: String,
    job_id: String,
    video_path: String,
    title: String,
    description: String,
    finished: Mutex<Option<Result<()>>>,
    stop: AtomicBool,
}
static BRIDGES: OnceLock<Mutex<std::collections::BTreeMap<String, Arc<BridgeState>>>> = OnceLock::new();
fn bridges() -> &'static Mutex<std::collections::BTreeMap<String, Arc<BridgeState>>> { BRIDGES.get_or_init(|| Mutex::new(std::collections::BTreeMap::new())) }
fn failure(message: impl Into<String>) -> Error { Error::new("youtube_handoff", message) }
fn cancelled(cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
    if cancel.load(std::sync::atomic::Ordering::SeqCst) { Err(Error::new("cancelled", "已停止 YouTube 信息带入；请在 Studio 核查窗口状态")) } else { Ok(()) }
}
fn valid_title(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().count() <= 100 && value.chars().any(|c| c.is_ascii_alphabetic())
        && !value.chars().any(|c| c.is_control() || matches!(c as u32, 0x3400..=0x9fff | 0xf900..=0xfaff | 0x3040..=0x30ff | 0xac00..=0xd7af | 0x20000..=0x323af))
}
pub fn validate(params: &Value) -> Result<()> {
    let title = required(params, "title")?;
    if !valid_title(title) { return Err(Error::new("invalid_request", "请填写 100 字符以内的英文标题")); }
    let description = required(params, "description")?;
    if description.chars().count() > 5000 { return Err(Error::new("invalid_request", "YouTube 简介不能超过 5000 个字符")); }
    let channel = url::Url::parse(required(params, "channelUrl")?).map_err(|_| Error::new("invalid_request", "频道网址无效"))?;
    if channel.scheme() != "https" || channel.host_str() != Some("www.youtube.com") || channel.port().is_some()
        || !channel.username().is_empty() || channel.password().is_some() || channel.path() != "/@deqingdao/videos" || channel.query().is_some() || channel.fragment().is_some() {
        return Err(Error::new("invalid_request", "请使用已确认的 deqingdao 频道网址"));
    }
    let path = Path::new(required(params, "videoPath")?);
    if !path.is_file() || path.metadata()?.len() == 0 { return Err(Error::new("invalid_media", "成片文件不存在或为空")); }
    Ok(())
}
pub struct Launch { pub url: String }

pub fn open_current_chrome(address: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let executable = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"].into_iter()
            .filter_map(std::env::var_os).map(|folder| std::path::PathBuf::from(folder).join("Google/Chrome/Application/chrome.exe"))
            .find(|path| path.is_file()).ok_or_else(|| failure("未找到 Chrome；请安装 Chrome 并加载 CutFlow YouTube 扩展"))?;
        cutflow_download::process::command(&executable).arg(address)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()
            .map_err(|_| failure("无法打开当前 Chrome，请检查 Chrome 安装"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    { let _ = address; Err(failure("当前 YouTube 扩展入口仅支持 Windows Chrome")) }
}

pub fn prepare(job: &Job) -> Result<Launch> {
    validate(&job.params)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let token = uuid::Uuid::new_v4().to_string();
    let state = Arc::new(BridgeState {
        port: listener.local_addr()?.port(), token: token.clone(), job_id: job.id.clone(),
        video_path: Path::new(required(&job.params, "videoPath")?).canonicalize()?.to_string_lossy().into_owned(),
        title: required(&job.params, "title")?.trim().to_owned(), description: required(&job.params, "description")?.trim().to_owned(),
        finished: Mutex::new(None), stop: AtomicBool::new(false),
    });
    bridges().lock().unwrap().insert(job.id.clone(), state.clone());
    let worker_state = state.clone();
    std::thread::spawn(move || bridge_loop(listener, worker_state));
    // The extension reads the fragment and uses it only on the matching tab.
    let fragment = format!("cutflow-upload={}.{}.{}", job.id, token, state.port);
    let encoded_fragment: String = url::form_urlencoded::byte_serialize(fragment.as_bytes()).collect();
    let url = format!("https://www.youtube.com/@deqingdao/videos#{encoded_fragment}");
    Ok(Launch { url })
}
fn bridge_loop(listener: TcpListener, state: Arc<BridgeState>) {
    let deadline = Instant::now() + Duration::from_secs(900);
    while Instant::now() < deadline && !state.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => { let _ = serve_http(stream, &state); }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break,
        }
    }
    state.stop.store(true, Ordering::SeqCst);
}
fn write_http(mut stream: TcpStream, status: &str, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())?;
    stream.write_all(&body)?; Ok(())
}
fn serve_http(stream: TcpStream, state: &Arc<BridgeState>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut head = String::new(); reader.read_line(&mut head)?;
    let mut content_length = 0usize;
    loop {
        let mut line = String::new(); reader.read_line(&mut line)?;
        if line == "\r\n" || line == "\n" || line.is_empty() { break; }
        if let Some(value) = line.strip_prefix("Content-Length:").or_else(|| line.strip_prefix("content-length:")) {
            content_length = value.trim().parse::<usize>().unwrap_or(0);
        }
    }
    if head.starts_with("OPTIONS ") { return write_http(stream, "204 No Content", &json!({})); }
    let path = head.split_whitespace().nth(1).unwrap_or("");
    if !head.starts_with("POST ") || content_length > 64 * 1024 { return write_http(stream, "400 Bad Request", &json!({"error":"invalid request"})); }
    let length = content_length.min(64 * 1024);
    let mut body = vec![0u8; length]; if length > 0 { reader.read_exact(&mut body)?; }
    let value: Value = if body.is_empty() { json!({}) } else { serde_json::from_slice(&body)? };
    if value["jobId"].as_str() != Some(state.job_id.as_str()) || value["token"].as_str() != Some(state.token.as_str()) { return write_http(stream, "401 Unauthorized", &json!({"error":"bridge token invalid"})); }
    match path {
        "/claim" => write_http(stream, "200 OK", &json!({"jobId":state.job_id,"videoPath":state.video_path,"title":state.title,"description":state.description,"studioUrl":"https://studio.youtube.com/"})),
        "/complete" => { let result = if value["status"] == "filled" { Ok(()) } else { Err(failure(format!("YouTube 扩展未完成表单带入：{}", value["message"].as_str().unwrap_or("请检查 Studio 登录和扩展权限").chars().take(400).collect::<String>()))) }; *state.finished.lock().unwrap() = Some(result.clone()); write_http(stream, "200 OK", &json!({"accepted":result.is_ok()})) }
        _ => write_http(stream, "404 Not Found", &json!({"error":"not found"})),
    }
}
pub fn run(runtime: &Runtime, job: &Job, cancel: &std::sync::atomic::AtomicBool) -> Result<Value> {
    validate(&job.params)?;
    let state = bridges().lock().unwrap().get(&job.id).cloned().ok_or_else(|| failure("YouTube 上传桥接已失效，请重新点击上传"))?;
    runtime.update(&job.id, "等待当前 Chrome 的 YouTube 扩展带入成片");
    let outcome = loop {
        if let Err(error) = cancelled(cancel) { break Err(error); }
        if let Some(result) = state.finished.lock().unwrap().clone() { break result; }
        if state.stop.load(Ordering::SeqCst) { break Err(failure("等待 YouTube 扩展回执超时；请检查扩展是否已安装并启用")); }
        std::thread::sleep(Duration::from_millis(200));
    };
    state.stop.store(true, Ordering::SeqCst);
    bridges().lock().unwrap().remove(&job.id);
    outcome?;
    runtime.update(&job.id, "英文标题和简介已填入，请在 YouTube Studio 审核并完成发布设置");
    Ok(json!({"opened":true,"fileSelected":true,"metadataFilled":true,"uploadComplete":false,"published":false,"channelUrl":required(&job.params,"channelUrl")?,"videoPath":required(&job.params,"videoPath")?,"reviewRequired":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_and_channel_are_checked_before_bridge_creation() {
        let request = json!({"title":"A valid English title","description":"Description","channelUrl":"https://www.youtube.com/@deqingdao/videos","videoPath":"missing.mp4"});
        assert_eq!(validate(&request).unwrap_err().code, "invalid_media");
        assert!(!valid_title("中文标题"));
        assert!(!valid_title(&"x".repeat(101)));
    }
}
