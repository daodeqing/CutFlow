//! Bounded output and cancellable, shell-free child processes.
use crate::{Result, io};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub fn command(path: &Path) -> Command {
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.stdin(Stdio::null());
    cmd
}

fn drain(mut stream: impl Read) -> Vec<u8> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    while let Ok(size) = stream.read(&mut chunk) {
        if size == 0 {
            break;
        }
        // Continue draining after the cap so a verbose child cannot deadlock.
        let left = (8 * 1024 * 1024usize).saturating_sub(data.len());
        data.extend_from_slice(&chunk[..size.min(left)]);
    }
    data
}

pub fn run(mut cmd: Command, cancel: &AtomicBool, seconds: u64) -> Result<Vec<u8>> {
    if cancel.load(Ordering::SeqCst) {
        return Err("任务已暂停".into());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|_| "无法启动组件，请检查 tools 中的 bdl、ffmpeg 和 ffprobe")?;
    let stdout = child.stdout.take().ok_or("无法读取组件输出")?;
    let stderr = child.stderr.take().ok_or("无法读取组件错误")?;
    let out = thread::spawn(move || drain(stdout));
    let err = thread::spawn(move || drain(stderr));
    let start = Instant::now();
    let mut stopped = false;
    let status = loop {
        if cancel.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(seconds) {
            stopped = true;
            #[cfg(windows)]
            {
                // BDL can be muxing with FFmpeg. Stop the entire owned process tree.
                let system = std::env::var_os("SystemRoot")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| "C:\\Windows".into());
                let mut killer = command(&system.join("System32/taskkill.exe"));
                let _ = killer
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .status();
            }
            let _ = child.kill();
            break child.wait().map_err(io)?;
        }
        match child.try_wait().map_err(io)? {
            Some(status) => break status,
            None => thread::sleep(Duration::from_millis(100)),
        }
    };
    let stdout = out.join().map_err(|_| "组件输出线程失败")?;
    let _stderr = err.join().map_err(|_| "组件错误线程失败")?;
    // Raw stderr can include Cookie/headers/CDN URLs. Never surface or persist it.
    if stopped {
        return Err(if cancel.load(Ordering::SeqCst) {
            "任务已暂停；保留断点"
        } else {
            "组件超时；已停止，请手动恢复"
        }
        .into());
    }
    if !status.success() {
        return Err("组件执行失败：请检查账号权限、可用画质、网络或磁盘；不会自动重试。403/412/429 冷却由 BDL 保留。".into());
    }
    Ok(stdout)
}
