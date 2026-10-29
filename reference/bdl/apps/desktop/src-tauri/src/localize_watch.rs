//! Lightweight, in-process monitoring for explicitly authorized Bilibili sources.
use crate::cutflow::{Hub, localization_start};
use bdl_tauri::state::AppState;
use cutflow_workbench::{Error, Result, atomic_json, required};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path, time::{SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Manager};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct WatchFile { version: u32, watches: Vec<Watch>, submitted: Vec<String> }
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Watch {
    id: String, url: String, title: String, options: Value,
    initialized: bool, anchor: Option<String>, seen: Vec<String>, pending: Vec<Pending>,
    last_checked: u64, issue: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending { bvid: String, params: Value }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn read(path: &Path) -> Result<WatchFile> {
    if !path.exists() { return Ok(WatchFile { version: 1, ..Default::default() }); }
    let file: WatchFile = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|_| Error::new("store_error", "监控设置无法读取，已停止以避免覆盖"))?;
    if file.version != 1 { return Err(Error::new("store_error", "监控设置版本不兼容")); }
    Ok(file)
}
fn write(path: &Path, file: &WatchFile) -> Result<()> { atomic_json(path, &serde_json::to_value(file)?) }
fn path(app: &AppHandle) -> std::path::PathBuf { app.state::<Hub>().runtime.root.join("localization-watch.json") }

pub fn list(app: &AppHandle) -> Result<Value> { Ok(serde_json::to_value(read(&path(app))?)?) }
pub fn add(app: &AppHandle, params: &Value) -> Result<Value> {
    let url = required(params, "url")?.trim();
    if url.len() > 2048 { return Err(Error::new("invalid_request", "来源链接过长")); }
    match bdl_core::input::classify_input(url) {
        Ok(bdl_core::input::ClassifiedInput::Favorite { .. } | bdl_core::input::ClassifiedInput::Uploader { .. }) => {}
        _ => return Err(Error::new("invalid_request", "请输入 B站收藏夹或 UP 主空间链接")),
    }
    required(params, "rights")?;
    let model = required(params, "model")?;
    let hub = app.state::<Hub>();
    let _guard = hub.watch.lock().unwrap();
    let path = path(app);
    let mut file = read(&path)?;
    if file.watches.len() >= 20 { return Err(Error::new("invalid_request", "最多监控 20 个来源")); }
    if file.watches.iter().any(|watch| watch.url == url) { return Err(Error::new("operation_conflict", "此来源已在监控列表中")); }
    let id = uuid::Uuid::new_v4().to_string();
    let options = json!({"model":model,"rights":params["rights"],"quality":params["quality"].as_str().unwrap_or("80"),
        "subtitleModel":params["subtitleModel"],"danmakuModel":params["danmakuModel"],
        "speechModel":params["speechModel"],"postModel":params["postModel"],"trendHint":params["trendHint"],
        "transcription":params["transcription"].as_str().unwrap_or("remote"),
        "whisperModel":params["whisperModel"].as_str().unwrap_or("base"),
        "removeBottomText":params["removeBottomText"].as_bool().unwrap_or(false)});
    file.watches.push(Watch { id: id.clone(), url: url.into(), options, ..Default::default() });
    write(&path, &file)?;
    Ok(json!({"id":id}))
}
pub fn remove(app: &AppHandle, id: &str) -> Result<Value> {
    let hub = app.state::<Hub>();
    let _guard = hub.watch.lock().unwrap();
    let path = path(app); let mut file = read(&path)?;
    let before = file.watches.len();
    file.watches.retain(|watch| watch.id != id);
    if file.watches.len() == before { return Err(Error::new("not_found", "监控来源不存在")); }
    write(&path, &file)?;
    Ok(json!({"removed":true}))
}

// New entries must precede the prior first item. If the anchor disappeared we
// stop rather than mistake old entries on a later page for new uploads.
fn newer(first_page: &[String], initialized: bool, anchor: Option<&str>, seen: &[String]) -> std::result::Result<Vec<String>, &'static str> {
    if !initialized { return Ok(Vec::new()); }
    let end = match anchor {
        Some(anchor) => first_page.iter().position(|item| item == anchor).ok_or("上次位置不在当前页，请核查来源并停止后重新添加监控")?,
        None => first_page.len(),
    };
    Ok(first_page[..end].iter().filter(|id| !seen.contains(id)).cloned().collect())
}
fn page_ids(tree: &bdl_core::model::NormalizedSourceTree) -> Vec<String> {
    let mut ids = HashSet::new(); let mut page = Vec::new();
    for part in tree.groups.iter().flat_map(|group| &group.items).flat_map(|item| &item.parts) {
        if let Some(bvid) = part.bvid.as_ref().filter(|id| id.len() <= 24 && id.starts_with("BV") && id.bytes().all(|c| c.is_ascii_alphanumeric())) {
            if ids.insert(bvid.to_ascii_lowercase()) { page.push(bvid.clone()); }
        }
    }
    page
}
pub fn scan(app: &AppHandle, force: bool) -> Result<()> {
    let hub = app.state::<Hub>();
    let Ok(_guard) = hub.watch.try_lock() else { return Ok(()); };
    let path = path(app); let mut file = read(&path)?;
    if file.watches.is_empty() { return Ok(()); }
    let mut known = file.submitted.iter().map(|id| id.to_ascii_lowercase()).collect::<HashSet<_>>();
    known.extend(file.watches.iter().flat_map(|watch| watch.pending.iter().map(|item| item.bvid.to_ascii_lowercase())));
    for watch in &mut file.watches {
        if (!watch.issue.is_empty() && watch.issue.starts_with("上次位置")) || (!force && now().saturating_sub(watch.last_checked) < 3600) { continue; }
        watch.last_checked = now();
        let state = app.state::<AppState>();
        let tree = match tauri::async_runtime::block_on(state.parse_source(&watch.url, false)) {
            Ok(tree) => tree,
            Err(_) => { watch.issue = "检查失败，请确认 B站登录和来源权限".into(); continue; }
        };
        let page = page_ids(&tree);
        let found = match newer(&page, watch.initialized, watch.anchor.as_deref(), &watch.seen) {
            Ok(found) => found,
            Err(issue) => { watch.issue = issue.into(); continue; }
        };
        watch.title = tree.source.title;
        for id in found {
            if !known.insert(id.to_ascii_lowercase()) { continue; }
            let mut params = watch.options.clone();
            params["url"] = json!(format!("https://www.bilibili.com/video/{id}"));
            params["operationId"] = json!(format!("watch-{}", uuid::Uuid::new_v4()));
            watch.pending.push(Pending { bvid: id, params });
        }
        watch.initialized = true;
        watch.anchor = page.first().cloned();
        for id in page { if !watch.seen.contains(&id) { watch.seen.push(id); } }
        if watch.seen.len() > 5000 { watch.seen.drain(..watch.seen.len()-5000); }
        watch.issue.clear();
    }
    // Save detection and idempotency keys before enqueuing anything.
    write(&path, &file)?;
    if let Some((index, pending)) = file.watches.iter().enumerate().find_map(|(i, watch)| watch.pending.first().cloned().map(|item| (i, item))) {
        match localization_start(app, pending.params.clone()) {
            Ok(_) => {
                file.watches[index].pending.remove(0);
                file.submitted.push(pending.bvid);
                if file.submitted.len() > 5000 { file.submitted.drain(..file.submitted.len()-5000); }
            }
            Err(error) if error.code == "busy" => {}
            Err(error) => { file.watches[index].issue = error.message; }
        }
        write(&path, &file)?;
    }
    Ok(())
}
pub fn spawn_scan(app: AppHandle) {
    std::thread::spawn(move || { let _ = scan(&app, true); });
}

#[cfg(test)]
mod tests {
    use super::newer;
    fn ids(values: &[&str]) -> Vec<String> { values.iter().map(|value| value.to_string()).collect() }
    #[test] fn baseline_and_anchor_prevent_old_video_replays() {
        let page = ids(&["BVnew", "BVold", "BVolder"]);
        assert_eq!(newer(&page, false, None, &[]).unwrap(), Vec::<String>::new());
        assert_eq!(newer(&page, true, Some("BVold"), &ids(&["BVold", "BVolder"])).unwrap(), ids(&["BVnew"]));
        assert!(newer(&page, true, Some("BVmissing"), &ids(&["BVold"])).is_err());
        assert!(newer(&page, true, Some("BVold"), &ids(&["BVnew", "BVold"])).unwrap().is_empty());
    }
}
