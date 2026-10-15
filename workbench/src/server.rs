use crate::{Error, Result, atomic_json, protocol::Call, secret};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

pub type Dispatch = Arc<dyn Fn(Call) -> Result<Value> + Send + Sync>;

#[derive(Serialize, Deserialize)]
struct Discovery {
    address: SocketAddr,
    process: u32,
    version: String,
}

pub struct Server {
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    pub fn start(root: &Path, dispatch: Dispatch) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let token = uuid::Uuid::new_v4().to_string();
        secret(root, "automation-token", Some(&token))?;
        atomic_json(
            &root.join("automation.json"),
            &Discovery {
                address: listener.local_addr()?,
                process: std::process::id(),
                version: "0.4".into(),
            },
        )?;
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        let active = Arc::new(AtomicUsize::new(0));
        let worker = std::thread::spawn(move || {
            while !running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, peer))
                        if peer.ip().is_loopback() && active.load(Ordering::SeqCst) < 16 =>
                    {
                        active.fetch_add(1, Ordering::SeqCst);
                        let active = active.clone();
                        let token = token.clone();
                        let dispatch = dispatch.clone();
                        std::thread::spawn(move || {
                            if let Err(error) = serve(stream, &token, dispatch) {
                                eprintln!("CutFlow control request: {error}");
                            }
                            active.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Ok((stream, peer)) if peer.ip().is_loopback() => {
                        let _ = serve(stream, &token, Arc::new(|_| Err(Error::new("busy", "本地控制请求过多，请稍后查询任务"))));
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve(mut stream: TcpStream, token: &str, dispatch: Dispatch) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let mut line = String::new();
    BufReader::new(stream.try_clone()?)
        .take(2 * 1024 * 1024)
        .read_line(&mut line)?;
    let input: Value = serde_json::from_str(&line)?;
    let supplied = input["token"].as_str().unwrap_or("");
    let mismatch = supplied
        .as_bytes()
        .iter()
        .zip(token.as_bytes())
        .fold(0u8, |difference, (left, right)| difference | (left ^ right));
    let result = if supplied.len() != token.len() || mismatch != 0 {
        Err(Error::new("unauthorized", "控制令牌无效"))
    } else {
        serde_json::from_value::<Call>(input.clone())
            .map_err(Error::from)
            .and_then(|call| dispatch(call))
    };
    let response = match result {
        Ok(value) => json!({"jsonrpc":"2.0","id":input["id"],"result":value}),
        Err(error) => {
            json!({"jsonrpc":"2.0","id":input["id"],"error":{"code":-32000,"message":error.message,"data":{"code":error.code}}})
        }
    };
    writeln!(stream, "{response}")?;
    Ok(())
}

pub fn call(root: &Path, request: &Call) -> Result<Value> {
    match call_direct(root, request) {
        Err(error) if error.code == "not_running" && root.file_name().is_none_or(|name| name != "editor")
            && (request.method.starts_with("cutflow.editor.") || request.method.starts_with("cutflow.project.")
                || request.method.starts_with("cutflow.edit.") || request.method.starts_with("cutflow.jobs.")
                || matches!(request.method.as_str(), "cutflow.assets.import" | "cutflow.preview" | "cutflow.export.open" | "cutflow.speech.run" | "cutflow.captions.run" | "cutflow.workflow.run"))
            && root.join("editor/automation.json").is_file() => call_direct(&root.join("editor"), request),
        result => result,
    }
}

fn call_direct(root: &Path, request: &Call) -> Result<Value> {
    let discovery: Discovery =
        serde_json::from_slice(&std::fs::read(root.join("automation.json")).map_err(|_| {
            Error::new(
                "not_running",
                "请先打开 CutFlow，并在 AI 创作页启用外部控制",
            )
        })?)?;
    if !discovery.address.ip().is_loopback() {
        return Err(Error::new("unauthorized", "只允许本地控制地址"));
    }
    let token = secret(root, "automation-token", None)?
        .ok_or_else(|| Error::new("unauthorized", "缺少控制令牌"))?;
    let mut stream = TcpStream::connect_timeout(&discovery.address, Duration::from_secs(5))
        .map_err(|_| {
            Error::new(
                "not_running",
                "CutFlow 控制服务未启动；请在软件中启用外部控制",
            )
        })?;
    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    writeln!(
        stream,
        "{}",
        json!({"id":1,"method":request.method,"params":request.params,"token":token})
    )?;
    let mut line = String::new();
    BufReader::new(stream)
        .take(16 * 1024 * 1024)
        .read_line(&mut line)?;
    let response: Value = serde_json::from_str(&line)?;
    if response.get("error").is_some() {
        return Err(Error::new(
            response["error"]["data"]["code"]
                .as_str()
                .unwrap_or("remote_error"),
            response["error"]["message"].as_str().unwrap_or("请求失败"),
        ));
    }
    Ok(response["result"].clone())
}
