use cutflow_workbench::{
    flow::{self, Connection, StreamResult},
    protocol::Call,
    server::{self, Server},
    tasks::Runtime,
    workflow::TaskSpec,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cutflow-contract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn mock(
    status: u16,
    content_type: &str,
    body: String,
) -> (Connection, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let connection = Connection {
        base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
    };
    let content_type = content_type.to_string();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                let size = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + size {
                    break;
                }
            }
        }
        write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        String::from_utf8_lossy(&request).into_owned()
    });
    (connection, worker)
}

#[test]
fn model_metadata_and_base_urls() {
    assert_eq!(
        flow::base_url("http://127.0.0.1:8000/").unwrap(),
        "http://127.0.0.1:8000/v1"
    );
    assert!(flow::base_url("https://secret@example.com").is_err());
    assert!(flow::base_url("file:///tmp").is_err());
    assert_eq!(
        flow::describe("veo_3_1_i2v_fast_portrait_6s", "").duration,
        Some(6.0)
    );
    assert_eq!(flow::describe("gemini-3.1-flash-image", "").kind, "image");
    assert_eq!(flow::describe("veo_3_1_t2v_fast", "").max_references, 0);
}

#[test]
fn stream_accumulates_split_media_and_rejects_errors() {
    let mut stream = StreamResult::default();
    for part in ["<video src=\"https://cdn.example/", "clip.mp4\"></video>"] {
        stream
            .event(&json!({"choices":[{"delta":{"content":part}}]}).to_string())
            .unwrap();
    }
    assert_eq!(stream.media_url().unwrap(), "https://cdn.example/clip.mp4");
    assert!(stream.event(r#"{"error":{"message":"credits"}}"#).is_err());
    assert_eq!(stream.event("invalid").unwrap_err().code, "result_unknown");
}

#[test]
fn openai_models_and_multimodal_stream_contract() {
    let (connection, worker) = mock(
        200,
        "application/json",
        json!({"data":[{"id":"gemini-3.1-flash-image"},{"id":"veo_3_1_t2v_fast"}]}).to_string(),
    );
    assert_eq!(flow::models(&connection, "test-key").unwrap().len(), 2);
    assert!(worker.join().unwrap().contains("GET /v1/models"));
    let reference = scratch().join("reference.png");
    std::fs::write(&reference, b"\x89PNG\r\n\x1a\nfixture").unwrap();
    let body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"delta":{"content":"![result](https://cdn.example/image.png)"}}]})
    );
    let (connection, worker) = mock(200, "text/event-stream", body);
    let request =
        json!({"model":"gemini-3.1-flash-image","prompt":"蓝色天空","references":[reference]});
    assert_eq!(
        flow::generate(
            &connection,
            "test-key",
            &request,
            &AtomicBool::new(false),
            |_| {}
        )
        .unwrap(),
        "https://cdn.example/image.png"
    );
    let received = worker.join().unwrap();
    assert!(received.contains("POST /v1/chat/completions"));
    assert!(received.contains("data:image/png;base64,"));
    assert!(received.contains("\"stream\":true"));
}

#[test]
fn missing_result_is_unknown_and_credit_error_is_explicit() {
    let (connection, worker) = mock(
        200,
        "text/event-stream",
        "data: {\"choices\":[{\"delta\":{\"content\":\"processing\"}}]}\n\n".into(),
    );
    let request = json!({"model":"veo_3_1_t2v_fast","prompt":"test"});
    assert_eq!(
        flow::generate(
            &connection,
            "test",
            &request,
            &AtomicBool::new(false),
            |_| {}
        )
        .unwrap_err()
        .code,
        "result_unknown"
    );
    worker.join().unwrap();
    let (connection, worker) = mock(402, "application/json", "{}".into());
    assert_eq!(
        flow::generate(
            &connection,
            "test",
            &request,
            &AtomicBool::new(false),
            |_| {}
        )
        .unwrap_err()
        .code,
        "insufficient_credit"
    );
    worker.join().unwrap();
}

#[test]
fn generation_is_cancelled_before_submission() {
    let connection = Connection {
        base_url: "http://127.0.0.1:1/v1".into(),
    };
    let request = json!({"model":"veo_3_1_t2v_fast","prompt":"test"});
    assert_eq!(
        flow::generate(
            &connection,
            "test",
            &request,
            &AtomicBool::new(true),
            |_| {}
        )
        .unwrap_err()
        .code,
        "cancelled"
    );
}

#[test]
fn media_download_never_forwards_api_key_and_rejects_error_page() {
    let root = scratch();
    let (connection, worker) = mock(200, "image/png", "binary-fixture".into());
    let url = connection.base_url.replace("/v1", "/image.png");
    flow::download(
        &connection,
        &url,
        &root.join("image.png"),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        !worker
            .join()
            .unwrap()
            .to_lowercase()
            .contains("authorization")
    );
    let (connection, worker) = mock(200, "text/html", "<html>error</html>".into());
    assert_eq!(
        flow::download(
            &connection,
            &connection.base_url,
            &root.join("bad.png"),
            &AtomicBool::new(false)
        )
        .unwrap_err()
        .code,
        "invalid_media"
    );
    worker.join().unwrap();
}

#[test]
fn jobs_are_idempotent_and_restart_never_retries_generation() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    let params = json!({"operationId":"same","model":"test"});
    let (job, fresh, _) = runtime.reserve("cutflow.generate", params.clone()).unwrap();
    assert!(fresh);
    assert!(
        !runtime
            .reserve("cutflow.generate", params.clone())
            .unwrap()
            .1
    );
    assert_eq!(
        runtime
            .reserve(
                "cutflow.generate",
                json!({"operationId":"same","model":"other"})
            )
            .err()
            .unwrap()
            .code,
        "operation_conflict"
    );
    runtime.update(&job.id, "generating");
    drop(runtime);
    let restarted = Runtime::open(root.clone(), root.join("tools")).unwrap();
    assert_eq!(
        restarted
            .existing("cutflow.generate", &params)
            .unwrap()
            .unwrap()
            .status,
        "result_unknown"
    );
    assert!(!restarted.reserve("cutflow.generate", params).unwrap().1);
}

#[test]
fn corrupted_store_is_preserved_and_blocks_new_work() {
    let root = scratch();
    std::fs::write(root.join("workbench.json"), b"broken").unwrap();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    assert!(
        runtime
            .reserve("cutflow.generate", json!({"operationId":"x"}))
            .is_err()
    );
    assert_eq!(
        std::fs::read(root.join("workbench.json")).unwrap(),
        b"broken"
    );
}

#[test]
fn task_spec_enforces_budget_unique_scenes_and_draft_boundary() {
    let mut input = json!({"version":1,"projectPath":"D:/project","operationId":"draft","expectedRevision":1,"scenes":[{"id":"one","source":{"kind":"generate","model":"test","prompt":"test","mediaKind":"video"}}]});
    let spec: TaskSpec = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(spec.validate().unwrap_err().code, "budget_exceeded");
    input["maxGenerationCalls"] = json!(1);
    assert!(
        serde_json::from_value::<TaskSpec>(input.clone())
            .unwrap()
            .validate()
            .is_ok()
    );
    input["stopAt"] = json!("export");
    assert!(
        serde_json::from_value::<TaskSpec>(input.clone())
            .unwrap()
            .validate()
            .is_err()
    );
    input["stopAt"] = json!("draft");
    let duplicate = input["scenes"][0].clone();
    input["scenes"].as_array_mut().unwrap().push(duplicate);
    assert!(
        serde_json::from_value::<TaskSpec>(input)
            .unwrap()
            .validate()
            .is_err()
    );
}

#[test]
fn local_control_requires_token_and_bridge_exit_keeps_server_running() {
    let root = scratch();
    let server = Server::start(&root, Arc::new(|call| Ok(json!({"method":call.method})))).unwrap();
    let call = Call {
        method: "cutflow.project.current".into(),
        params: json!({}),
    };
    assert_eq!(server::call(&root, &call).unwrap()["method"], call.method);
    cutflow_workbench::secret(&root, "automation-token", Some("wrong-token")).unwrap();
    assert_eq!(server::call(&root, &call).unwrap_err().code, "unauthorized");
    drop(server);
}

#[test]
fn reference_signature_is_validated_before_upload() {
    let path = scratch().join("fake.png");
    std::fs::write(&path, b"not an image").unwrap();
    assert!(flow::reference_data(&path).is_err());
}

#[test]
fn chinese_subtitle_chunks_are_bounded_and_complete() {
    let text = "你好。世界！这是中文配音。".repeat(30);
    let chunks = cutflow_workbench::workflow::sentences(&text);
    assert!(chunks.iter().all(|part| part.chars().count() <= 60));
    assert_eq!(chunks.join(""), text);
}

#[test]
fn real_engine_draft_uses_source_trim_and_one_undo_step() {
    use concat_project::{Command, Editor, commands::NewMedia};
    let media:NewMedia=serde_json::from_value(json!({"path":"D:/fixture.mp4","name":"fixture","duration":20.0,"kind":"video","width":1920,"height":1080,"frameRate":30.0,"hasAudio":true})).unwrap();
    let mut editor = Editor::new();
    let mut trial = editor.clone();
    let mut commands = Vec::new();
    let clip_id = cutflow_workbench::timeline::append_media(
        &mut trial,
        &mut commands,
        media,
        0.0,
        4.0,
        3.0,
        true,
        true,
    )
    .unwrap()
    .unwrap();
    let clip = trial.project().active().clip(&clip_id).unwrap();
    assert_eq!(clip.source_start, 3.0);
    assert_eq!(clip.duration, 4.0);
    assert_eq!(clip.start, 0.0);
    assert_eq!(clip.muted, Some(true));
    assert!(editor.project().media.is_empty());
    editor.apply(Command::Batch { commands }).unwrap();
    assert_eq!(editor.project(), trial.project());
    editor.undo();
    assert!(editor.project().active().clips.is_empty());
    assert!(editor.project().media.is_empty());
    editor.redo();
    assert_eq!(editor.project().active().clips.len(), 1);
}

#[test]
fn one_portable_store_has_one_writer() {
    let root = scratch();
    let first = Runtime::open(root.clone(), root.join("tools")).unwrap();
    assert_eq!(
        Runtime::open(root.clone(), root.join("tools"))
            .err()
            .unwrap()
            .code,
        "busy"
    );
    drop(first);
    assert!(Runtime::open(root.clone(), root.join("tools")).is_ok());
}

#[test]
fn reserve_failure_does_not_leave_a_running_job() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    let file = root.join("workbench.json");
    let backup = root.join("saved.json");
    std::fs::rename(&file, &backup).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(
        runtime
            .reserve("cutflow.generate", json!({"operationId":"fail"}))
            .is_err()
    );
    assert!(runtime.store.lock().unwrap().jobs.is_empty());
}

#[test]
fn mcp_schemas_require_revision_and_expose_versioned_scenes() {
    let tools = cutflow_workbench::protocol::tools();
    let rows = tools.as_array().unwrap();
    let edit = rows
        .iter()
        .find(|tool| tool["name"] == "cutflow_edit_apply")
        .unwrap();
    assert!(
        edit["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("expectedRevision"))
    );
    assert!(edit["inputSchema"]["properties"].get("apiKey").is_none());
    let workflow = rows
        .iter()
        .find(|tool| tool["name"] == "cutflow_workflow_run")
        .unwrap();
    assert_eq!(
        workflow["inputSchema"]["properties"]["scenes"]["items"]["properties"]["source"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn local_assets_are_project_owned_and_reimports_do_not_duplicate() {
    let root=scratch();let project=root.join("project");std::fs::create_dir(&project).unwrap();
    let source=root.join("source.mp4");std::fs::write(&source,b"media-fixture").unwrap();
    let target=cutflow_workbench::media::copy_into_project(&project,&project.join("assets/local"),&source).unwrap();
    assert!(target.starts_with(project.canonicalize().unwrap()));
    assert_eq!(target,cutflow_workbench::media::copy_into_project(&project,&project.join("assets/local"),&source).unwrap());
    std::fs::write(&target,b"changed").unwrap();
    assert!(cutflow_workbench::media::copy_into_project(&project,&project.join("assets/local"),&source).is_err());
    assert_eq!(std::fs::read(&source).unwrap(),b"media-fixture");
}
