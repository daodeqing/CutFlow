use cutflow_workbench::{
    Error, creation::{CreationProject, CreationSession, CreationTurn}, flow::{self, Connection},
    tasks::{Job, Runtime}, protocol,
};
use serde_json::{Value, json};
use std::{io::{Read, Write}, net::TcpListener, path::PathBuf, sync::atomic::AtomicBool};

fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cutflow-creation-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn local_control(
    responses: Vec<Option<Value>>, root: Option<PathBuf>,
) -> (Connection, std::thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let connection = Connection { base_url:format!("http://{}/v1",listener.local_addr().unwrap()) };
    let worker = std::thread::spawn(move || {
        let mut requests = vec![];
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
            let mut bytes = vec![];
            let mut buffer = [0; 4096];
            let body_start;
            loop {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let size = headers.lines().find_map(|line| line.strip_prefix("content-length:").and_then(|s| s.trim().parse::<usize>().ok())).unwrap_or(0);
                    if bytes.len() >= end + 4 + size { body_start=end + 4; break; }
                }
            }
            let request: Value = serde_json::from_slice(&bytes[body_start..]).unwrap();
            assert_eq!(request["model"], "gflow-project-control");
            assert_eq!(request["stream"], false);
            let control: Value = serde_json::from_str(request["messages"][0]["content"].as_str().unwrap()).unwrap();
            if control["op"] == "projects.create" {
                if let Some(root) = &root {
                    let stored: Value = serde_json::from_slice(&std::fs::read(root.join("workbench.json")).unwrap()).unwrap();
                    assert_eq!(stored["projects"][0]["state"], "creating");
                    assert_eq!(stored["projects"][0]["operationId"], control["operationId"]);
                }
            }
            requests.push(control);
            if let Some(response) = response {
                let body = json!({"choices":[{"message":{"content":response.to_string()}}]}).to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        }
        requests
    });
    (connection, worker)
}

fn capability(instance: &str) -> Value {
    json!({"version":1,"instanceId":instance,"projectBinding":true,"capabilities":{"projectBinding":true},
        "models":[{"id":"gflow-nano-banana-2.1","kind":"image","maxReferences":3,"aspects":["1:1","16:9"],"imageSizes":["1K","2K","4K"]}]})
}

fn ready_runtime() -> Runtime {
    let root = scratch();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    {
        let mut store = runtime.store.lock().unwrap();
        store.instance_id = Some("service-a".into());
        store.project_binding = true;
        store.projects.push(serde_json::from_value::<CreationProject>(json!({
            "id":"project-a","name":"A","state":"ready","flowProjectId":"real-google-project",
            "instanceId":"service-a","accountLabel":"account-a","createdAt":1,"updatedAt":1
        })).unwrap());
        store.sessions.push(serde_json::from_value::<CreationSession>(json!({
            "id":"session-a","projectId":"project-a","title":"新聊天","createdAt":1,"updatedAt":1,
            "drafts":{"image":{},"video":{}},"operationId":"session-create"
        })).unwrap());
        store.sessions.push(serde_json::from_value::<CreationSession>(json!({
            "id":"session-b","projectId":"project-a","title":"B","createdAt":2,"updatedAt":2,
            "drafts":{"image":{},"video":{}}
        })).unwrap());
    }
    runtime.persist().unwrap();
    runtime
}

fn request(operation: &str, session: &str, turn: &str) -> Value {
    json!({"operationId":operation,"creationProjectId":"project-a","sessionId":session,"turnId":turn,
        "model":"gflow-nano-banana-2.1","kind":"image","prompt":"a cat","maxGenerationCalls":1})
}

#[test]
fn v1_migration_keeps_original_bytes_paths_and_unclassified_ownership() {
    let root = scratch();
    let asset = root.join("generated/old-job/generated-0.png");
    std::fs::create_dir_all(asset.parent().unwrap()).unwrap();
    std::fs::write(&asset, b"original image").unwrap();
    let original = json!({"version":1,"connection":{"baseUrl":""},"models":[],"references":[],"jobs":[{
        "id":"old-job","operationId":"old-op","method":"cutflow.generate","params":{"prompt":"old cat","model":"gflow-nano-banana-2.1","kind":"image"},
        "status":"completed","stage":"done","result":{"assets":[{"path":asset}]},"error":null
    }]}).to_string();
    std::fs::write(root.join("workbench.json"), &original).unwrap();
    let runtime = Runtime::open(root.clone(),root.join("tools")).unwrap();
    assert_eq!(std::fs::read_to_string(root.join("backups/workbench.v1.json")).unwrap(),original);
    assert_eq!(std::fs::read(&asset).unwrap(), b"original image");
    let store = runtime.store.lock().unwrap();
    assert_eq!(store.version,2);
    assert_eq!(store.projects[0].id,"legacy");
    assert_eq!(store.projects[0].state,"legacy");
    assert!(store.projects[0].flow_project_id.is_none());
    assert_eq!(store.jobs[0].result.as_ref().unwrap()["assets"][0]["path"],json!(asset));
    assert_eq!(store.turns[0].job_id,"old-job");
    let session = store.sessions[0].id.clone();
    drop(store);
    let mut params = request("legacy-rejected",&session,"turn-legacy");
    params["creationProjectId"]=json!("legacy");
    assert_eq!(runtime.reserve("cutflow.generate",params).err().unwrap().code,"project_not_ready");
}

#[test]
fn control_creation_is_durable_idempotent_and_does_not_generate_media() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(),root.join("tools")).unwrap();
    let response = json!({"version":1,"instanceId":"service-a","project":{
        "id":"remote-local-id","flowProjectId":"real-google-project","state":"ready","accountLabel":"Google A"
    }});
    let (connection, worker) = local_control(vec![Some(capability("service-a")),Some(response)],Some(root.clone()));
    runtime.dispatch("cutflow.connection.save",json!({"baseUrl":connection.base_url,"apiKey":"test-only-key"})).unwrap();
    let params = json!({"name":"test project","preferredModel":"gflow-nano-banana-2.1","operationId":"create-once"});
    let first = runtime.dispatch("cutflow.creation.projects.create",params.clone()).unwrap();
    let again = runtime.dispatch("cutflow.creation.projects.create",params.clone()).unwrap();
    assert_eq!(first,again);
    assert_eq!(first["state"],"ready");
    assert_eq!(first["flowProjectId"],"real-google-project");
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(),2);
    assert_eq!(requests[0]["op"],"capabilities");
    assert_eq!(requests[1]["op"],"projects.create");
    assert_eq!(requests[1]["projectId"],first["id"]);
    assert_eq!(requests[0]["workspaceId"],requests[1]["workspaceId"]);
    assert_eq!(runtime.catalog(false).unwrap()["models"][0]["imageSizes"][2],"4K");
    assert!(runtime.store.lock().unwrap().jobs.is_empty());
    drop(runtime);
    let restarted = Runtime::open(root.clone(),root.join("tools")).unwrap();
    assert_eq!(restarted.dispatch("cutflow.creation.projects.create",params).unwrap()["id"],first["id"]);
}

#[test]
fn a_lost_creation_response_remains_unknown_without_recreating() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(),root.join("tools")).unwrap();
    let (connection, worker)=local_control(vec![Some(capability("service-a")),None],Some(root.clone()));
    runtime.dispatch("cutflow.connection.save",json!({"baseUrl":connection.base_url,"apiKey":"test-key"})).unwrap();
    let params=json!({"name":"uncertain","preferredModel":"gflow-nano-banana-2.1","operationId":"uncertain-create"});
    let result=runtime.dispatch("cutflow.creation.projects.create",params.clone()).unwrap();
    assert_eq!(result["state"],"creation_unknown");
    assert_eq!(runtime.dispatch("cutflow.creation.projects.create",params.clone()).unwrap()["id"],result["id"]);
    assert_eq!(worker.join().unwrap().len(),2);
    drop(runtime);
    let restarted=Runtime::open(root.clone(),root.join("tools")).unwrap();
    assert_eq!(restarted.dispatch("cutflow.creation.projects.create",params).unwrap()["state"],"creation_unknown");
}

#[test]
fn binding_freezes_real_project_and_enforces_session_ownership_and_busy() {
    let runtime=ready_runtime();
    let params=request("first","session-a","turn-a");
    let (job, fresh, _)=runtime.reserve("cutflow.generate",params.clone()).unwrap();
    assert!(fresh);
    assert_eq!(job.params["gflow_project_id"],"real-google-project");
    assert_eq!(job.params["gflow_instance_id"],"service-a");
    assert_eq!(job.params["gflow_workspace_id"],runtime.catalog(false).unwrap()["workspaceId"]);
    assert_eq!(runtime.reserve("cutflow.generate",params.clone()).unwrap().0.id,job.id);
    assert!(!runtime.reserve("cutflow.generate",params).unwrap().1);
    assert_eq!(runtime.reserve("cutflow.generate",request("busy","session-a","turn-b")).err().unwrap().code,"session_busy");
    assert!(runtime.reserve("cutflow.generate",request("other-session","session-b","turn-other")).is_ok());
    let mut foreign=request("foreign","missing-session","turn-foreign");
    assert_eq!(runtime.reserve("cutflow.generate",foreign.clone()).err().unwrap().code,"session_project_mismatch");
    foreign["creationProjectId"]=json!("missing-project");
    assert_eq!(runtime.reserve("cutflow.generate",foreign).err().unwrap().code,"not_found");
    let store=runtime.store.lock().unwrap();
    assert_eq!(store.turns.len(),2);
    assert_eq!(store.turns[0].session_id,"session-a");
    assert_eq!(store.sessions.iter().find(|s| s.id=="session-a").unwrap().title,"a cat");
}

#[test]
fn terminal_changes_appear_in_revision_delta_and_preserve_turn_results() {
    let runtime=ready_runtime();
    let (job,_,_)=runtime.reserve("cutflow.generate",request("delta","session-a","turn-delta")).unwrap();
    let initial=runtime.dispatch("cutflow.jobs.list",json!({"activeOnly":true})).unwrap();
    let revision=initial["revision"].as_u64().unwrap();
    assert_eq!(initial["jobs"].as_array().unwrap().len(),1);
    runtime.update(&job.id,"generating");
    runtime.finish(&job.id,Ok(json!({"assets":[{"path":"generated/cat.png","thumbnail":"generated/thumb.jpg"}]})));
    let changed=runtime.dispatch("cutflow.jobs.list",json!({"sinceRevision":revision})).unwrap();
    assert_eq!(changed["jobs"].as_array().unwrap().len(),1);
    assert_eq!(changed["jobs"][0]["status"],"completed");
    assert!(changed["revision"].as_u64().unwrap()>revision);
    assert!(runtime.dispatch("cutflow.jobs.list",json!({"activeOnly":true})).unwrap()["jobs"].as_array().unwrap().is_empty());
    let turn=runtime.dispatch("cutflow.creation.turns.list",json!({"sessionId":"session-a"})).unwrap();
    assert_eq!(turn["turns"][0]["job"]["result"]["assets"][0]["path"],"generated/cat.png");
    assert_eq!(runtime.dispatch("cutflow.creation.turns.list",json!({"sessionId":"session-b"})).unwrap()["turns"].as_array().unwrap().len(),0);
}

#[test]
fn per_kind_drafts_and_selected_asset_cannot_cross_sessions() {
    let runtime=ready_runtime();
    let (job,_,_)=runtime.reserve("cutflow.generate",request("image","session-a","turn-image")).unwrap();
    runtime.finish(&job.id,Ok(json!({"assets":[{"path":"generated/cat.png"}]})));
    runtime.dispatch("cutflow.creation.drafts.save",json!({"sessionId":"session-a","kind":"image","draft":{"prompt":"red cat","independent":false,"selectedAssetPath":"generated/cat.png"}})).unwrap();
    runtime.dispatch("cutflow.creation.drafts.save",json!({"sessionId":"session-a","kind":"video","draft":{"prompt":"walking cat","independent":true}})).unwrap();
    assert_eq!(runtime.dispatch("cutflow.creation.drafts.save",json!({"sessionId":"session-b","kind":"image","draft":{"selectedAssetPath":"generated/cat.png"}})).unwrap_err().code,"invalid_reference");
    let root=runtime.root.clone();
    drop(runtime);
    let restarted=Runtime::open(root.clone(),root.join("tools")).unwrap();
    let sessions=restarted.dispatch("cutflow.creation.sessions.list",json!({"projectId":"project-a"})).unwrap();
    let session=sessions["sessions"].as_array().unwrap().iter().find(|s|s["id"]=="session-a").unwrap();
    assert_eq!(session["drafts"]["image"]["prompt"],"red cat");
    assert_eq!(session["drafts"]["video"]["prompt"],"walking cat");
}

#[test]
fn instance_changes_block_old_projects_and_catalog_refresh_is_nonfatal() {
    let runtime=ready_runtime();
    {
        let mut store=runtime.store.lock().unwrap();
        store.instance_id=Some("service-b".into());
    }
    assert_eq!(runtime.reserve("cutflow.generate",request("instance","session-a","turn-instance")).err().unwrap().code,"service_instance_changed");
    // Missing API key is represented in the catalog without preventing settings from opening.
    let catalog=runtime.catalog(true).unwrap();
    assert_eq!(catalog["projectBinding"],false);
    assert!(catalog["fault"].as_str().unwrap().contains("API Key"));
    assert_eq!(runtime.store.lock().unwrap().jobs.len(),0);
}

#[test]
fn ready_project_capability_refresh_queries_the_bound_account_without_new_creation() {
    let runtime = ready_runtime();
    let response = json!({"version":1,"instanceId":"service-a","project":{
        "id":"project-a","flowProjectId":"real-google-project","state":"ready","accountLabel":"Account A"
    },"models":[{"id":"gflow-nano-banana-2.1","kind":"image","maxReferences":3,"imageSizes":["1K","2K"]}]});
    let (connection, worker) = local_control(vec![Some(capability("service-a")), Some(response)], None);
    runtime.dispatch("cutflow.connection.save", json!({"baseUrl":connection.base_url,"apiKey":"fixture"})).unwrap();
    let cached = runtime.dispatch("cutflow.creation.projects.get", json!({"projectId":"project-a"})).unwrap();
    assert_eq!(cached["state"], "ready");
    let updated = runtime.dispatch("cutflow.creation.projects.get", json!({"projectId":"project-a","refresh":true})).unwrap();
    assert_eq!(updated["models"][0]["imageSizes"], json!(["1K","2K"]));
    assert_eq!(updated["flowProjectId"], "real-google-project");
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["op"], "capabilities");
    assert_eq!(requests[1]["op"], "projects.get");
    assert_eq!(requests[1]["flowProjectId"], "real-google-project");
    assert!(runtime.store.lock().unwrap().jobs.is_empty());
    assert!(protocol::tools().to_string().contains("refresh"));
}

#[test]
fn ready_project_refresh_failure_keeps_the_existing_binding_and_capabilities() {
    let runtime = ready_runtime();
    let (connection, worker) = local_control(vec![Some(capability("service-a")), None], None);
    runtime.dispatch("cutflow.connection.save", json!({"baseUrl":connection.base_url,"apiKey":"fixture"})).unwrap();
    assert!(runtime.dispatch("cutflow.creation.projects.get", json!({"projectId":"project-a","refresh":true})).is_err());
    let existing = runtime.dispatch("cutflow.creation.projects.get", json!({"projectId":"project-a"})).unwrap();
    assert_eq!(existing["state"], "ready");
    assert_eq!(existing["flowProjectId"], "real-google-project");
    assert_eq!(worker.join().unwrap()[1]["op"], "projects.get");
}

#[test]
fn projects_and_turns_page_large_histories_without_catalog_payload_growth() {
    let runtime=ready_runtime();
    {
        let mut store=runtime.store.lock().unwrap();
        store.sessions.clear();
        for index in 0..500 {
            store.sessions.push(serde_json::from_value::<CreationSession>(json!({
                "id":format!("session-{index}"),"projectId":"project-a","title":format!("Chat {index}"),
                "createdAt":index,"updatedAt":index,"drafts":{"image":{},"video":{}}
            })).unwrap());
        }
        for index in 0..1000 {
            let job_id=format!("job-{index}");
            let session_id="session-0";
            store.jobs.push(serde_json::from_value::<Job>(json!({
                "id":job_id,"operationId":format!("op-{index}"),"method":"cutflow.generate",
                "params":{"sessionId":session_id,"creationProjectId":"project-a"},"status":"completed",
                "stage":"done","result":null,"error":null,"revision":index+1
            })).unwrap());
            store.turns.push(CreationTurn {id:format!("turn-{index}"),session_id:session_id.into(),job_id,
                prompt:"cat".into(),params:json!({}),created_at:index});
        }
    }
    let mut count=0; let mut cursor=Value::Null;
    loop {
        let page=runtime.dispatch("cutflow.creation.sessions.list",json!({"projectId":"project-a","cursor":cursor})).unwrap();
        count+=page["sessions"].as_array().unwrap().len();
        cursor=page["nextCursor"].clone();
        if cursor.is_null(){break;}
    }
    assert_eq!(count,500);
    let turns=runtime.dispatch("cutflow.creation.turns.list",json!({"sessionId":"session-0"})).unwrap();
    assert_eq!(turns["turns"].as_array().unwrap().len(),50);
    assert_eq!(turns["turns"][0]["id"],"turn-999");
    let catalog=runtime.catalog(false).unwrap();
    assert!(catalog.get("jobs").is_none());
    assert!(catalog.get("turns").is_none());
    assert!(catalog.to_string().len()<2000);
}

#[test]
fn control_model_is_not_selectable_and_generation_forwards_fixed_binding() {
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let connection=Connection {base_url:format!("http://{}/v1",listener.local_addr().unwrap())};
    let worker=std::thread::spawn(move || {
        let (mut stream, _)=listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
        let mut bytes=vec![]; let mut buffer=[0;4096]; let offset;
        loop {
            let n=stream.read(&mut buffer).unwrap();assert!(n>0);bytes.extend_from_slice(&buffer[..n]);
            if let Some(end)=bytes.windows(4).position(|w|w==b"\r\n\r\n"){
                let header=String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                let size=header.lines().find_map(|l|l.strip_prefix("content-length:").and_then(|s|s.trim().parse::<usize>().ok())).unwrap_or(0);
                if bytes.len()>=end+4+size{offset=end+4;break;}
            }
        }
        let value:Value=serde_json::from_slice(&bytes[offset..]).unwrap();
        let body=json!({"choices":[{"message":{"content":"![cat](https://cdn.example/cat.png)"}}]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        value
    });
    flow::generate(&connection,"fixture",&json!({"model":"gflow-nano-banana-2.1","prompt":"cat","stream":false,
        "gflow_workspace_id":"workspace-a","gflow_project_id":"real-google-project","gflow_instance_id":"service-a"}),
        &AtomicBool::new(false), |_|{}).unwrap();
    let payload=worker.join().unwrap();
    assert_eq!(payload["gflow_workspace_id"],"workspace-a");
    assert_eq!(payload["gflow_project_id"],"real-google-project");
    assert_eq!(payload["gflow_instance_id"],"service-a");
    assert_eq!(flow::generate(&connection,"fixture",&json!({"model":"gflow-project-control","prompt":"cat"}),
        &AtomicBool::new(false), |_|{}).unwrap_err().code,"invalid_request");
    let tools=protocol::tools();
    let schema=tools.as_array().unwrap().iter().find(|t|t["name"]=="cutflow_creation_projects_create").unwrap();
    assert_eq!(schema["inputSchema"]["required"],json!(["name","preferredModel","operationId"]));
}

#[test]
fn session_ids_cannot_escape_the_generated_directory() {
    let runtime=ready_runtime();
    let mut params=request("folder","session-a","turn-folder");
    params["maxGenerationCalls"]=json!(0);
    let (job,_,cancel)=runtime.reserve("cutflow.generate",params).unwrap();
    assert_eq!(runtime.execute(&job,&cancel).unwrap_err().code,"budget_required");
    assert!(runtime.root.join("generated/project-a/session-a").join(&job.id).is_dir());
    assert_eq!(runtime.store.lock().unwrap().turns[0].job_id,job.id);
    runtime.finish(&job.id,Err(Error::new("cancelled","fixture complete")));
}

#[test]
fn continuation_reference_must_belong_to_the_same_chat_and_be_sent_explicitly() {
    let runtime=ready_runtime();
    let (original,_,_)=runtime.reserve("cutflow.generate",request("original","session-a","turn-original")).unwrap();
    runtime.finish(&original.id,Ok(json!({"assets":[{"path":"generated/original.png"}]})));
    let mut omitted=request("omitted","session-a","turn-omitted");
    omitted["continuationAssetPath"]=json!("generated/original.png");
    assert_eq!(runtime.reserve("cutflow.generate",omitted.clone()).err().unwrap().code,"invalid_reference");
    omitted["references"]=json!(["generated/original.png"]);
    omitted["sessionId"]=json!("session-b");
    assert_eq!(runtime.reserve("cutflow.generate",omitted.clone()).err().unwrap().code,"invalid_reference");
    omitted["sessionId"]=json!("session-a");
    omitted["continuationAssetPath"]=json!("another-session.png");
    assert_eq!(runtime.reserve("cutflow.generate",omitted.clone()).err().unwrap().code,"invalid_reference");
    omitted["continuationAssetPath"]=json!("generated/original.png");
    let (continued,_,_)=runtime.reserve("cutflow.generate",omitted).unwrap();
    assert_eq!(continued.params["continuationAssetPath"],"generated/original.png");
    assert_eq!(runtime.store.lock().unwrap().turns.len(),2);
    assert_eq!(runtime.dispatch("cutflow.creation.drafts.save",json!({"sessionId":"session-a","kind":"video","draft":{"selectedAssetPath":"generated/original.png"}})).unwrap_err().code,"invalid_reference");
}

#[test]
fn pending_get_uses_the_original_local_project_and_operation_ids() {
    let runtime=ready_runtime();
    {
        let mut store=runtime.store.lock().unwrap();
        store.projects[0].state="creation_unknown".into();
        store.projects[0].operation_id="original-create".into();
        store.projects[0].flow_project_id=None;
    }
    let result=json!({"version":1,"instanceId":"service-a","project":{
        "id":"project-a","flowProjectId":"recovered-google-project","state":"ready","accountLabel":"Account A"
    }});
    let (connection,worker)=local_control(vec![Some(capability("service-a")),Some(result)],None);
    // Save API configuration without invalidating the already-recorded project owner.
    runtime.dispatch("cutflow.connection.save",json!({"baseUrl":connection.base_url,"apiKey":"fixture"})).unwrap();
    let recovered=runtime.dispatch("cutflow.creation.projects.get",json!({"projectId":"project-a"})).unwrap();
    assert_eq!(recovered["state"],"ready");
    assert_eq!(recovered["flowProjectId"],"recovered-google-project");
    let requests=worker.join().unwrap();
    assert_eq!(requests[1]["op"],"projects.get");
    assert_eq!(requests[1]["projectId"],"project-a");
    assert_eq!(requests[1]["operationId"],"original-create");
}

#[test]
fn new_sessions_have_complete_drafts_and_create_replays_after_auto_rename() {
    let runtime=ready_runtime();
    let params=json!({"projectId":"project-a","title":"新聊天","operationId":"new-session-once"});
    let session=runtime.dispatch("cutflow.creation.sessions.create",params.clone()).unwrap();
    assert_eq!(session["drafts"]["image"]["references"],json!([]));
    assert_eq!(session["drafts"]["video"]["imageSize"],"auto");
    assert_eq!(session["drafts"]["image"]["independent"],false);
    let sid=session["id"].as_str().unwrap();
    runtime.reserve("cutflow.generate",request("rename",sid,"turn-rename")).unwrap();
    let replay=runtime.dispatch("cutflow.creation.sessions.create",params).unwrap();
    assert_eq!(replay["id"],session["id"]);
    assert_eq!(replay["title"],"a cat");
    assert_eq!(runtime.store.lock().unwrap().sessions.len(),3);
}

#[test]
fn model_capabilities_reject_unsupported_type_dimensions_and_references() {
    let runtime=ready_runtime();
    {
        let mut store=runtime.store.lock().unwrap();
        let mut model=flow::describe("gflow-nano-banana-2.1","");
        flow::apply_model_metadata(&mut model,&json!({"aspects":["1:1"],"imageSizes":["1K"],"maxReferences":1}));
        store.models.push(model);
    }
    let mut params=request("capability","session-a","turn-capability");
    params["kind"]=json!("video");
    assert_eq!(runtime.reserve("cutflow.generate",params.clone()).err().unwrap().code,"invalid_request");
    params["kind"]=json!("image");
    params["aspect_ratio"]=json!("16:9");
    assert_eq!(runtime.reserve("cutflow.generate",params.clone()).err().unwrap().code,"invalid_request");
    params["aspect_ratio"]=json!("1:1");
    params["image_size"]=json!("4K");
    assert_eq!(runtime.reserve("cutflow.generate",params.clone()).err().unwrap().code,"invalid_request");
    params["image_size"]=json!("1K");
    params["references"]=json!(["a.png","b.png"]);
    assert_eq!(runtime.reserve("cutflow.generate",params.clone()).err().unwrap().code,"invalid_reference");
    assert!(runtime.store.lock().unwrap().jobs.is_empty());
}

#[test]
fn fixed_account_model_limits_and_structured_project_errors_are_preserved() {
    let root=scratch();
    let runtime=Runtime::open(root.clone(),root.join("tools")).unwrap();
    let response=json!({"version":1,"instanceId":"service-a","project":{"id":"ignored","flowProjectId":"google-id","state":"ready","accountLabel":"Account A"},
        "models":[{"id":"gflow-nano-banana-2.1","kind":"image","maxReferences":3,"imageSizes":["1K"]}]});
    let (connection,worker)=local_control(vec![Some(capability("service-a")),Some(response)],None);
    runtime.dispatch("cutflow.connection.save",json!({"baseUrl":connection.base_url,"apiKey":"fixture"})).unwrap();
    let project=runtime.dispatch("cutflow.creation.projects.create",json!({"name":"bound","preferredModel":"gflow-nano-banana-2.1","operationId":"bound-models"})).unwrap();
    assert_eq!(project["models"][0]["imageSizes"],json!(["1K"]));
    let session=runtime.dispatch("cutflow.creation.sessions.create",json!({"projectId":project["id"],"operationId":"bound-session"})).unwrap();
    let mut parameters=request("bound-gen",session["id"].as_str().unwrap(),"bound-turn");
    parameters["creationProjectId"]=project["id"].clone();
    parameters["image_size"]=json!("4K");
    assert_eq!(runtime.reserve("cutflow.generate",parameters).err().unwrap().code,"invalid_request");
    worker.join().unwrap();
    let response=json!({"version":1,"instanceId":"service-a","project":{"id":"ignored","state":"failed","flowProjectId":"","error":{"code":"no_accounts","message":"没有可用 Google 账号"}}});
    let (connection,worker)=local_control(vec![Some(capability("service-a")),Some(response)],None);
    runtime.dispatch("cutflow.connection.save",json!({"baseUrl":connection.base_url,"apiKey":"fixture"})).unwrap();
    let failed=runtime.dispatch("cutflow.creation.projects.create",json!({"name":"failure","preferredModel":"gflow-nano-banana-2.1","operationId":"failure-op"})).unwrap();
    assert_eq!(failed["state"],"failed");
    assert_eq!(failed["error"],"没有可用 Google 账号");
    worker.join().unwrap();
}

#[test]
fn control_completion_errors_keep_machine_code_and_message_without_an_instance_id() {
    for code in ["invalid_request","project_service_changed","project_binding_conflict"] {
        let response=json!({"version":1,"projectBinding":true,"error":{
            "code":code,"message":"项目参数或绑定不匹配","status_code":409
        }});
        let (connection,worker)=local_control(vec![Some(response)],None);
        let result=flow::project_control(&connection,"fixture",&json!({
            "version":1,"op":"capabilities","workspaceId":uuid::Uuid::new_v4().to_string()
        })).unwrap_err();
        assert_eq!(result.code,code);
        assert_eq!(result.message,"项目参数或绑定不匹配");
        worker.join().unwrap();
    }
}
