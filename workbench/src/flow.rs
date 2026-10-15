use crate::{Error, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
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
}

pub fn describe(id: &str, description: &str) -> Model {
    let image = id.contains("image") || id.contains("imagen");
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
        kind: if image { "image" } else { "video" }.into(),
        max_references: references,
        duration,
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
            Some(describe(
                item["id"].as_str()?,
                item["description"].as_str().unwrap_or(""),
            ))
        })
        .collect())
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

#[derive(Default)]
pub struct StreamResult {
    pub content: String,
    pub url: Option<String>,
    pub done: bool,
}

impl StreamResult {
    pub fn event(&mut self, data: &str) -> Result<()> {
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let payload: Value = serde_json::from_str(data)
            .map_err(|_| Error::new("result_unknown", "生成服务返回无效消息，结果待核查"))?;
        if payload.get("error").is_some() {
            return Err(Error::new(
                "provider_error",
                "生成服务报告失败，请在服务端核查详情与额度",
            ));
        }
        if let Some(url) = payload["url"].as_str() {
            self.url = Some(url.into());
        }
        let part = payload["choices"][0]["delta"]["content"]
            .as_str()
            .or_else(|| payload["choices"][0]["message"]["content"].as_str())
            .unwrap_or("");
        if self.content.len() + part.len() > 512 * 1024 {
            return Err(Error::new("provider_error", "生成服务响应超过安全长度"));
        }
        self.content.push_str(part);
        Ok(())
    }

    pub fn media_url(&self) -> Result<String> {
        if let Some(url) = &self.url {
            return Ok(url.clone());
        }
        for pattern in [
            r#"!\[[^\]]*\]\(([^\s)]+)\)"#,
            r#"<video\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
        ] {
            if let Some(found) = regex::Regex::new(pattern).unwrap().captures(&self.content) {
                return Ok(found[1].replace("&amp;", "&"));
            }
        }
        Err(Error::new(
            "result_unknown",
            "未收到最终媒体地址，请先核查服务端生成历史",
        ))
    }
}

pub fn generate(
    connection: &Connection,
    key: &str,
    request: &Value,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&str),
) -> Result<String> {
    let model = crate::required(request, "model")?;
    let prompt = crate::required(request, "prompt")?;
    let mut content = vec![json!({"type":"text","text":prompt})];
    let references = request["references"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let capability = describe(model, "");
    if references.len() > capability.max_references {
        return Err(Error::new(
            "invalid_request",
            "所选模型不支持此数量的参考图，请选图生模型",
        ));
    }
    for reference in references {
        let path = reference
            .as_str()
            .ok_or_else(|| Error::new("invalid_request", "参考图必须是本地路径"))?;
        content
            .push(json!({"type":"image_url","image_url":{"url":reference_data(Path::new(path))?}}));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(Error::new("cancelled", "提交前已取消"));
    }
    let response = agent()
        .post(&format!(
            "{}/chat/completions",
            base_url(&connection.base_url)?
        ))
        .set("Authorization", &format!("Bearer {key}"))
        .send_json(
            json!({"model":model,"messages":[{"role":"user","content":content}],"stream":true}),
        )
        .map_err(http_error)?;
    let streamed = response
        .header("Content-Type")
        .unwrap_or("")
        .contains("text/event-stream");
    let reader = response.into_reader().take(8 * 1024 * 1024);
    let mut result = StreamResult::default();
    if streamed {
        let mut event = String::new();
        for line in BufReader::new(reader).lines() {
            if cancel.load(Ordering::SeqCst) {
                return Err(Error::new(
                    "result_unknown",
                    "已停止本地等待；服务端可能仍在生成并计费",
                ));
            }
            let line =
                line.map_err(|_| Error::new("result_unknown", "流式连接中断，结果待核查"))?;
            if line.len() > 1024 * 1024 {
                return Err(Error::new("provider_error", "流式消息过长"));
            }
            if line.is_empty() && !event.is_empty() {
                result.event(event.trim_end())?;
                event.clear();
                progress("生成服务处理中；收到进度消息");
                if result.done {
                    break;
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                event.push_str(data.strip_prefix(' ').unwrap_or(data));
                event.push('\n');
            }
        }
        if !event.is_empty() {
            result.event(event.trim_end())?;
        }
    } else {
        let payload: Value = serde_json::from_reader(reader)
            .map_err(|_| Error::new("result_unknown", "响应中断或格式无效，生成结果待核查"))?;
        result.event(&payload.to_string())?;
    }
    result.media_url()
}

pub fn download(
    connection: &Connection,
    url: &str,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
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
        let response = media_agent.get(&target).call().map_err(|_| {
            Error::new(
                "download_error",
                "生成结果已返回，但媒体下载失败；可核查已保存的结果地址",
            )
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
        std::fs::rename(partial, destination)?;
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
