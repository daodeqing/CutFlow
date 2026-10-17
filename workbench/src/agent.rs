use cutflow_workbench::{
    Error, Result,
    protocol::{self, Call},
    server,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
};

fn home(arguments: &[String]) -> Result<PathBuf> {
    if let Some(index) = arguments.iter().position(|value| value == "--portable") {
        return arguments
            .get(index + 1)
            .map(PathBuf::from)
            .ok_or_else(|| Error::new("invalid_request", "--portable 缺少目录"));
    }
    cutflow_workbench::portable_root()
}

fn run() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty() || arguments.iter().any(|value| value == "--help") {
        println!(
            "cutflow-agent {}\n  mcp [--portable DIR]\n  call METHOD --params FILE [--portable DIR]\n  call METHOD --stdin [--portable DIR]\n  schema\nOpen CutFlow independently. Main window and editor provide separate local token channels. No Python or AI application is required to keep CutFlow running.", env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if arguments[0] == "schema" {
        println!("{}", protocol::tools());
        return Ok(());
    }
    let root = home(&arguments)?;
    if arguments[0] == "mcp" {
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            if line.len() > 2 * 1024 * 1024 {
                return Err(Error::new("invalid_request", "MCP 请求过长"));
            }
            let request: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(_) => {
                    println!(
                        "{}",
                        json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                    );
                    continue;
                }
            };
            if request.get("id").is_none() {
                continue;
            }
            let result = match request["method"].as_str().unwrap_or("") {
                "initialize" => Ok(
                    json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"CutFlow","version":env!("CARGO_PKG_VERSION")}}),
                ),
                "ping" => Ok(json!({})),
                "tools/list" => Ok(json!({"tools":protocol::tools()})),
                "tools/call" => {
                    let name = request["params"]["name"].as_str().unwrap_or("");
                    let method = protocol::METHODS
                        .iter()
                        .find(|(method, _)| method.replace('.', "_") == name)
                        .map(|(method, _)| *method);
                    let result = method
                        .ok_or_else(|| Error::new("method_not_found", "未知工具"))
                        .and_then(|method| {
                            server::call(
                                &root,
                                &Call {
                                    method: method.into(),
                                    params: request["params"]["arguments"].clone(),
                                },
                            )
                        });
                    Ok(match result {
                        Ok(value) => {
                            json!({"content":[{"type":"text","text":value.to_string()}],"isError":false})
                        }
                        Err(error) => {
                            json!({"content":[{"type":"text","text":serde_json::to_string(&error)?}],"isError":true})
                        }
                    })
                }
                _ => Err(Error::new("method_not_found", "未知 MCP 方法")),
            };
            let response = match result {
                Ok(value) => json!({"jsonrpc":"2.0","id":request["id"],"result":value}),
                Err(error) => {
                    json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32601,"message":error.message}})
                }
            };
            println!("{response}");
            std::io::stdout().flush()?;
        }
    } else if arguments[0] == "call" {
        let method = arguments
            .get(1)
            .ok_or_else(|| Error::new("invalid_request", "缺少 METHOD"))?;
        let mut text = String::new();
        if let Some(index) = arguments.iter().position(|value| value == "--params") {
            text = std::fs::read_to_string(
                arguments
                    .get(index + 1)
                    .ok_or_else(|| Error::new("invalid_request", "缺少请求文件"))?,
            )?;
        } else if arguments.iter().any(|value| value == "--stdin") {
            std::io::stdin()
                .take(2 * 1024 * 1024)
                .read_to_string(&mut text)?;
        }
        let params = if text.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&text)?
        };
        println!(
            "{}",
            server::call(
                &root,
                &Call {
                    method: method.clone(),
                    params
                }
            )?
        );
    } else {
        return Err(Error::new("invalid_request", "请运行 --help 查看用法"));
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", serde_json::to_string(&error).unwrap_or_default());
        std::process::exit(1);
    }
}
