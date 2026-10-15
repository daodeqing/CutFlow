//! CutFlow's BDL process adapter. No network listener, no shell commands.
pub mod actions;
pub mod credentials;
pub mod process;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Running,
    Paused,
    Failed,
    Ready,
    Imported,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub input: String,
    pub parts: String,
    pub quality: String,
    pub allow_lower: bool,
    pub target: PathBuf,
    pub rights: String,
    pub status: Status,
    pub error: Option<String>,
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub path: PathBuf,
    pub bytes: u64,
    pub width: u64,
    pub height: u64,
    pub duration: f64,
    pub codec: String,
    pub frame_rate: String,
    pub imported: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Queue {
    pub version: u32,
    pub jobs: Vec<Job>,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            version: 1,
            jobs: Vec::new(),
        }
    }
}

impl Queue {
    pub fn load(root: &Path) -> Result<Self> {
        let file = root.join("queue.json");
        if !file.exists() {
            return Ok(Self {
                version: 1,
                jobs: vec![],
            });
        }
        let mut queue: Self = serde_json::from_slice(&fs::read(file).map_err(io)?).map_err(io)?;
        if queue.version != 1 {
            return Err("任务文件版本不兼容，请保留原文件".into());
        }
        for job in &mut queue.jobs {
            if matches!(job.status, Status::Running | Status::Queued) {
                job.status = Status::Paused;
            }
        }
        Ok(queue)
    }
    pub fn save(&self, root: &Path) -> Result<()> {
        atomic_json(&root.join("queue.json"), self)
    }
    pub fn enqueue(
        &mut self,
        input: &str,
        parts: &str,
        quality: &str,
        allow_lower: bool,
        target: PathBuf,
        rights: &str,
    ) -> Result<String> {
        let input = validate_input(input)?;
        let parts = validate_parts(parts)?;
        if !["120", "80", "best"].contains(&quality) {
            return Err("不支持的画质策略".into());
        }
        if rights.trim().is_empty() {
            return Err("请填写素材用途或授权备注".into());
        }
        if self.jobs.len() >= 1000 {
            return Err("任务数量达到 1000 上限，请归档任务文件".into());
        }
        let mut selected = parts.split(',').collect::<Vec<_>>();
        selected.sort_unstable();
        if self.jobs.iter().any(|j| {
            let mut existing = j.parts.split(',').collect::<Vec<_>>();
            existing.sort_unstable();
            source_key(&j.input) == source_key(&input)
                && existing == selected
                && j.target == target
                && j.quality == quality
                && j.allow_lower == allow_lower
                && matches!(
                    j.status,
                    Status::Queued | Status::Running | Status::Paused | Status::Ready
                )
        }) {
            return Err("相同素材、分 P 与画质的任务已存在，请在任务列表继续或查看".into());
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io)?
            .as_nanos();
        let id = format!("{stamp:x}-{}", self.jobs.len());
        self.jobs.push(Job {
            id: id.clone(),
            input,
            parts,
            quality: quality.into(),
            allow_lower,
            target,
            rights: rights.trim().chars().take(1000).collect(),
            status: Status::Queued,
            error: None,
            assets: vec![],
        });
        Ok(id)
    }
}

/// A regular video URL and its BV/av id refer to the same source for deduplication.
pub fn source_key(input: &str) -> &str {
    input
        .split("/video/")
        .nth(1)
        .unwrap_or(input)
        .split(['?', '#', '/'])
        .next()
        .unwrap_or(input)
}

pub fn validate_input(input: &str) -> Result<String> {
    let input = input.trim();
    if input.len() == 12
        && input.starts_with("BV")
        && input.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Ok(input.into());
    }
    if input.starts_with("av") && input.len() > 2 && input[2..].bytes().all(|b| b.is_ascii_digit())
    {
        return Ok(input.into());
    }
    // Only regular videos for P0. B23 redirects remain BDL's allowlisted responsibility.
    if let Some(tail) = input.strip_prefix("https://") {
        if let Some((host, path)) = tail.split_once('/') {
            if ((host == "www.bilibili.com" || host == "bilibili.com")
                && path.starts_with("video/"))
                || host == "b23.tv"
            {
                if !input.chars().any(|c| c.is_whitespace() || c.is_control())
                    && input.len() <= 2048
                {
                    return Ok(input.into());
                }
            }
        }
    }
    Err("请输入普通视频 BV/av 号、https://www.bilibili.com/video/ 链接或 b23.tv 短链".into())
}

pub fn validate_parts(value: &str) -> Result<String> {
    let mut parts = Vec::new();
    for item in value.split(',') {
        let p: u32 = item
            .trim()
            .parse()
            .map_err(|_| "分 P 使用英文逗号分隔，例如 1,2")?;
        if p == 0 || p > 10000 {
            return Err("分 P 序号应为 1 至 10000".into());
        }
        if !parts.contains(&p) {
            parts.push(p);
        }
    }
    if parts.is_empty() || parts.len() > 100 {
        return Err("每个任务选择 1 至 100 个分 P".into());
    }
    Ok(parts
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(","))
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::create_dir_all(path.parent().ok_or("无效存储路径")?).map_err(io)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(io)?;
    let temp = path.with_extension("json.new");
    {
        use std::io::Write;
        let mut file = fs::File::create(&temp).map_err(io)?;
        file.write_all(&bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
    }
    replace_file(&temp, path)
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).map_err(io)
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH, same filesystem.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
        return Err(io(std::io::Error::last_os_error()));
    }
    Ok(())
}

pub fn io(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone)]
pub struct Engine {
    pub tools: PathBuf,
    pub data: PathBuf,
}

impl Engine {
    pub fn missing_components(&self) -> Vec<String> {
        ["bdl", "ffmpeg", "ffprobe"]
            .iter()
            .filter(|name| !self.binary(name).is_file())
            .map(|s| s.to_string())
            .collect()
    }
    pub fn check_components(&self, cancel: &AtomicBool) -> Result<String> {
        let mut lines = Vec::new();
        for name in ["bdl", "ffmpeg", "ffprobe"] {
            let mut cmd = process::command(&self.binary(name));
            cmd.arg(if name == "bdl" {
                "--version"
            } else {
                "-version"
            });
            let bytes = process::run(cmd, cancel, 15)?;
            let version = String::from_utf8_lossy(&bytes)
                .lines()
                .next()
                .unwrap_or("版本未知")
                .chars()
                .take(150)
                .collect::<String>();
            lines.push(format!("{name}：{version}"));
        }
        Ok(lines.join("\n"))
    }
    pub fn binary(&self, name: &str) -> PathBuf {
        self.tools.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        })
    }
    fn command(&self, cookie: Option<&str>) -> std::process::Command {
        let mut cmd = process::command(&self.binary("bdl"));
        // Do not inherit an unrelated ambient account.
        cmd.env_remove("BDL_COOKIE");
        if let Some(cookie) = cookie {
            cmd.env("BDL_COOKIE", cookie);
        }
        cmd
    }
    pub fn parse(&self, input: &str, cancel: &AtomicBool) -> Result<String> {
        let input = validate_input(input)?;
        let cookie = credentials::load(&self.data)?;
        let mut cmd = self.command(cookie.as_deref());
        cmd.args(["parse", &input, "--with-streams", "--json"]);
        let output = process::run(cmd, cancel, 90)?;
        let data: Value = serde_json::from_slice(&output).map_err(|_| "解析器返回了无效数据")?;
        // Project/store/UI never receive CDN addresses or headers.
        Ok(summarize(&data))
    }
    pub fn download(&self, job: &Job, cancel: Arc<AtomicBool>) -> Result<Vec<Asset>> {
        let root = self.data.join("jobs").join(&job.id);
        let output = root.join("media");
        fs::create_dir_all(&output).map_err(io)?;
        let state = root.join("bdl-state.json");
        let cookie = credentials::load(&self.data)?;
        let mut cmd = self.command(cookie.as_deref());
        cmd.arg("download")
            .arg(&job.input)
            .args([
                "--parts",
                &job.parts,
                "--quality",
                &job.quality,
                "--missing-quality",
                if job.allow_lower { "lower" } else { "skip" },
                "--codec",
                "auto",
                "--container",
                "mp4",
            ])
            .arg("--output")
            .arg(&output)
            .arg("--state")
            .arg(&state)
            .arg("--ffmpeg")
            .arg(self.binary("ffmpeg"))
            .arg("--json");
        if state.exists() {
            cmd.arg("--resume");
        }
        let output_bytes = process::run(cmd, &cancel, 6 * 60 * 60)?;
        let json: Value = serde_json::from_slice(&output_bytes)
            .map_err(|_| "下载器返回了无效数据；可保留断点手动恢复")?;
        let downloads = json
            .get("downloads")
            .and_then(Value::as_array)
            .ok_or("下载结果缺少 downloads")?;
        let mut assets = Vec::new();
        let allowed = output.canonicalize().map_err(io)?;
        for record in downloads {
            for file in record
                .get("outputs")
                .and_then(Value::as_array)
                .ok_or("下载结果缺少 outputs")?
            {
                let path = PathBuf::from(file.as_str().ok_or("文件路径不是字符串")?)
                    .canonicalize()
                    .map_err(io)?;
                if !path.starts_with(&allowed) {
                    return Err("下载器返回了任务目录外的路径".into());
                }
                let asset = self.probe(&path, &cancel)?;
                if job.quality == "120" && !job.allow_lower && !is_uhd(asset.width, asset.height) {
                    return Err(format!(
                        "严格 4K 校验失败：实际 {}×{}；文件保留，未入库",
                        asset.width, asset.height
                    ));
                }
                if !assets.iter().any(|a: &Asset| a.path == asset.path) {
                    assets.push(asset);
                }
            }
        }
        if assets.is_empty() {
            return Err("未生成任何媒体文件".into());
        }
        // Commit only after every final file passed probe. No scanning for partial files.
        let manifest = serde_json::json!({"version":1,"jobId":job.id,"source":job.input,
            "parts":job.parts,"requestedQuality":job.quality,"allowLower":job.allow_lower,
            "rights":job.rights,"targetProject":job.target,"assets":assets});
        atomic_json(&root.join("asset-manifest.json"), &manifest)?;
        Ok(assets)
    }
    pub fn probe(&self, path: &Path, cancel: &AtomicBool) -> Result<Asset> {
        let mut cmd = process::command(&self.binary("ffprobe"));
        cmd.args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(path);
        let bytes = process::run(cmd, cancel, 60)?;
        let json: Value = serde_json::from_slice(&bytes).map_err(io)?;
        let streams = json["streams"].as_array().ok_or("媒体缺少流信息")?;
        let video = streams
            .iter()
            .find(|s| s["codec_type"] == "video")
            .ok_or("下载文件没有视频轨")?;
        if !streams.iter().any(|s| s["codec_type"] == "audio") {
            return Err("下载文件没有音频轨，未入库".into());
        }
        let duration = json["format"]["duration"]
            .as_str()
            .unwrap_or("0")
            .parse::<f64>()
            .map_err(io)?;
        let width = video["width"].as_u64().unwrap_or(0);
        let height = video["height"].as_u64().unwrap_or(0);
        let bytes = fs::metadata(path).map_err(io)?.len();
        if bytes == 0 || width == 0 || height == 0 || !duration.is_finite() || duration <= 0.0 {
            return Err("媒体规格无效或文件不完整".into());
        }
        Ok(Asset {
            path: path.into(),
            bytes,
            width,
            height,
            duration,
            codec: video["codec_name"].as_str().unwrap_or("unknown").into(),
            frame_rate: video["avg_frame_rate"].as_str().unwrap_or("0/1").into(),
            imported: false,
        })
    }
}

pub fn is_uhd(width: u64, height: u64) -> bool {
    width.max(height) >= 3840 && width.min(height) >= 2160
}

pub fn summarize(data: &Value) -> String {
    let mut lines = vec![
        data["source"]["title"]
            .as_str()
            .unwrap_or("解析完成")
            .to_owned(),
    ];
    for group in data["groups"].as_array().into_iter().flatten() {
        for item in group["items"].as_array().into_iter().flatten() {
            for (i, part) in item["parts"].as_array().into_iter().flatten().enumerate() {
                let mut formats = Vec::new();
                for stream in part["streams"].as_array().into_iter().flatten() {
                    if stream["kind"] != "video" {
                        continue;
                    }
                    let q = stream["quality"]["value"].as_u64().unwrap_or(0);
                    let codec = stream["codec"].as_str().unwrap_or("unknown");
                    let label = format!("qn{q}/{codec}");
                    if !formats.contains(&label) {
                        formats.push(label);
                    }
                }
                lines.push(format!(
                    "P{} · {} · {}",
                    i + 1,
                    part["title"].as_str().unwrap_or("未命名"),
                    formats.join("、")
                ));
            }
        }
    }
    lines.push("实际像素、帧率与编码在下载完成后探测；可用档位以本次账号权限为准。".into());
    lines.join("\n")
}

#[cfg(test)]
mod tests;
