//! Small, headless Whisper worker. No desktop editor or AI agent is required.
use sha2::{Digest, Sha256};
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, time::Duration};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn model_spec(id: &str) -> Option<(u64, &'static str)> {
    match id {
        "tiny" => Some((77_691_713, "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21")),
        "base" => Some((147_951_465, "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe")),
        "small" => Some((487_601_967, "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b")),
        _ => None,
    }
}
fn model(root: &Path, id: &str) -> Result<PathBuf, String> {
    let (expected_size, expected_hash) = model_spec(id).ok_or("语音模型无效")?;
    let folder = root.join("whisper-models");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join(format!("ggml-{id}.bin"));
    if path.is_file() { return Ok(path); }
    let part = folder.join(format!("ggml-{id}.{}.part", std::process::id()));
    let mut output = OpenOptions::new().write(true).create_new(true).open(&part).map_err(|_| "无法创建模型下载文件")?;
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-{id}.bin");
    let response = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(120)).build()
        .get(&url).call().map_err(|_| "下载 Whisper 模型失败，请检查网络；保留已下载部分")?;
    let mut reader = response.into_reader();
    let mut buffer = [0u8; 65536]; let mut size = 0u64; let mut hasher = Sha256::new();
    loop {
        let count = reader.read(&mut buffer).map_err(|_| "模型下载中断；保留已下载部分")?;
        if count == 0 { break; }
        size += count as u64;
        if size > expected_size { return Err("模型文件大小异常，已保留供检查".into()); }
        hasher.update(&buffer[..count]); output.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())?;
    drop(output);
    if size != expected_size || format!("{:x}", hasher.finalize()) != expected_hash { return Err("模型校验失败，已保留下载文件".into()); }
    if path.exists() { return Err("模型文件已存在，请检查后重试".into()); }
    fs::rename(&part, &path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn command(path: &Path) -> Command {
    let mut result = Command::new(path);
    #[cfg(windows)] { use std::os::windows::process::CommandExt; result.creation_flags(0x08000000); }
    result.stdin(Stdio::null()); result
}
fn run() -> Result<(), String> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("cutflow-transcribe {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut args = std::env::args_os().skip(1);
    let video = PathBuf::from(args.next().ok_or("缺少视频路径")?);
    let id = args.next().ok_or("缺少语音模型")?.to_string_lossy().into_owned();
    let duration = args.next().ok_or("缺少视频时长")?.to_string_lossy().parse::<f64>().map_err(|_| "视频时长无效")?;
    if !duration.is_finite() || duration <= 0.0 || duration > 315_360_000.0 { return Err("视频时长无效".into()); }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let tools = exe.parent().ok_or("找不到工具目录")?;
    let root = tools.parent().ok_or("找不到程序目录")?.join("portable");
    let temp = root.join("temp"); fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    unsafe { std::env::set_var("TEMP", &temp); std::env::set_var("TMP", &temp); }
    let model_path = model(&root, &id)?;
    let context = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default()).map_err(|_| "无法加载 Whisper 模型")?;
    let mut state = context.create_state().map_err(|_| "无法初始化 Whisper")?;
    let mut result = Vec::new(); let mut offset = 0.0;
    while offset < duration {
        let length = (duration - offset).min(120.0);
        let output = command(&tools.join("ffmpeg.exe")).args(["-v","error","-ss",&format!("{offset:.3}"),"-t",&format!("{length:.3}"),"-i"])
            .arg(&video).args(["-map","0:a:0","-ac","1","-ar","16000","-f","f32le","pipe:1"])
            .output().map_err(|_| "无法启动 FFmpeg")?;
        if !output.status.success() || output.stdout.is_empty() || output.stdout.len() > 8_000_000 { return Err("无法读取视频音轨".into()); }
        let samples: Vec<f32> = output.stdout.chunks_exact(4).map(|bytes| f32::from_le_bytes([bytes[0],bytes[1],bytes[2],bytes[3]])).collect();
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(std::thread::available_parallelism().map(|x| x.get().min(8)).unwrap_or(4) as i32);
        params.set_language(Some("auto")); params.set_print_special(false); params.set_print_progress(false);
        params.set_print_realtime(false); params.set_print_timestamps(false); params.set_suppress_blank(true);
        state.full(params, &samples).map_err(|_| "Whisper 识别失败")?;
        for segment in state.as_iter() {
            let start = segment.start_timestamp() as f64 / 100.0 + offset;
            let end = segment.end_timestamp() as f64 / 100.0 + offset;
            let text = segment.to_str_lossy().map_err(|_| "语音结果编码无效")?.trim().to_owned();
            if end > start && !text.is_empty() && !(text.starts_with('[') && text.ends_with(']')) {
                result.push(serde_json::json!({"start":start,"end":end,"text":text}));
            }
        }
        offset += length;
    }
    println!("{}", serde_json::json!({"segments":result}));
    Ok(())
}
fn main() { if let Err(error) = run() { eprintln!("{error}"); std::process::exit(1); } }
