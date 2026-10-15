#![recursion_limit = "256"]
pub mod accounts;
pub mod flow;
pub mod media;
pub mod protocol;
pub mod server;
pub mod tasks;
pub mod timeline;
pub mod workflow;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new("io_error", error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new("invalid_request", error.to_string())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

pub fn required<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| Error::new("invalid_request", format!("缺少 {name}")))
}

pub fn portable_root() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let folder = executable
        .parent()
        .ok_or_else(|| Error::new("io_error", "找不到程序目录"))?;
    let folder = if folder.file_name().is_some_and(|name| name == "tools") {
        folder.parent().unwrap_or(folder)
    } else {
        folder
    };
    Ok(folder.join("portable"))
}

pub fn secret(root: &Path, name: &str, value: Option<&str>) -> Result<Option<String>> {
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(Error::new("invalid_request", "凭据名称无效"));
    }
    std::fs::create_dir_all(root)?;
    let path = root.join(format!("{name}.dpapi"));
    if let Some(value) = value {
        let encrypted = cutflow_download::credentials::protect(value.as_bytes(), true)
            .map_err(|message| Error::new("credential_error", message))?;
        std::fs::write(path, encrypted)?;
        Ok(None)
    } else if path.exists() {
        let decrypted = cutflow_download::credentials::protect(&std::fs::read(path)?, false)
            .map_err(|message| Error::new("credential_error", message))?;
        Ok(Some(
            String::from_utf8(decrypted).map_err(|_| Error::new("credential_error", "凭据损坏"))?,
        ))
    } else {
        Ok(None)
    }
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| Error::new("io_error", "路径无父目录"))?;
    std::fs::create_dir_all(directory)?;
    cutflow_download::atomic_json(path, value).map_err(|message| Error::new("io_error", message))
}
