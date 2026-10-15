use super::*;
fn temporary() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cutflow-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn restrict_sources_before_starting_a_process() {
    for input in [
        "BV1xx411c7mD",
        "av123",
        "https://www.bilibili.com/video/BV1xx411c7mD?p=2",
        "https://b23.tv/abc",
    ] {
        assert!(validate_input(input).is_ok(), "{input}");
    }
    for input in [
        "--help",
        "https://evil.com/video/abc",
        "https://www.bilibili.com.evil.com/video/abc",
        "https://www.bilibili.com@evil.com/video/a",
        "http://b23.tv/a",
        "https://www.bilibili.com/video/a\n--help",
        "https://space.bilibili.com/1",
    ] {
        assert!(validate_input(input).is_err(), "{input}");
    }
}
#[test]
fn part_selection_has_bounds_and_stable_deduplication() {
    assert_eq!(validate_parts("2,1,2").unwrap(), "2,1");
    for value in ["", "0", "1;rm", "-1", "10001", "1,"] {
        assert!(validate_parts(value).is_err());
    }
}
#[test]
fn uhd_is_actual_dimensions_including_portrait() {
    assert!(is_uhd(3840, 2160));
    assert!(is_uhd(2160, 3840));
    assert!(!is_uhd(1920, 1080));
    assert!(!is_uhd(3840, 1600));
}
#[test]
fn crash_recovery_does_not_automatically_restart_network_tasks() {
    let root = temporary();
    let mut q = Queue {
        version: 1,
        jobs: vec![],
    };
    q.enqueue("av123", "1", "120", false, root.clone(), "自己拍摄")
        .unwrap();
    q.jobs[0].status = Status::Running;
    q.save(&root).unwrap();
    let recovered = Queue::load(&root).unwrap();
    assert_eq!(recovered.jobs[0].status, Status::Paused);
    assert!(recovered.jobs[0].assets.is_empty());
}
#[test]
fn queue_atomic_replace_preserves_valid_json_on_multiple_saves() {
    let root = temporary();
    let mut q = Queue {
        version: 1,
        jobs: vec![],
    };
    q.save(&root).unwrap();
    q.enqueue("av123", "1,2", "120", false, root.clone(), "授权")
        .unwrap();
    q.save(&root).unwrap();
    assert_eq!(Queue::load(&root).unwrap().jobs.len(), 1);
    fs::write(root.join("queue.json"), b"corrupt").unwrap();
    assert!(Queue::load(&root).is_err());
}
#[test]
fn sanitized_parse_does_not_contain_network_secrets() {
    let value = serde_json::json!({"source":{"title":"测试"},"groups":[{"items":[{"parts":[{"title":"片段", "streams":[{"kind":"video","quality":{"kind":"quality","value":120},"codec":"hevc","urls":["https://secret"],"headers":[{"name":"Cookie","value":"SESSDATA=secret"}]}]}]}]}]});
    let text = summarize(&value);
    assert!(text.contains("qn120/hevc"));
    assert!(!text.contains("secret"));
    assert!(!text.contains("Cookie"));
}
#[test]
fn cancelled_work_never_starts_component() {
    let cancel = AtomicBool::new(true);
    let cmd = process::command(Path::new("definitely-not-installed"));
    assert!(process::run(cmd, &cancel, 1).unwrap_err().contains("暂停"));
}
#[test]
fn invalid_credentials_never_get_saved() {
    let root = temporary();
    assert!(credentials::save(&root, "SESSDATA=\r\nfoo").is_err());
    assert!(credentials::save(&root, "other=secret").is_err());
    assert!(!root.join("account.dpapi").exists());
}
#[cfg(windows)]
#[test]
fn dpapi_roundtrip_does_not_store_plain_cookie() {
    let root = temporary();
    credentials::save(&root, "SESSDATA=unit-test-cookie; other=x").unwrap();
    assert_eq!(
        credentials::load(&root).unwrap().unwrap(),
        "SESSDATA=unit-test-cookie; other=x"
    );
    let bytes = fs::read(root.join("account.dpapi")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("unit-test-cookie"));
    credentials::clear(&root).unwrap();
    assert!(credentials::load(&root).unwrap().is_none());
}
#[cfg(unix)]
#[test]
fn actual_ffprobe_checks_streams_and_dimensions() {
    let root = temporary();
    let tools = root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    std::os::unix::fs::symlink("/usr/bin/ffprobe", tools.join("ffprobe")).unwrap();
    let media = root.join("sample.mp4");
    let status = std::process::Command::new("/usr/bin/ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=320x180:r=30:d=0.3",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.3",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&media)
        .status()
        .unwrap();
    assert!(status.success());
    let asset = Engine { tools, data: root }
        .probe(&media, &AtomicBool::new(false))
        .unwrap();
    assert_eq!((asset.width, asset.height), (320, 180));
    assert!(asset.duration > 0.1);
    assert_eq!(asset.codec, "h264");
}

#[cfg(unix)]
fn fixture_engine(size: &str, outside: bool) -> (PathBuf, Engine, Job) {
    use std::os::unix::fs::PermissionsExt;
    let root = temporary();
    let tools = root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    std::os::unix::fs::symlink("/usr/bin/ffprobe", tools.join("ffprobe")).unwrap();
    let media = root.join("fixture.mp4");
    let source = format!("color=c=green:s={size}:r=30:d=0.1");
    let status = std::process::Command::new("/usr/bin/ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &source,
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.1",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&media)
        .status()
        .unwrap();
    assert!(status.success());
    let fixture = tools.join("bdl");
    // Simulates only the upstream CLI boundary, not the media probe.
    let script = format!(
        "#!/usr/bin/env python3\nimport sys,json,pathlib,shutil\na=sys.argv\nassert a[a.index('--missing-quality')+1]=='skip'\nassert a[a.index('--quality')+1]=='120'\np=pathlib.Path(a[a.index('--output')+1])/'final.mp4'\nshutil.copyfile({},p)\nprint(json.dumps({{'downloads':[{{'outputs':[str({})]}}]}}))\n",
        serde_json::to_string(&media).unwrap(),
        if outside {
            serde_json::to_string(&media).unwrap()
        } else {
            "p".into()
        }
    );
    fs::write(&fixture, script).unwrap();
    fs::set_permissions(&fixture, fs::Permissions::from_mode(0o755)).unwrap();
    let mut queue = Queue {
        version: 1,
        jobs: vec![],
    };
    queue
        .enqueue("av123", "1", "120", false, root.clone(), "fixture")
        .unwrap();
    let job = queue.jobs.remove(0);
    let engine = Engine {
        tools,
        data: root.clone(),
    };
    (root, engine, job)
}
#[cfg(unix)]
#[test]
fn strict_download_rejects_lower_dimensions_and_does_not_commit_manifest() {
    let (root, engine, job) = fixture_engine("320x180", false);
    let err = engine
        .download(&job, Arc::new(AtomicBool::new(false)))
        .unwrap_err();
    assert!(err.contains("严格 4K"));
    assert!(
        !root
            .join("jobs")
            .join(&job.id)
            .join("asset-manifest.json")
            .exists()
    );
}
#[cfg(unix)]
#[test]
fn strict_download_commits_a_probed_real_uhd_asset() {
    let (root, engine, job) = fixture_engine("3840x2160", false);
    let assets = engine
        .download(&job, Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert_eq!((assets[0].width, assets[0].height), (3840, 2160));
    let manifest =
        fs::read_to_string(root.join("jobs").join(&job.id).join("asset-manifest.json")).unwrap();
    assert!(manifest.contains("3840"));
    assert!(!manifest.contains("Cookie"));
}
#[cfg(unix)]
#[test]
fn handoff_refuses_paths_outside_the_owned_task_directory() {
    let (_, engine, job) = fixture_engine("320x180", true);
    assert!(
        engine
            .download(&job, Arc::new(AtomicBool::new(false)))
            .unwrap_err()
            .contains("目录外")
    );
}

fn action_queue(statuses: &[Status]) -> Queue {
    let mut q = Queue {
        version: 1,
        jobs: vec![],
    };
    for (i, status) in statuses.iter().enumerate() {
        q.enqueue(
            &format!("av{}", i + 1),
            "1",
            "120",
            false,
            PathBuf::from("project"),
            "fixture",
        )
        .unwrap();
        q.jobs[i].status = status.clone();
    }
    q
}
#[test]
fn cancelling_a_waiting_task_leaves_other_downloads_running() {
    use crate::actions::Action;
    let mut q = action_queue(&[Status::Running, Status::Queued]);
    let id = q.jobs[1].id.clone();
    q.action(&id, Action::Cancel, false).unwrap();
    assert_eq!(q.jobs[0].status, Status::Running);
    assert_eq!(q.jobs[1].status, Status::Cancelled);
}
#[test]
fn late_success_is_not_an_implicit_resume_of_a_cancelled_task() {
    use crate::actions::Action;
    let mut q = action_queue(&[Status::Running]);
    let id = q.jobs[0].id.clone();
    q.action(&id, Action::Cancel, true).unwrap();
    assert!(q.action(&id, Action::Resume, true).is_err());
    q.action(&id, Action::Resume, false).unwrap();
    assert_eq!(q.jobs[0].status, Status::Queued);
}
#[test]
fn continue_queue_never_retries_failures_or_cancelled_tasks() {
    let mut q = action_queue(&[
        Status::Paused,
        Status::Queued,
        Status::Failed,
        Status::Cancelled,
        Status::Imported,
    ]);
    assert_eq!(q.resume_waiting(), 2);
    assert_eq!(q.jobs[2].status, Status::Failed);
    assert_eq!(q.jobs[3].status, Status::Cancelled);
    assert_eq!(q.jobs[4].status, Status::Imported);
}
#[test]
fn completed_task_actions_are_guarded_in_the_backend() {
    use crate::actions::Action;
    let mut q = action_queue(&[Status::Imported]);
    let id = q.jobs[0].id.clone();
    for a in [Action::Pause, Action::Cancel, Action::Resume] {
        assert!(q.action(&id, a, false).is_err());
    }
    assert!(q.action(&id, Action::Remove, true).is_err());
    q.action(&id, Action::Remove, false).unwrap();
    assert!(q.jobs.is_empty());
}
#[test]
fn pause_all_preserves_completed_and_ready_assets() {
    let mut q = action_queue(&[
        Status::Running,
        Status::Queued,
        Status::Ready,
        Status::Imported,
        Status::Failed,
    ]);
    assert_eq!(q.pause_all(), 2);
    assert_eq!(q.jobs[2].status, Status::Ready);
    assert_eq!(q.jobs[3].status, Status::Imported);
    assert_eq!(q.jobs[4].status, Status::Failed);
}
#[test]
fn duplicate_source_url_and_bv_id_cannot_enqueue_twice() {
    let mut q = Queue {
        version: 1,
        jobs: vec![],
    };
    q.enqueue(
        "BV1xx411c7mD",
        "1,2",
        "120",
        false,
        PathBuf::from("project"),
        "fixture",
    )
    .unwrap();
    assert!(
        q.enqueue(
            "https://www.bilibili.com/video/BV1xx411c7mD?p=1",
            "2,1",
            "120",
            false,
            PathBuf::from("project"),
            "fixture"
        )
        .is_err()
    );
    assert!(
        q.enqueue(
            "BV1xx411c7mD",
            "1,2",
            "80",
            false,
            PathBuf::from("project"),
            "fixture"
        )
        .is_ok()
    );
}
#[test]
fn filters_do_not_confuse_no_results_with_no_tasks() {
    let q = action_queue(&[Status::Ready, Status::Failed]);
    assert!(actions::matches_filter(&q.jobs[0], 2, "av1"));
    assert!(!actions::matches_filter(&q.jobs[1], 2, ""));
    assert!(!actions::matches_filter(&q.jobs[0], 0, "does-not-match"));
}
#[test]
fn removing_a_record_never_deletes_a_media_file() {
    let root = temporary();
    let media = root.join("keep.mp4");
    fs::write(&media, b"keep").unwrap();
    let mut q = action_queue(&[Status::Cancelled]);
    let id = q.jobs[0].id.clone();
    q.jobs[0].assets.push(Asset {
        path: media.clone(),
        bytes: 4,
        width: 1,
        height: 1,
        duration: 1.0,
        codec: "test".into(),
        frame_rate: "1/1".into(),
        imported: false,
    });
    q.action(&id, actions::Action::Remove, false).unwrap();
    assert_eq!(fs::read(media).unwrap(), b"keep");
}
#[test]
fn new_default_queue_uses_the_supported_storage_version() {
    let root = temporary();
    Queue::default().save(&root).unwrap();
    assert_eq!(Queue::load(&root).unwrap().version, 1);
}
