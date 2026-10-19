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
    assert_eq!(flow::describe("my-custom-model", "").kind, "unknown");
    assert_eq!(flow::describe("Nano-Banana-Pro", "").kind, "image");
}

#[test]
fn non_streaming_and_structured_chat_media_are_supported() {
    let (connection, worker) = mock(200, "application/json", json!({"choices":[{"message":{"content":[{"type":"image_url","image_url":{"url":"https://cdn.example/image.png"}}]}}]}).to_string());
    let output = flow::generate(&connection, "test-key", &json!({"model":"custom", "prompt":"test", "stream":false}), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(output, "https://cdn.example/image.png");
    assert!(worker.join().unwrap().contains("\"stream\":false"));
    let mut plain = StreamResult::default();
    plain.event(&json!({"choices":[{"message":{"content":"https://cdn.example/movie.mp4?token=public"}}]}).to_string()).unwrap();
    assert_eq!(plain.media_url().unwrap(), "https://cdn.example/movie.mp4?token=public");
}

#[test]
fn stream_without_done_is_unknown_even_if_a_media_url_arrived() {
    let body = format!("data: {}\n\n", json!({"choices":[{"delta":{"content":"![result](https://cdn.example/image.png)"}}]}));
    let (connection, worker) = mock(200, "text/event-stream", body);
    let result = flow::generate(&connection, "key", &json!({"model":"custom","prompt":"test"}), &AtomicBool::new(false), |_| {});
    assert_eq!(result.unwrap_err().code, "result_unknown");
    worker.join().unwrap();
}

#[test]
fn generation_without_project_uses_public_assets_and_propagates_quota_error() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    let (connection, worker) = mock(402, "application/json", "{}".into());
    runtime.dispatch("cutflow.connection.save", json!({"baseUrl":connection.base_url,"apiKey":"test-key"})).unwrap();
    let (job, _, cancel) = runtime.reserve("cutflow.generate", json!({"operationId":"no-project","kind":"image","model":"custom","prompt":"test","maxGenerationCalls":1})).unwrap();
    assert_eq!(runtime.execute(&job, &cancel).unwrap_err().code, "insufficient_credit");
    assert!(root.join("generated").join(&job.id).is_dir());
    worker.join().unwrap();
    assert_eq!(runtime.dispatch("cutflow.jobs.retryDownload", json!({"jobId":job.id})).unwrap_err().code, "invalid_request");
}

#[test]
fn main_and_editor_have_separate_locks_and_control_discovery() {
    let root = scratch();
    let main = Runtime::open(root.clone(), root.join("tools")).unwrap();
    let editor = Runtime::open(root.join("editor"), root.join("tools")).unwrap();
    let server = Server::start(&editor.root, Arc::new(|_| Ok(json!({"running":true})))).unwrap();
    assert_eq!(server::call(&main.root, &Call { method:"cutflow.editor.status".into(), params:json!({}) }).unwrap()["running"], true);
    assert!(server::call(&main.root, &Call { method:"cutflow.generate".into(), params:json!({}) }).is_err());
    drop(server);
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


#[test]
fn cancellation_interrupts_a_provider_that_sends_no_bytes() {
    use std::{sync::atomic::Ordering, time::{Duration, Instant}};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let connection = Connection { base_url: format!("http://{}/v1", listener.local_addr().unwrap()) };
    let cancel = Arc::new(AtomicBool::new(false));
    let server_cancel = cancel.clone();
    let server = std::thread::spawn(move || {
        let (_socket, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_millis(150));
        server_cancel.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(300));
    });
    let began = Instant::now();
    let error = flow::generate(&connection, "secret", &json!({"model":"custom","prompt":"test"}), &cancel, |_| {}).unwrap_err();
    assert_eq!(error.code, "cancelled");
    assert!(began.elapsed() < Duration::from_secs(1));
    server.join().unwrap();
}

#[test]
fn generation_options_reach_the_provider_and_text_models_are_separate() {
    let (connection, server) = mock(200, "application/json", json!({"url":"https://cdn.example/a.png"}).to_string());
    flow::generate(&connection, "secret", &json!({"model":"gflow-nano-banana-2.1","prompt":"test","aspect_ratio":"3:4","image_size":"4K","stream":false}), &AtomicBool::new(false), |_| {}).unwrap();
    let raw = server.join().unwrap();
    let body: serde_json::Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["generationConfig"]["imageConfig"]["aspectRatio"], "3:4");
    assert_eq!(body["generationConfig"]["imageConfig"]["imageSize"], "4K");
    assert_eq!(flow::describe("gflow-omni-1.1-flash", "").kind, "video");
    assert_eq!(flow::describe("gpt-6-luna", "").kind, "text");
}

#[test]
fn inline_generated_images_can_be_saved_and_validated() {
    use base64::Engine;
    let root = scratch();
    let bytes = b"\x89PNG\r\n\x1a\nfixture";
    let url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
    let path = flow::download(&Connection { base_url:"http://localhost/v1".into() }, &url, &root.join("image.bin"), &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut result = StreamResult::default();
    result.event(&json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(bytes)}]}).to_string()).unwrap();
    assert_eq!(result.media_url().unwrap(), url);
}

#[test]
fn model_listing_is_not_treated_as_verified_response() {
    let (connection, server) = mock(200, "application/json", json!({"choices":[{"message":{"content":"OK"}}]}).to_string());
    let result = flow::check_response(&connection, "secret", "gpt-6-luna").unwrap();
    assert_eq!(result["verified"], true);
    assert!(server.join().unwrap().starts_with("POST /v1/chat/completions"));
    let (connection, server) = mock(200, "application/json", json!({"data":[{"id":"gpt-6-luna"}]}).to_string());
    assert!(flow::check_response(&connection, "secret", "gpt-6-luna").is_err());
    server.join().unwrap();
}


#[test]
fn openai_images_use_the_image_endpoint_and_reference_edits_use_multipart() {
    let (connection, server) = mock(200, "application/json", json!({"data":[{"url":"https://cdn.example/image.png"}]}).to_string());
    flow::generate(&connection, "key", &json!({"model":"gpt-image-1.5","prompt":"circle","aspect_ratio":"1:1","image_size":"low"}), &AtomicBool::new(false), |_| {}).unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v1/images/generations"));
    assert!(request.contains("\"quality\":\"low\""));
    let root = scratch();
    let reference = root.join("reference.png");
    std::fs::write(&reference, b"\x89PNG\r\n\x1a\nfixture").unwrap();
    let (connection, server) = mock(200, "application/json", json!({"data":[{"url":"https://cdn.example/image.png"}]}).to_string());
    flow::generate(&connection, "key", &json!({"model":"gpt-image-1.5","prompt":"circle","references":[reference]}), &AtomicBool::new(false), |_| {}).unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v1/images/edits"));
    assert!(request.contains("multipart/form-data"));
    assert!(request.contains("name=\"image[]\""));
}

#[test]
fn download_retry_requires_an_unsaved_captured_media_result() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    let (job, _, _) = runtime.reserve("cutflow.generate", json!({
        "operationId":"retry-eligibility","kind":"image","model":"custom","prompt":"test","maxGenerationCalls":1
    })).unwrap();
    runtime.finish(&job.id, Err(cutflow_workbench::Error::new("captcha_failed", "No token")));
    assert_eq!(runtime.snapshot()["jobs"][0]["canRetryDownload"], false);
    let folder = root.join("generated").join(&job.id);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("generation-result.json"), br#"{"urls":["https://cdn.example/image.png"]}"#).unwrap();
    assert_eq!(runtime.snapshot()["jobs"][0]["canRetryDownload"], true);
    let image = folder.join("generated-0.png");
    std::fs::write(&image, b"saved").unwrap();
    runtime.store.lock().unwrap().jobs[0].result = Some(json!({"assets":[{"path":image}]}));
    assert_eq!(runtime.snapshot()["jobs"][0]["canRetryDownload"], false);
    assert!(runtime.dispatch("cutflow.jobs.retryDownload", json!({"jobId":job.id})).is_err());
}


#[test]
fn structured_stage_progress_is_safe_and_kept_separate_from_media() {
    let mut stream = StreamResult::default();
    stream.event(&json!({"gflow_progress":{"stage":"generating","percent":43,"basis":"stage"}}).to_string()).unwrap();
    let progress = stream.progress.clone().unwrap();
    assert_eq!(progress.stage, "正在生成");
    assert_eq!(progress.percent, Some(43));
    assert_eq!(progress.basis, "stage");
    assert!(stream.content.is_empty());
    assert!(stream.url.is_none());
    stream.event(&json!({"choices":[{"delta":{"gflow_progress":{"stage":"upsampling_4k","percent":82,"basis":"stage"}}}]}).to_string()).unwrap();
    assert_eq!(stream.progress.as_ref().unwrap().stage, "正在放大至 4K");
    assert_eq!(stream.progress.as_ref().unwrap().percent, Some(82));

    // Unknown/out-of-range progress cannot become a fake numerical percentage.
    stream.event(&json!({"gflow_progress":{"stage":"准备放大","percent":101,"basis":"stage"}}).to_string()).unwrap();
    assert_eq!(stream.progress.as_ref().unwrap().percent, None);
    let previous = stream.progress.clone();
    for text in ["https://cdn.example/image.png?token=secret", "data:image/png;base64,aGVsbG8=", "Authorization: Bearer secret", "api_key=secret"] {
        stream.event(&json!({"gflow_progress":{"stage":text,"percent":99,"basis":"stage"}}).to_string()).unwrap();
        assert_eq!(stream.progress, previous);
    }
    stream.event(&json!({"gflow_progress":{"stage":"fake","percent":99,"basis":"elapsed"}}).to_string()).unwrap();
    assert_eq!(stream.progress, previous);
}

#[test]
fn streamed_media_chunks_reassemble_without_a_large_single_event() {
    use base64::Engine;
    let bytes = b"synthetic-large-media";
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut stream = StreamResult::default();
    stream.event(&json!({"gflow_media":{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa_2K.jpg","mime":"image/jpeg","encoding":"base64","chunk":encoded,"done":false}}).to_string()).unwrap();
    assert!(stream.url.is_none());
    stream.event(&json!({"gflow_media":{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa_2K.jpg","mime":"image/jpeg","encoding":"base64","chunk":"","done":true}}).to_string()).unwrap();
    assert_eq!(stream.media_url().unwrap(), format!("data:image/jpeg;base64,{encoded}"));
    assert!(stream.event(&json!({"gflow_media":{"id":"../secret.jpg","mime":"image/jpeg","chunk":"","done":true}}).to_string()).is_err());
}

#[test]
fn generation_callback_only_receives_sanitized_stage_progress() {
    let events = [
        json!({"choices":[{"delta":{"reasoning_content":"正在生成图片"}}]}),
        json!({"choices":[{"delta":{"reasoning_content":"https://cdn.example/image.png?token=secret"}}]}),
        json!({"gflow_progress":{"stage":"准备放大 fixture-key","percent":72,"basis":"stage"}}),
        json!({"choices":[{"delta":{"content":"![result](https://cdn.example/image.png)"}}]}),
    ];
    let body = events.iter().map(|event| format!("data: {event}\n\n")).collect::<String>() + "data: [DONE]\n\n";
    let (connection, worker) = mock(200, "text/event-stream", body);
    let mut progress = Vec::new();
    let result = flow::generate(&connection, "fixture-key", &json!({"model":"custom","prompt":"test"}), &AtomicBool::new(false), |update| progress.push(update)).unwrap();
    assert_eq!(result, "https://cdn.example/image.png");
    assert_eq!(progress.len(), 2);
    assert_eq!(progress[0].stage, "正在生成图片");
    assert_eq!(progress[0].percent, None);
    assert_eq!(progress[1].percent, Some(72));
    assert_eq!(progress[1].stage, "准备放大 [REDACTED_SECRET]");
    assert!(progress.iter().all(|update| !update.stage.contains("https://") && !update.stage.contains("fixture-key")));
    worker.join().unwrap();
}

#[test]
fn a_stream_failure_does_not_erase_the_last_observed_stage() {
    let body = format!("data: {}\n\ndata: {}\n\n",
        json!({"gflow_progress":{"stage":"正在生成","percent":61,"basis":"stage"}}),
        json!({"error":{"message":"provider unavailable"}}));
    let (connection, worker) = mock(200, "text/event-stream", body);
    let mut progress = Vec::new();
    let error = flow::generate(&connection, "fixture", &json!({"model":"custom","prompt":"test"}), &AtomicBool::new(false), |update| progress.push(update)).unwrap_err();
    assert_eq!(error.code, "provider_error");
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].percent, Some(61));
    worker.join().unwrap();
}

#[test]
fn media_download_errors_preserve_http_status_and_offer_download_only_retry() {
    let root = scratch();
    let (connection, worker) = mock(403, "application/json", "{}".into());
    let error = flow::download(&connection, &connection.base_url.replace("/v1", "/image.png"), &root.join("image.png"), &AtomicBool::new(false)).unwrap_err();
    assert_eq!(error.code, "download_error");
    assert!(error.message.contains("HTTP 403"));
    assert!(error.message.contains("只重试下载"));
    assert!(!error.message.contains("http://"));
    worker.join().unwrap();
}


#[test]
fn download_only_retry_uses_receipt_and_keeps_download_stage_on_failure() {
    let root = scratch();
    let runtime = Runtime::open(root.clone(), root.join("tools")).unwrap();
    let (connection, worker) = mock(403, "application/json", "{}".into());
    let (job, _, _) = runtime.reserve("cutflow.generate", json!({
        "operationId":"download-only","kind":"image","model":"custom","n":2
    })).unwrap();
    let folder = root.join("generated").join(&job.id);
    std::fs::create_dir_all(&folder).unwrap();
    let receipt = json!({"urls":[connection.base_url.replace("/v1", "/image.png")],"connection":connection});
    cutflow_workbench::atomic_json(&folder.join("generation-result.json"), &receipt).unwrap();
    runtime.finish(&job.id, Err(cutflow_workbench::Error::new("download_error", "原图下载失败")));
    runtime.dispatch("cutflow.jobs.retryDownload", json!({"jobId":job.id})).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let view = loop {
        let view = runtime.dispatch("cutflow.jobs.get", json!({"jobId":job.id})).unwrap();
        if view["status"] != "running" { break view; }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(view["status"], "failed");
    assert_eq!(view["progress"]["stage"], "只重试下载已有结果");
    assert_eq!(view["progress"]["percent"], 92);
    assert!(view["error"]["message"].as_str().unwrap().contains("HTTP 403"));
    assert!(view["canRetryDownload"].as_bool().unwrap());
    assert!(worker.join().unwrap().starts_with("GET /image.png "));
}
