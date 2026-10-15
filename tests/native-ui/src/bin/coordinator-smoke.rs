//! Exercises production callbacks/coordinator with a simulated media host.
//! No Bilibili requests, real editor media decoding or Windows execution.
use cutflow_download::{Asset, Queue, Status};
use cutflow_native_ui_check::{
    downloads::Downloads,
    host::{self, Shell, Studio},
    ui::{App, Theme},
};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
struct Headless {
    window: Rc<MinimalSoftwareWindow>,
    start: std::time::Instant,
}
impl slint::platform::Platform for Headless {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }
}
fn pump() {
    for _ in 0..8 {
        std::thread::sleep(Duration::from_millis(100));
        slint::platform::update_timers_and_animations();
    }
}
fn read(root: &Path) -> Queue {
    serde_json::from_slice(&std::fs::read(root.join("queue.json")).unwrap()).unwrap()
}
#[cfg(unix)]
fn main() {
    let root = std::env::temp_dir().join(format!(
        "cutflow-ui-smoke-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("CutFlow");
    let a = root.join("project-a");
    let b = root.join("project-b");
    for p in [&data, &a, &b] {
        std::fs::create_dir_all(p).unwrap();
    }
    // Single-threaded initialization before the coordinator spawns any workers.
    unsafe {
        std::env::set_var("LOCALAPPDATA", &root);
    }
    let tools = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    #[cfg(unix)]
    for (name, target) in [
        ("bdl", "/usr/bin/true"),
        ("ffmpeg", "/usr/bin/ffmpeg"),
        ("ffprobe", "/usr/bin/ffprobe"),
    ] {
        let path = tools.join(name);
        if !path.exists() {
            std::os::unix::fs::symlink(target, path).unwrap();
        }
    }
    let mut q = Queue {
        version: 1,
        jobs: vec![],
    };
    for (input, target) in [("av1", &a), ("av2", &b), ("av3", &b)] {
        q.enqueue(input, "1", "120", false, target.clone(), "fixture")
            .unwrap();
        let job = q.jobs.last_mut().unwrap();
        job.status = Status::Ready;
        // Two B jobs reference the same media, deliberately testing import replay.
        let file = target.join("sample.mp4");
        std::fs::write(&file, b"fixture").unwrap();
        job.assets.push(Asset {
            path: file,
            bytes: 7,
            width: 3840,
            height: 2160,
            duration: 1.0,
            codec: "fixture".into(),
            frame_rate: "30/1".into(),
            imported: false,
        });
    }
    let id_a = q.jobs[0].id.clone();
    let id_b = q.jobs[1].id.clone();
    q.save(&data).unwrap();
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless {
        window,
        start: std::time::Instant::now(),
    }))
    .unwrap();
    let app = App::new().unwrap();
    app.global::<Theme>().set_dark(true);
    let mut studio = Studio::default();
    studio
        .open_project(concat_host::projects::open(a.to_str().unwrap()).unwrap())
        .unwrap();
    let shell = Rc::new(Shell {
        studio: RefCell::new(studio),
        models: (),
    });
    Shell::install(shell.clone());
    host::install_app(&app);
    let service = Downloads::install(&app);
    app.show().unwrap();
    app.invoke_download_open();
    pump();
    assert!(app.get_download_opened());
    assert_eq!(shell.studio.borrow().project().media.len(), 1);
    assert_eq!(read(&data).jobs[0].status, Status::Imported);
    assert_eq!(read(&data).jobs[1].status, Status::Ready);
    println!("PASS open-panel callback, current-project import, foreign-project isolation");
    app.invoke_download_switch_project(id_b.clone().into());
    pump();
    pump();
    assert_eq!(shell.studio.borrow().project().media.len(), 1);
    assert!(
        read(&data)
            .jobs
            .iter()
            .all(|j| j.status == Status::Imported)
    );
    println!("PASS save-before-switch, bound-project import, duplicate handoff reconciliation");
    app.invoke_download_form_changed("av101".into(), "0".into(), 0, false, "fixture".into());
    assert!(!app.get_download_form_valid());
    app.invoke_download_form_changed("av101".into(), "1,2".into(), 0, false, "fixture".into());
    assert!(app.get_download_form_valid());
    println!("PASS live form validation and submit availability");
    app.invoke_download_enqueue("av101".into(), "1".into(), 0, false, "fixture".into());
    app.invoke_download_enqueue("av102".into(), "1".into(), 0, false, "fixture".into());
    let q = read(&data);
    let p = q
        .jobs
        .iter()
        .find(|j| j.input == "av101")
        .unwrap()
        .id
        .clone();
    let other = q
        .jobs
        .iter()
        .find(|j| j.input == "av102")
        .unwrap()
        .id
        .clone();
    app.invoke_download_cancel(p.clone().into());
    assert_eq!(
        read(&data)
            .jobs
            .iter()
            .find(|j| j.id == other)
            .unwrap()
            .status,
        Status::Queued
    );
    app.invoke_download_pause(other.clone().into());
    pump();
    app.invoke_download_resume(other.clone().into());
    app.invoke_download_pause_all();
    assert_eq!(
        read(&data)
            .jobs
            .iter()
            .find(|j| j.id == other)
            .unwrap()
            .status,
        Status::Paused
    );
    println!("PASS enqueue, single cancel, pause, resume, pause-all callback persistence");
    app.invoke_download_resume_all();
    app.invoke_download_pause_all();
    pump();
    app.invoke_download_resume(id_a.clone().into());
    pump();
    assert_eq!(read(&data).jobs[0].status, Status::Imported);
    assert_eq!(app.get_download_message_tone(), 2);
    println!("PASS continue-queue callback and invalid-action error feedback");
    app.invoke_download_filter_changed(2, "does-not-exist".into());
    pump();
    assert_eq!(app.get_download_jobs().row_count(), 0);
    app.invoke_download_filter_changed(0, "".into());
    pump();
    assert_eq!(app.get_download_jobs().row_count(), 5);
    app.invoke_download_remove(p.into());
    pump();
    assert_eq!(read(&data).jobs.len(), 4);
    assert!(a.join("sample.mp4").exists());
    println!("PASS task filters, search reset, remove-record preserves media");
    app.invoke_download_parse("av1".into());
    app.invoke_download_stop_parse();
    pump();
    assert!(!app.get_download_busy());
    #[cfg(unix)]
    {
        app.invoke_download_check_tools();
        pump();
        assert!(app.get_download_tools().contains("ffprobe"));
    }
    app.invoke_download_close();
    assert!(!app.get_download_opened());
    println!("PASS cancel-parse, local component checks, return-to-editor callback");
    // A save failure refuses project switching and retains the current session.
    std::fs::write(b.join("fail-save"), b"test").unwrap();
    app.invoke_download_switch_project(id_a.into());
    pump();
    assert_eq!(
        shell.studio.borrow().session.as_ref().unwrap().path(),
        b.to_str().unwrap()
    );
    assert_eq!(app.get_download_message_tone(), 2);
    app.invoke_download_open();
    app.invoke_download_choose_project(true);
    assert!(app.get_download_opened());
    println!("PASS save-failure blocks project switching and new-project navigation");
    service.shutdown();
    println!("All coordinator/UI callback smoke scenarios passed.");
}

#[cfg(not(unix))]
fn main() {
    println!(
        "The coordinator smoke fixture runs on Unix; Windows behavior still needs native acceptance tests."
    );
}
