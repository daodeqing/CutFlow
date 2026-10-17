//! Project-bound creation state. No remote generation is performed by this module.
use crate::{Error, Result, flow, required, tasks::{Job, Runtime, Store}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
pub fn workspace() -> String { uuid::Uuid::new_v4().to_string() }
pub fn empty_draft() -> Value {
    json!({"model":"","modelSource":"catalog","prompt":"","aspect":"auto","imageSize":"auto",
        "count":"1","duration":"8","references":[],"stream":true,"independent":false,"selectedAssetPath":""})
}
pub fn default_drafts() -> Value { json!({"image":empty_draft(),"video":empty_draft()}) }
pub fn normalize_draft(draft: &Value) -> Value {
    let mut output = empty_draft();
    if let Some(fields) = draft.as_object() {
        for (key, value) in fields { output[key] = value.clone(); }
    }
    output
}
pub fn normalize_drafts(drafts: &Value) -> Value {
    json!({"image":normalize_draft(&drafts["image"]),"video":normalize_draft(&drafts["video"])})
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CreationProject {
    pub id: String,
    pub name: String,
    pub state: String,
    pub flow_project_id: Option<String>,
    pub instance_id: Option<String>,
    pub account_label: Option<String>,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub models: Option<Vec<flow::Model>>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub operation_id: String,
    #[serde(default)]
    pub preferred_model: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CreationSession {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub drafts: Value,
    #[serde(default)]
    pub operation_id: String,
    #[serde(default)]
    pub requested_title: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CreationTurn {
    pub id: String,
    pub session_id: String,
    pub job_id: String,
    pub prompt: String,
    pub params: Value,
    pub created_at: u64,
}

pub fn migrate(store: &mut Store) {
    // Legacy paths and request parameters remain untouched; their cloud owner is unknown.
    let jobs: Vec<Job> = store.jobs.iter().filter(|job| job.method == "cutflow.generate").cloned().collect();
    if !jobs.is_empty() {
        let timestamp = now();
        store.projects.push(CreationProject {
            id:"legacy".into(), name:"历史未归类".into(), state:"legacy".into(),
            flow_project_id:None, instance_id:None, account_label:None, error:None, models:None,
            created_at:timestamp, updated_at:timestamp, operation_id:String::new(), preferred_model:String::new(),
        });
        for (index, job) in jobs.iter().enumerate() {
            let timestamp = timestamp.saturating_add(index as u64);
            let session_id = format!("legacy-{}", job.id);
            store.sessions.push(CreationSession {
                id:session_id.clone(), project_id:"legacy".into(),
                title:job.params["prompt"].as_str().unwrap_or("历史生成").chars().take(40).collect(),
                created_at:timestamp, updated_at:timestamp,
                drafts:default_drafts(), operation_id:String::new(), requested_title:String::new(),
            });
            store.turns.push(CreationTurn {
                id:format!("legacy-turn-{}", job.id), session_id, job_id:job.id.clone(),
                prompt:job.params["prompt"].as_str().unwrap_or("").into(),
                params:job.params.clone(), created_at:timestamp,
            });
        }
    }
    store.version = 2;
    store.revision = store.revision.saturating_add(1);
}

fn page<T: Clone>(items: &[T], ids: &[String], params: &Value) -> Result<(Vec<T>, Option<String>)> {
    let start = if let Some(cursor) = params["cursor"].as_str().filter(|s| !s.is_empty()) {
        ids.iter().position(|id| id == cursor)
            .ok_or_else(|| Error::new("invalid_cursor", "列表游标已失效，请重新加载"))? + 1
    } else { 0 };
    let limit = params["limit"].as_u64().unwrap_or(50).clamp(1, 50) as usize;
    let end = (start + limit).min(items.len());
    Ok((items[start..end].to_vec(), if end < items.len() { Some(ids[end - 1].clone()) } else { None }))
}

impl Runtime {
    pub fn catalog(&self, refresh: bool) -> Result<Value> {
        if refresh { let _ = self.refresh_project_binding(); }
        let store = self.store.lock().unwrap();
        Ok(json!({"connection":store.connection, "models":store.models.iter().filter(|model| model.id != "gflow-project-control").collect::<Vec<_>>(),
            "references":store.references, "fault":self.fault.as_ref().or(store.project_binding_fault.as_ref()), "workspaceId":store.workspace_id,
            "instanceId":store.instance_id, "projectBinding":store.project_binding, "revision":store.revision}))
    }

    pub fn refresh_project_binding(&self) -> Result<Value> {
        let (connection, workspace_id) = {
            let store = self.store.lock().unwrap();
            (store.connection.clone(), store.workspace_id.clone())
        };
        let result = self.api_key().and_then(|key| flow::project_control(&connection, &key,
            &json!({"version":1,"op":"capabilities","workspaceId":workspace_id}))).and_then(|value| {
                if value["instanceId"].as_str().is_none_or(str::is_empty) {
                    Err(Error::new("project_binding_unavailable", "服务未返回稳定的实例标识"))
                } else { Ok(value) }
            });
        let mut store = self.store.lock().unwrap();
        // A delayed response for an old connection must never activate the new connection.
        if store.connection.base_url != connection.base_url {
            return Err(Error::new("connection_changed", "AI 服务连接已变化，请重新检查连接"));
        }
        match result {
            Ok(value) => {
                let instance = value["instanceId"].as_str().unwrap().to_string();
                let supported = value["projectBinding"].as_bool()
                    .or_else(|| value["capabilities"]["projectBinding"].as_bool()).unwrap_or(false);
                store.instance_id = Some(instance);
                store.project_binding = supported;
                store.project_binding_fault = if supported { None } else { Some("当前服务不支持专属 Flow 项目绑定，请先更新 GFlow 插件".into()) };
                if let Some(models) = value["models"].as_array() {
                    // This is the authoritative GFlow catalog for the connected instance.
                    // Preserve unrelated text models used by the connection test.
                    store.models.retain(|model| !model.id.starts_with("gflow-"));
                    for metadata in models {
                        if let Some(id) = metadata["id"].as_str().filter(|id| *id != "gflow-project-control") {
                            let mut model = flow::describe(id, metadata["description"].as_str().unwrap_or(""));
                            flow::apply_model_metadata(&mut model, metadata);
                            store.models.push(model);
                        }
                    }
                }
                store.revision = store.revision.saturating_add(1);
                drop(store);
                self.persist()?;
                if !supported { return Err(Error::new("project_binding_unavailable", "当前服务不支持专属 Flow 项目绑定，请先更新 GFlow 插件")); }
                Ok(value)
            }
            Err(error) => {
                store.project_binding = false;
                store.project_binding_fault = Some(error.message.clone());
                store.revision = store.revision.saturating_add(1);
                drop(store);
                self.persist()?;
                Err(error)
            }
        }
    }

    pub fn creation_dispatch(&self, method: &str, params: Value) -> Result<Value> {
        match method {
            "cutflow.creation.catalog" => self.catalog(params["refresh"].as_bool().unwrap_or(false)),
            "cutflow.creation.projects.list" => {
                let store = self.store.lock().unwrap();
                let mut items = store.projects.iter().collect::<Vec<_>>();
                items.sort_by(|a,b| b.updated_at.cmp(&a.updated_at).then_with(|| a.id.cmp(&b.id)));
                let ids = items.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
                let (projects, cursor) = page(&items, &ids, &params)?;
                let projects = projects.into_iter().cloned().collect::<Vec<_>>();
                drop(store);
                Ok(json!({"projects":projects,"nextCursor":cursor}))
            }
            "cutflow.creation.projects.create" => self.create_creation_project(&params),
            "cutflow.creation.projects.get" => {
                let id = required(&params, "projectId")?;
                let project = self.store.lock().unwrap().projects.iter().find(|p| p.id == id).cloned()
                    .ok_or_else(|| Error::new("not_found", "项目不存在"))?;
                if matches!(project.state.as_str(), "creating" | "creation_unknown") {
                    self.reconcile_creation_project(&project)
                } else { Ok(json!(project)) }
            }
            "cutflow.creation.sessions.list" => {
                let id = required(&params, "projectId")?;
                let store = self.store.lock().unwrap();
                if !store.projects.iter().any(|p| p.id == id) { return Err(Error::new("not_found", "项目不存在")); }
                let mut items = store.sessions.iter().filter(|s| s.project_id == id).collect::<Vec<_>>();
                items.sort_by(|a,b| b.updated_at.cmp(&a.updated_at).then_with(|| a.id.cmp(&b.id)));
                let ids = items.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
                let (sessions, cursor) = page(&items, &ids, &params)?;
                let sessions = sessions.into_iter().cloned().collect::<Vec<_>>();
                drop(store);
                Ok(json!({"sessions":sessions,"nextCursor":cursor}))
            }
            "cutflow.creation.sessions.create" => {
                let project_id = required(&params, "projectId")?;
                let operation = required(&params, "operationId")?;
                if operation.len() > 128 { return Err(Error::new("invalid_request", "operationId 过长")); }
                let title = params["title"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or("新聊天").chars().take(80).collect::<String>();
                let mut store = self.store.lock().unwrap();
                if let Some(session) = store.sessions.iter().find(|s| s.operation_id == operation) {
                    if session.project_id != project_id || (!session.requested_title.is_empty() && session.requested_title != title) {
                        return Err(Error::new("operation_conflict", "operationId 已被其他聊天使用"));
                    }
                    return Ok(json!(session));
                }
                let project = store.projects.iter().find(|p| p.id == project_id)
                    .ok_or_else(|| Error::new("not_found", "项目不存在"))?;
                if project.state != "ready" { return Err(Error::new("project_not_ready", "请先完成 Flow 项目绑定")); }
                let timestamp = now();
                let session = CreationSession { id:workspace(), project_id:project_id.into(), title:title.clone(),
                    created_at:timestamp, updated_at:timestamp, drafts:default_drafts(), operation_id:operation.into(), requested_title:title.clone() };
                store.sessions.push(session.clone());
                if let Some(project) = store.projects.iter_mut().find(|p| p.id == project_id) { project.updated_at = timestamp; }
                store.revision = store.revision.saturating_add(1);
                drop(store);
                self.persist()?;
                Ok(json!(session))
            }
            "cutflow.creation.turns.list" => {
                let id = required(&params, "sessionId")?;
                let (turns, jobs, cursor) = {
                    let store = self.store.lock().unwrap();
                    if !store.sessions.iter().any(|s| s.id == id) { return Err(Error::new("not_found", "聊天不存在")); }
                    let mut turns = store.turns.iter().filter(|t| t.session_id == id).collect::<Vec<_>>();
                    turns.sort_by(|a,b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
                    let ids = turns.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
                    let (turns, cursor) = page(&turns, &ids, &params)?;
                    let turns = turns.into_iter().cloned().collect::<Vec<_>>();
                    let jobs = store.jobs.iter().filter(|job| turns.iter().any(|turn| turn.job_id == job.id)).cloned().collect::<Vec<_>>();
                    (turns, jobs, cursor)
                };
                let turns = turns.iter().map(|turn| {
                    let mut value = json!(turn);
                    value["job"] = jobs.iter().find(|job| job.id == turn.job_id).map(|job| self.job_view(job)).unwrap_or(Value::Null);
                    value
                }).collect::<Vec<_>>();
                Ok(json!({"turns":turns,"nextCursor":cursor}))
            }
            "cutflow.creation.drafts.save" => {
                let id = required(&params, "sessionId")?;
                let kind = required(&params, "kind")?;
                if !matches!(kind, "image" | "video") || !params["draft"].is_object() || params["draft"].to_string().len() > 128 * 1024 {
                    return Err(Error::new("invalid_request", "草稿应为 image/video 对象，且不能超过 128 KiB"));
                }
                let mut store = self.store.lock().unwrap();
                if let Some(path) = params["draft"]["selectedAssetPath"].as_str().filter(|s| !s.is_empty()) {
                    if kind != "image" { return Err(Error::new("invalid_reference", "视频草稿不使用图片连续改图引用")); }
                    let belongs = store.turns.iter().filter(|turn| turn.session_id == id).any(|turn| {
                        store.jobs.iter().find(|job| job.id == turn.job_id && job.params["kind"] == "image").and_then(|job| job.result.as_ref())
                            .and_then(|result| result["assets"].as_array()).is_some_and(|assets| assets.iter().any(|asset| asset["path"].as_str() == Some(path)))
                    });
                    if !belongs { return Err(Error::new("invalid_reference", "连续改图只能选择当前聊天中已生成的图片")); }
                }
                if params["draft"].get("independent").is_some_and(|v| !v.is_boolean()) {
                    return Err(Error::new("invalid_request", "independent 应为布尔值"));
                }
                let session = store.sessions.iter_mut().find(|s| s.id == id)
                    .ok_or_else(|| Error::new("not_found", "聊天不存在"))?;
                session.drafts[kind] = normalize_draft(&params["draft"]);
                // Saving a draft does not reorder history while the user is browsing it.
                store.revision = store.revision.saturating_add(1);
                drop(store);
                self.persist()?;
                Ok(json!({"saved":true}))
            }
            _ => Err(Error::new("method_not_found", "未知创作方法")),
        }
    }

    fn create_creation_project(&self, params: &Value) -> Result<Value> {
        let _gate = self.project_creation_gate.lock().unwrap();
        let operation = required(params, "operationId")?;
        let name = required(params, "name")?.trim();
        let model = required(params, "preferredModel")?;
        if operation.len() > 128 || name.chars().count() > 80 || !model.starts_with("gflow-") || model == "gflow-project-control" {
            return Err(Error::new("invalid_request", "项目名最多 80 字，且必须选择 gflow- 生成模型"));
        }
        if let Some(project) = self.store.lock().unwrap().projects.iter().find(|p| p.operation_id == operation).cloned() {
            if project.name != name || project.preferred_model != model {
                return Err(Error::new("operation_conflict", "operationId 已被其他项目请求使用"));
            }
            // Reusing a pending operation can only query its receipt; never create again.
            return Ok(json!(project));
        }
        self.refresh_project_binding()?;
        let (connection, instance, workspace_id) = {
            let store = self.store.lock().unwrap();
            (store.connection.clone(), store.instance_id.clone(), store.workspace_id.clone())
        };
        let timestamp = now();
        let project = CreationProject { id:workspace(), name:name.into(), state:"creating".into(),
            flow_project_id:None, instance_id:instance, account_label:None, error:None, models:None,
            created_at:timestamp, updated_at:timestamp, operation_id:operation.into(), preferred_model:model.into() };
        {
            let mut store = self.store.lock().unwrap();
            store.projects.push(project.clone());
            store.revision = store.revision.saturating_add(1);
        }
        // This receipt must be durable before the remote create call is sent.
        self.persist()?;
        let result = self.api_key().and_then(|key| flow::project_control(&connection, &key,
            &json!({"version":1,"op":"projects.create","workspaceId":workspace_id,"operationId":operation,"projectId":project.id,"name":name,"preferredModel":model})));
        self.apply_project_response(&project, result)
    }

    fn reconcile_creation_project(&self, project: &CreationProject) -> Result<Value> {
        let _gate = self.project_creation_gate.lock().unwrap();
        // A simultaneous create may have completed while this reader waited.
        let current = self.store.lock().unwrap().projects.iter().find(|p| p.id == project.id).cloned()
            .ok_or_else(|| Error::new("not_found", "项目不存在"))?;
        if current.state == "ready" { return Ok(json!(current)); }
        let project = &current;
        self.refresh_project_binding()?;
        let (connection, workspace_id, instance) = {
            let store = self.store.lock().unwrap();
            (store.connection.clone(), store.workspace_id.clone(), store.instance_id.clone())
        };
        if instance != project.instance_id { return Err(Error::new("service_instance_changed", "AI 服务实例已变化，禁止查询或重建旧绑定")); }
        let response = self.api_key().and_then(|key| flow::project_control(&connection, &key,
            &json!({"version":1,"op":"projects.get","workspaceId":workspace_id,"operationId":project.operation_id,"projectId":project.id,"flowProjectId":project.flow_project_id})));
        self.apply_project_response(project, response)
    }

    fn apply_project_response(&self, original: &CreationProject, result: Result<Value>) -> Result<Value> {
        let mut project = original.clone();
        project.updated_at = now();
        match result {
            Ok(response) => {
                if response["instanceId"].as_str() != original.instance_id.as_deref() {
                    project.state = "creation_unknown".into();
                    project.error = Some("服务实例不一致，创建结果待核查".into());
                } else {
                    if let Some(models) = response["models"].as_array() {
                        project.models = Some(models.iter().filter_map(|metadata| {
                            let id = metadata["id"].as_str()?;
                            if id == "gflow-project-control" { return None; }
                            let mut model = flow::describe(id, metadata["description"].as_str().unwrap_or(""));
                            flow::apply_model_metadata(&mut model, metadata);
                            Some(model)
                        }).collect());
                    }
                    let remote = &response["project"];
                    let remote_id = remote["flowProjectId"].as_str().or_else(|| remote["projectId"].as_str());
                    let remote_error = remote["error"].as_str().or_else(|| remote["error"]["message"].as_str()).map(str::to_owned);
                    project.account_label = remote["accountLabel"].as_str().map(str::to_owned);
                    if let Some(id) = remote_id.filter(|s| !s.is_empty()) {
                        project.flow_project_id = Some(id.into());
                        project.state = match remote["state"].as_str().unwrap_or("ready") {
                            "ready" => "ready",
                            "failed" => "failed",
                            _ => "creation_unknown",
                        }.into();
                        project.error = remote_error;
                    } else {
                        project.state = if remote["state"] == "failed" { "failed" } else { "creation_unknown" }.into();
                        project.error = remote_error.or_else(|| Some("服务尚未确认项目标识，创建结果待核查".into()));
                    }
                }
            }
            Err(error) => {
                project.state = if matches!(error.code.as_str(), "provider_unauthorized" | "insufficient_credit" | "rate_limited" | "invalid_model" | "no_accounts" | "operation_conflict") { "failed" } else { "creation_unknown" }.into();
                project.error = Some(error.message);
            }
        }
        let mut store = self.store.lock().unwrap();
        let entry = store.projects.iter_mut().find(|p| p.id == original.id)
            .ok_or_else(|| Error::new("not_found", "项目记录不存在"))?;
        *entry = project.clone();
        store.revision = store.revision.saturating_add(1);
        drop(store);
        self.persist()?;
        Ok(json!(project))
    }

    /// Resolve a new generation once, before reserving its immutable job context.
    pub fn bind_generation(&self, params: &mut Value) -> Result<()> {
        let Some(project_id) = params["creationProjectId"].as_str().filter(|s| !s.is_empty()).map(str::to_owned) else {
            if params.get("sessionId").is_some() || params.get("turnId").is_some() {
                return Err(Error::new("project_required", "聊天生成必须指定所属项目"));
            }
            // Existing workflows and the agent API keep their legacy asset path.
            return Ok(());
        };
        let session_id = required(params, "sessionId")?.to_string();
        let turn_id = required(params, "turnId")?.to_string();
        if turn_id.len() > 128 { return Err(Error::new("invalid_request", "turnId 过长")); }
        let store = self.store.lock().unwrap();
        let project = store.projects.iter().find(|p| p.id == project_id)
            .ok_or_else(|| Error::new("not_found", "项目不存在"))?;
        if project.state != "ready" || project.flow_project_id.as_deref().is_none_or(str::is_empty) {
            return Err(Error::new("project_not_ready", "项目尚未绑定，历史未归类项目不可生成"));
        }
        if !store.sessions.iter().any(|s| s.id == session_id && s.project_id == project_id) {
            return Err(Error::new("session_project_mismatch", "聊天不属于所选项目"));
        }
        if !store.project_binding || store.instance_id != project.instance_id {
            return Err(Error::new("service_instance_changed", "请检查服务实例，禁止向另一套账号数据库提交旧项目"));
        }
        let model = required(params, "model")?;
        if !model.starts_with("gflow-") || model == "gflow-project-control" {
            return Err(Error::new("invalid_request", "绑定项目必须使用 gflow- 图片或视频生成模型"));
        }
        let kind = required(params, "kind")?;
        if let Some(value) = params.get("continuationAssetPath") {
            let path = value.as_str().filter(|s| !s.is_empty())
                .ok_or_else(|| Error::new("invalid_reference", "连续改图引用路径无效"))?;
            if kind != "image" { return Err(Error::new("invalid_reference", "连续改图仅支持图片生成")); }
            let belongs = store.turns.iter().filter(|turn| turn.session_id == session_id).any(|turn| {
                store.jobs.iter().find(|job| job.id == turn.job_id && job.params["kind"] == "image").and_then(|job| job.result.as_ref())
                    .and_then(|result| result["assets"].as_array()).is_some_and(|assets| assets.iter().any(|asset| asset["path"].as_str() == Some(path)))
            });
            if !belongs { return Err(Error::new("invalid_reference", "连续改图引用必须来自当前聊天的生成图片")); }
            if !params["references"].as_array().is_some_and(|references| references.iter().any(|reference| reference.as_str() == Some(path))) {
                return Err(Error::new("invalid_reference", "连续改图图片必须明确包含在本轮参考图中"));
            }
        }
        let described = flow::describe(model, "");
        if described.kind == "text" || (described.kind != "unknown" && described.kind != kind) {
            return Err(Error::new("invalid_request", "所选模型与图片/视频类型不匹配"));
        }
        let capabilities = project.models.as_ref().unwrap_or(&store.models);
        if let Some(capability) = capabilities.iter().find(|m| m.id == model) {
            if capability.kind != "unknown" && capability.kind != kind {
                return Err(Error::new("invalid_request", "所选模型与生成类型不匹配"));
            }
            if params["references"].as_array().is_some_and(|references| references.len() > capability.max_references) {
                return Err(Error::new("invalid_reference", "参考图超过所选模型上限"));
            }
            if let Some(aspect) = params["aspect_ratio"].as_str().filter(|s| *s != "auto") {
                if capability.aspects.as_ref().is_some_and(|values| !values.is_empty() && !values.iter().any(|v| v == aspect)) {
                    return Err(Error::new("invalid_request", "所选模型不支持该尺寸比例"));
                }
            }
            if let Some(size) = params["image_size"].as_str().filter(|s| *s != "auto") {
                if capability.image_sizes.as_ref().is_some_and(|values| !values.is_empty() && !values.iter().any(|v| v == size)) {
                    return Err(Error::new("invalid_request", "所选模型不支持该画质/分辨率"));
                }
            }
            if let Some(duration) = params["duration"].as_f64() {
                if capability.durations.as_ref().is_some_and(|values| !values.is_empty() && !values.contains(&duration)) {
                    return Err(Error::new("invalid_request", "所选模型不支持该时长"));
                }
            }
        }
        params["gflow_workspace_id"] = json!(store.workspace_id);
        params["gflow_project_id"] = json!(project.flow_project_id);
        params["gflow_instance_id"] = json!(project.instance_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pagination_has_a_stable_cursor_and_a_fifty_item_cap() {
        let items = (0..120).collect::<Vec<_>>();
        let ids = items.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        let (first, cursor) = page(&items, &ids, &json!({"limit":1000})).unwrap();
        assert_eq!(first.len(), 50);
        let (next, _) = page(&items, &ids, &json!({"cursor":cursor})).unwrap();
        assert_eq!(next[0], 50);
        assert!(page(&items, &ids, &json!({"cursor":"missing"})).is_err());
    }
}
