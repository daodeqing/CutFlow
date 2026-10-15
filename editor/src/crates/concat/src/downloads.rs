// SPDX-License-Identifier: AGPL-3.0-or-later
//! Native task UI. The event-loop thread owns the queue; workers own I/O.
use crate::{
    host::Shell,
    ui::{App, DownloadJobData},
};
use concat_host::media::{self, MediaSummary};
use concat_project::Command;
use cutflow_download::{
    Asset, Engine, Queue, Status,
    actions::{self, Action},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

#[derive(Clone, PartialEq)]
enum Work {
    Parse,
    Download(String),
    Import(String),
    Check,
}
impl Work {
    fn id(&self) -> Option<&str> {
        match self {
            Self::Download(id) | Self::Import(id) => Some(id),
            _ => None,
        }
    }
}
enum Done {
    Parsed(cutflow_download::Result<String>),
    Downloaded(String, cutflow_download::Result<Vec<Asset>>),
    Probed(String, Vec<cutflow_download::Result<MediaSummary>>),
    Checked(cutflow_download::Result<String>),
    Crashed,
}
struct Active {
    kind: Work,
    cancel: Arc<AtomicBool>,
    worker: thread::JoinHandle<()>,
    started: Instant,
}
#[derive(Default)]
struct Form {
    input: String,
    parts: String,
    quality: i32,
    rights: String,
}
struct State {
    queue: Queue,
    engine: Engine,
    tx: mpsc::Sender<Done>,
    rx: mpsc::Receiver<Done>,
    active: Option<Active>,
    autorun: bool,
    fault: Option<String>,
    message: String,
    tone: i32,
    tools: String,
    tools_ready: bool,
    filter: i32,
    query: String,
    form: Form,
    current: Option<PathBuf>,
    rows: Rc<VecModel<DownloadJobData>>,
    dirty: bool,
    _lock: Option<std::fs::File>,
}
fn same_path(a: &Path, b: &Path) -> bool {
    a.canonicalize()
        .ok()
        .zip(b.canonicalize().ok())
        .is_some_and(|(a, b)| a == b)
}
fn project_path() -> Option<PathBuf> {
    let mut path = None;
    Shell::with(|shell, _| {
        path = shell
            .studio
            .borrow()
            .session
            .as_ref()
            .map(|s| PathBuf::from(s.path()))
    });
    path
}
fn open_folder(path: &Path) -> cutflow_download::Result<()> {
    if !path.is_dir() {
        return Err("目录尚未创建，完成一次下载后再打开。".into());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        opener::open(path).map_err(|_| "无法打开文件夹，请检查目录是否仍存在。".into())
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        Err("文件夹入口仅用于桌面版".into())
    }
}
fn save_current() -> cutflow_download::Result<()> {
    let mut result = Ok(());
    Shell::with(|shell, _| {
        let mut studio = shell.studio.borrow_mut();
        studio.flush_commit();
        if let Some(session) = studio.session.as_mut() {
            result = session.save(None);
        }
    });
    result
}
impl State {
    fn tell(&mut self, text: impl Into<String>, tone: i32) {
        self.message = text.into();
        self.tone = tone;
        self.dirty = true;
    }
    fn writable(&mut self) -> bool {
        if let Some(e) = self.fault.clone() {
            self.tell(e, 2);
            false
        } else {
            true
        }
    }
    fn network_ready(&mut self) -> bool {
        if !self.writable() {
            return false;
        }
        if !self.tools_ready {
            self.tell("下载组件未就绪，请点击检查组件或打开组件目录。", 2);
            return false;
        }
        true
    }
    fn persist(&mut self) -> bool {
        if self.fault.is_some() {
            return false;
        }
        if let Err(e) = self.queue.save(&self.engine.data) {
            self.fault = Some(format!("任务保存失败，暂停新操作：{e}"));
            self.autorun = false;
            if let Some(active) = &self.active {
                active.cancel.store(true, Ordering::SeqCst);
            }
            self.dirty = true;
            return false;
        }
        self.dirty = true;
        true
    }
    fn form_error(&self) -> String {
        if !(0..=2).contains(&self.form.quality) {
            return "请选择支持的画质。".into();
        }

        if self.fault.is_some() {
            return "任务存储不可写，请先处理下方错误。".into();
        }
        if self.current.is_none() {
            return "先新建或打开一个剪辑工程。".into();
        }
        if !self.tools_ready {
            return "先准备 bdl、ffmpeg 和 ffprobe 组件。".into();
        }
        if self.form.input.trim().is_empty() {
            return "粘贴视频链接，再填写分 P 和用途备注。".into();
        }
        if let Err(e) = cutflow_download::validate_input(&self.form.input) {
            return e;
        }
        if let Err(e) = cutflow_download::validate_parts(&self.form.parts) {
            return e;
        }
        if self.form.rights.trim().is_empty() {
            return "请填写素材用途或授权备注。".into();
        }
        String::new()
    }
    fn sync_context(&mut self) {
        let current = project_path();
        if self.current != current {
            self.current = current;
            self.dirty = true;
        }
    }
    fn publish(&mut self, app: &App) {
        self.sync_context();
        app.set_download_message(self.fault.as_deref().unwrap_or(&self.message).into());
        app.set_download_message_tone(if self.fault.is_some() { 2 } else { self.tone });
        app.set_download_busy(self.active.is_some());
        app.set_download_available(self.fault.is_none());
        app.set_download_account(self.engine.data.join("account.dpapi").exists());
        app.set_download_tools(self.tools.clone().into());
        app.set_download_project(
            self.current
                .as_ref()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_default()
                .into(),
        );
        app.set_download_can_parse(
            self.active.is_none() && self.tools_ready && self.fault.is_none(),
        );
        app.set_download_parsing(self.active.as_ref().is_some_and(|a| a.kind == Work::Parse));
        let error = self.form_error();
        app.set_download_form_valid(error.is_empty());
        app.set_download_form_error(error.into());
        let pending = self
            .queue
            .jobs
            .iter()
            .any(|j| matches!(j.status, Status::Queued | Status::Running));
        app.set_download_can_pause_all(
            self.fault.is_none()
                && (pending || self.active.as_ref().is_some_and(|a| a.kind == Work::Parse)),
        );
        app.set_download_can_resume_all(
            self.fault.is_none()
                && self.tools_ready
                && self
                    .active
                    .as_ref()
                    .is_none_or(|a| !a.cancel.load(Ordering::SeqCst))
                && self.queue.jobs.iter().any(|j| {
                    j.status == Status::Paused || (j.status == Status::Queued && !self.autorun)
                }),
        );
        let activity = if let Some(a) = &self.active {
            let what = if a.cancel.load(Ordering::SeqCst) {
                "正在停止"
            } else {
                match a.kind {
                    Work::Parse => "解析画质",
                    Work::Download(_) => "下载与校验",
                    Work::Import(_) => "准备入库",
                    Work::Check => "检查组件",
                }
            };
            format!("{what} · {} 秒", a.started.elapsed().as_secs())
        } else if !self.autorun && self.queue.jobs.iter().any(|j| j.status == Status::Queued) {
            "队列已暂停".into()
        } else {
            "本地剪辑可继续".into()
        };
        app.set_download_activity(activity.into());
        if !self.dirty {
            return;
        }
        let count = |status| {
            self.queue
                .jobs
                .iter()
                .filter(|j| j.status == status)
                .count()
        };
        app.set_download_stats(
            format!(
                "{} 个任务   ·   {} 等待   {} 待入库   {} 完成",
                self.queue.jobs.len(),
                count(Status::Queued) + count(Status::Paused),
                count(Status::Ready),
                count(Status::Imported)
            )
            .into(),
        );
        let rows = self
            .queue
            .jobs
            .iter()
            .rev()
            .filter(|j| actions::matches_filter(j, self.filter, &self.query))
            .map(|job| {
                let active = self
                    .active
                    .as_ref()
                    .is_some_and(|a| a.kind.id() == Some(job.id.as_str()));
                let rules = job.actions(active);
                let matches = self
                    .current
                    .as_ref()
                    .is_some_and(|p| same_path(p, &job.target));
                let status = if active
                    && self
                        .active
                        .as_ref()
                        .is_some_and(|a| a.cancel.load(Ordering::SeqCst))
                {
                    "正在停止"
                } else if active
                    && self
                        .active
                        .as_ref()
                        .is_some_and(|a| matches!(a.kind, Work::Import(_)))
                {
                    "入库中"
                } else {
                    match job.status {
                        Status::Queued => {
                            if self.autorun {
                                "排队"
                            } else {
                                "等待继续"
                            }
                        }
                        Status::Running => "下载中",
                        Status::Paused => "已暂停",
                        Status::Failed => "失败",
                        Status::Ready => "待入库",
                        Status::Imported => "已入库",
                        Status::Cancelled => "已取消",
                    }
                };
                let specs = job
                    .assets
                    .iter()
                    .map(|a| {
                        format!(
                            "{}×{} · {} · {} fps",
                            a.width, a.height, a.codec, a.frame_rate
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" / ");
                let detail = if specs.is_empty() {
                    format!(
                        "画质 {} · {}",
                        job.quality,
                        if job.allow_lower {
                            "允许降档"
                        } else {
                            "不降档"
                        }
                    )
                } else {
                    specs
                };
                let hint = job.error.clone().unwrap_or_else(|| {
                    if job.status == Status::Ready && !matches {
                        "打开绑定工程后再导入，素材不会进入其他工程。".into()
                    } else if job.status == Status::Imported {
                        "已保存到工程素材库，可拖入时间线。".into()
                    } else if matches!(job.status, Status::Paused | Status::Cancelled) {
                        "已下载文件和恢复检查点保留。".into()
                    } else {
                        String::new()
                    }
                });
                DownloadJobData {
                    id: job.id.clone().into(),
                    title: format!("{} · P{}", job.input, job.parts).into(),
                    status: status.into(),
                    detail: detail.into(),
                    target: job
                        .target
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                        .into(),
                    hint: hint.into(),
                    tone: if job.error.is_some() {
                        2
                    } else if active {
                        1
                    } else {
                        0
                    },
                    resumable: rules.resume && (self.tools_ready || !job.assets.is_empty()),
                    stoppable: rules.pause,
                    cancellable: rules.cancel,
                    removable: rules.remove,
                    importable: rules.import && matches,
                    revealable: self.engine.data.join("jobs").join(&job.id).is_dir(),
                    switchable: job.status == Status::Ready && !matches,
                }
            })
            .collect::<Vec<_>>();
        self.rows.set_vec(rows);
        self.dirty = false;
    }
    fn launch(&mut self, kind: Work, f: impl FnOnce(Arc<AtomicBool>) -> Done + Send + 'static) {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let tx = self.tx.clone();
        let worker = thread::spawn(move || {
            let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(flag)))
                .unwrap_or(Done::Crashed);
            let _ = tx.send(done);
        });
        self.active = Some(Active {
            kind,
            cancel,
            worker,
            started: Instant::now(),
        });
        self.dirty = true;
    }
    fn launch_parse(&mut self, input: String) {
        if !self.network_ready() {
            return;
        }
        if self.active.is_some() {
            self.tell("当前任务结束后再解析；仍可添加排队任务。", 0);
            return;
        }
        if let Err(e) = cutflow_download::validate_input(&input) {
            self.tell(e, 2);
            return;
        }
        let engine = self.engine.clone();
        self.tell("正在查询当前账号可用画质…", 1);
        self.launch(Work::Parse, move |flag| {
            Done::Parsed(engine.parse(&input, &flag))
        });
    }
    fn enqueue(&mut self, input: String, parts: String, quality: i32, lower: bool, rights: String) {
        self.sync_context();
        if !self.network_ready() {
            return;
        }
        let Some(target) = self.current.clone() else {
            self.tell("请先新建或打开剪辑工程。", 2);
            return;
        };
        let quality = match quality {
            0 => "120",
            1 => "80",
            2 => "best",
            _ => {
                self.tell("无效画质选项", 2);
                return;
            }
        };
        match self.queue.enqueue(
            &input,
            &parts,
            quality,
            lower && quality != "best",
            target,
            &rights,
        ) {
            Ok(_) => {
                // An error-paused queue remains paused when another task is added.
                let held = self
                    .queue
                    .jobs
                    .iter()
                    .any(|j| matches!(j.status, Status::Failed | Status::Paused))
                    && !self.autorun;
                if self.persist() {
                    if !held {
                        self.autorun = true;
                    }
                    self.tell(
                        if held {
                            "任务已添加；队列暂停中，请点击继续队列。"
                        } else {
                            "任务已加入队列，完成后自动校验并入库。"
                        },
                        1,
                    );
                }
            }
            Err(e) => self.tell(e, 2),
        }
    }
    fn next_download(&mut self) {
        if self.active.is_some() || !self.autorun || self.fault.is_some() || !self.tools_ready {
            return;
        }
        let Some(index) = self
            .queue
            .jobs
            .iter()
            .position(|j| j.status == Status::Queued)
        else {
            return;
        };
        self.queue.jobs[index].status = Status::Running;
        if !self.persist() {
            self.queue.jobs[index].status = Status::Paused;
            return;
        }
        let job = self.queue.jobs[index].clone();
        let id = job.id.clone();
        let engine = self.engine.clone();
        self.tell("正在下载母版；校验通过后才会入库。", 1);
        self.launch(Work::Download(id), move |flag| {
            Done::Downloaded(job.id.clone(), engine.download(&job, flag))
        });
    }
    fn prepare_import(&mut self, id: &str) {
        self.sync_context();
        if !self.writable() {
            return;
        }
        if self.active.is_some() {
            self.tell("请等待当前处理结束后再导入。", 0);
            return;
        }
        let Some(job) = self
            .queue
            .jobs
            .iter()
            .find(|j| j.id == id && j.status == Status::Ready)
            .cloned()
        else {
            self.tell("该任务当前没有可入库的完成素材。", 2);
            return;
        };
        if !self
            .current
            .as_ref()
            .is_some_and(|p| same_path(p, &job.target))
        {
            self.tell("请先切换到此任务绑定的工程。", 2);
            return;
        }
        if let Some(held) = self.queue.jobs.iter_mut().find(|j| j.id == id) {
            held.error = None;
        }
        self.tell("正在校验媒体并准备导入绑定工程…", 1);
        self.launch(Work::Import(job.id.clone()), move |flag| {
            let results = job
                .assets
                .iter()
                .enumerate()
                .map(|(index,a)| {
                    if flag.load(Ordering::SeqCst) {
                        return Err("已停止入库".into());
                    }
                    if std::fs::metadata(&a.path).map(|m| m.len()).ok() != Some(a.bytes) {
                        return Err("媒体已改变或丢失，请检查母版文件。".into());
                    }
                    let folder=job.target.join("assets/bilibili").join(&job.id);
                    std::fs::create_dir_all(&folder).map_err(|error|error.to_string())?;
                    let target=folder.join(format!("source-{index}.{}",a.path.extension().and_then(|extension|extension.to_str()).unwrap_or("mp4")));
                    if !target.exists() {
                        let partial=target.with_extension("part");
                        std::fs::copy(&a.path,&partial).map_err(|error|error.to_string())?;
                        std::fs::rename(&partial,&target).map_err(|error|error.to_string())?;
                    }
                    if std::fs::metadata(&target).map(|metadata|metadata.len()).ok()!=Some(a.bytes) {return Err("工程素材已变化，请核查文件后重试入库".into());}
                    media::probe(&target.to_string_lossy())
                })
                .collect();
            Done::Probed(job.id, results)
        });
    }
    fn commit_import(&mut self, id: &str, results: Vec<cutflow_download::Result<MediaSummary>>) {
        if self.fault.is_some() {
            return;
        }
        let Some(job) = self
            .queue
            .jobs
            .iter_mut()
            .find(|j| j.id == id && j.status == Status::Ready)
        else {
            return;
        };
        let summaries = match results
            .into_iter()
            .collect::<cutflow_download::Result<Vec<_>>>()
        {
            Ok(s) => s,
            Err(e) => {
                job.error = Some(e.clone());
                self.tell(e, 2);
                return;
            }
        };
        let mut result = Err("当前工程已切换，素材保持待入库状态。".to_string());
        Shell::with(|shell, app| {
            let mut studio = shell.studio.borrow_mut();
            if !studio
                .session
                .as_ref()
                .is_some_and(|s| same_path(Path::new(s.path()), &job.target))
            {
                return;
            }
            let commands = summaries
                .iter()
                .filter(|s| {
                    !studio
                        .project()
                        .media
                        .iter()
                        .any(|m| same_path(Path::new(&m.path), Path::new(&s.path)))
                })
                .map(|s| Command::AddMedia {
                    item: s.to_new_media(),
                })
                .collect::<Vec<_>>();
            if !commands.is_empty() {
                studio.apply(Command::Batch { commands });
            }
            if !summaries.iter().all(|summary| {
                studio
                    .project()
                    .media
                    .iter()
                    .any(|m| same_path(Path::new(&m.path), Path::new(&summary.path)))
            }) {
                result = Err("部分素材被编辑器拒绝，请检查格式兼容性。".into());
                return;
            }
            result = studio
                .session
                .as_mut()
                .ok_or("工程已关闭".to_string())
                .and_then(|s| s.save(None));
            if result.is_ok() {
                studio.refresh_art();
                studio.notify("下载素材已入库并保存。", false);
            }
            studio.publish(&app, &shell.models);
        });
        match result {
            Ok(()) => {
                job.status = Status::Imported;
                job.error = None;
                for a in &mut job.assets {
                    a.imported = true;
                }
                self.tell("素材已进入绑定工程，可拖入时间线剪辑。", 1);
            }
            Err(e) => {
                job.error = Some(e.clone());
                self.tell(e, 2);
            }
        }
    }
    fn action(&mut self, id: &str, action: Action) {
        if !self.writable() {
            return;
        }
        let active = self
            .active
            .as_ref()
            .is_some_and(|a| a.kind.id() == Some(id));
        if matches!(action, Action::Resume)
            && self
                .queue
                .jobs
                .iter()
                .find(|j| j.id == id)
                .is_some_and(|j| j.assets.is_empty())
            && !self.network_ready()
        {
            return;
        }
        match self.queue.action(id, action, active) {
            Err(e) => self.tell(e, 2),
            Ok(()) => {
                if active && matches!(action, Action::Pause | Action::Cancel) {
                    if let Some(a) = &self.active {
                        a.cancel.store(true, Ordering::SeqCst);
                    }
                }
                if self.persist() {
                    if matches!(action, Action::Resume) {
                        self.autorun = true;
                    }
                    self.tell(
                        match action {
                            Action::Pause => "任务已暂停；其他排队任务按原顺序继续。",
                            Action::Cancel => "任务已取消，已下载文件保留。",
                            Action::Resume => "任务已重新排队。",
                            Action::Remove => "任务记录已移除，媒体文件和工程保留。",
                        },
                        1,
                    );
                }
            }
        }
    }
    fn pause_all(&mut self) {
        if !self.writable() {
            return;
        }
        self.autorun = false;
        self.queue.pause_all();
        if let Some(a) = &self.active {
            if matches!(a.kind, Work::Download(_) | Work::Parse) {
                a.cancel.store(true, Ordering::SeqCst);
            }
        }
        if self.persist() {
            self.tell("队列已暂停；已下载文件和恢复检查点保留。", 1);
        }
    }
    fn resume_all(&mut self) {
        if !self.network_ready() {
            return;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|a| a.cancel.load(Ordering::SeqCst))
        {
            self.tell("正在停止组件，请稍后继续。", 0);
            return;
        }
        let count = self.queue.resume_waiting();
        if self.persist() {
            self.autorun = true;
            self.tell(
                format!("已继续 {count} 个等待任务；失败和取消任务需单独重试。"),
                1,
            );
        }
    }
    fn reveal(&mut self, id: &str) {
        if !self.queue.jobs.iter().any(|j| j.id == id) {
            self.tell("任务不存在", 2);
            return;
        }
        let dir = self.engine.data.join("jobs").join(id);
        let media = dir.join("media");
        if let Err(e) = open_folder(if media.is_dir() { &media } else { &dir }) {
            self.tell(e, 2);
        }
    }
    fn check(&mut self) {
        if self.active.is_some() {
            self.tell("当前任务结束后再检查组件。", 0);
            return;
        }
        let engine = self.engine.clone();
        self.tell("正在本地检查组件版本与可执行状态…", 1);
        self.launch(Work::Check, move |flag| {
            Done::Checked(engine.check_components(&flag))
        });
    }
    fn switch_project(&mut self, id: &str) {
        if self.active.is_some() {
            self.tell("当前处理结束后再切换工程。", 0);
            return;
        }
        let Some(target) = self
            .queue
            .jobs
            .iter()
            .find(|j| j.id == id)
            .map(|j| j.target.clone())
        else {
            self.tell("任务不存在", 2);
            return;
        };
        let info = match concat_host::projects::open(&target.to_string_lossy()) {
            Ok(info) => info,
            Err(e) => {
                self.tell(e, 2);
                return;
            }
        };
        let mut result = Ok(());
        Shell::with(|shell, app| {
            let mut studio = shell.studio.borrow_mut();
            studio.close_project();
            if studio.session.is_some() {
                result = Err("当前工程保存失败，已取消切换。".into());
                return;
            }
            result = studio.open_project(info);
            studio.publish(&app, &shell.models);
        });
        match result {
            Ok(()) => self.tell("已切换到绑定工程；校验通过后自动入库。", 1),
            Err(e) => self.tell(e, 2),
        }
    }
    fn tick(&mut self, app: &App) {
        self.sync_context();
        while let Ok(done) = self.rx.try_recv() {
            let Some(active) = self.active.take() else {
                continue;
            };
            let stopped = active.cancel.load(Ordering::SeqCst);
            let kind = active.kind;
            let _ = active.worker.join();
            self.dirty = true;
            match done {
                Done::Parsed(result) => {
                    if stopped {
                        self.tell("解析已取消。", 0);
                    } else {
                        match result {
                            Ok(text) => {
                                app.set_download_formats(text.into());
                                self.tell("解析完成，选择分 P 和画质后加入队列。", 1);
                            }
                            Err(e) => {
                                self.autorun = false;
                                self.tell(e, 2);
                            }
                        }
                    }
                }
                Done::Downloaded(id, result) => {
                    let mut error = None;
                    let mut ready = false;
                    if let Some(job) = self.queue.jobs.iter_mut().find(|j| j.id == id) {
                        let kept = matches!(job.status, Status::Paused | Status::Cancelled);
                        match result {
                            Ok(assets) => {
                                job.assets = assets;
                                if !kept {
                                    job.status = Status::Ready;
                                    job.error = None;
                                    ready = true;
                                }
                            }
                            Err(e) => {
                                if !kept {
                                    job.status = Status::Failed;
                                    job.error = Some(e.clone());
                                    error = Some(e);
                                }
                            }
                        }
                    }
                    self.persist();
                    if let Some(e) = error {
                        self.autorun = false;
                        self.tell(e, 2);
                    } else if ready {
                        self.tell("下载校验完成，等待导入绑定工程。", 1);
                    }
                }
                Done::Probed(id, results) => {
                    if !stopped {
                        self.commit_import(&id, results);
                    }
                    self.persist();
                }
                Done::Checked(result) => match result {
                    Ok(report) => {
                        self.tools_ready = true;
                        self.tools = report;
                        self.tell("三个组件已通过本地启动检查。", 1);
                    }
                    Err(e) => {
                        self.tools_ready = false;
                        let missing = self.engine.missing_components();
                        self.tools = if missing.is_empty() {
                            "组件存在，但启动检查失败；请检查 DLL 和版本。".into()
                        } else {
                            format!("缺少组件：{}", missing.join("、"))
                        };
                        self.tell(e, 2);
                    }
                },
                Done::Crashed => {
                    self.autorun = false;
                    if let Some(id) = kind.id() {
                        if let Some(job) = self.queue.jobs.iter_mut().find(|j| j.id == id) {
                            if !matches!(job.status, Status::Paused | Status::Cancelled) {
                                if matches!(kind, Work::Download(_)) {
                                    job.status = Status::Failed;
                                }
                                job.error = Some("后台任务异常退出，可手动重试。".into());
                            }
                        }
                    }
                    self.persist();
                    self.tell("后台任务异常退出；队列已暂停。", 2);
                }
            }
        }
        if self.active.is_none() && self.fault.is_none() {
            let next = self
                .queue
                .jobs
                .iter()
                .find(|j| {
                    j.status == Status::Ready
                        && j.error.is_none()
                        && self
                            .current
                            .as_ref()
                            .is_some_and(|p| same_path(p, &j.target))
                })
                .map(|j| j.id.clone());
            if let Some(id) = next {
                self.prepare_import(&id);
            }
        }
        self.next_download();
        self.publish(app);
    }
}

/// Keeps all task workers under the lifetime of the editor window.
pub struct Downloads {
    state: Rc<RefCell<State>>,
    timer: slint::Timer,
}
impl Downloads {
    pub fn install(app: &App) -> Self {
        let root = std::env::current_exe().ok()
            .and_then(|path| path.parent().map(|parent|parent.join("portable/localappdata/CutFlow")))
            .unwrap_or_else(|| PathBuf::from("portable/localappdata/CutFlow"));
        let tools = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("tools")))
            .unwrap_or_default();
        let lock = (|| -> Result<std::fs::File, String> {
            std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(root.join("queue.lock"))
                .map_err(|e| e.to_string())?;
            file.try_lock()
                .map_err(|_| "另一个 CutFlow 实例正在管理下载任务。".to_string())?;
            Ok(file)
        })();
        // Lock before reading, so another writer cannot change our initial snapshot.
        let (mut fault, lock) = match lock {
            Ok(f) => (None, Some(f)),
            Err(e) => (Some(e), None),
        };
        let queue = match Queue::load(&root) {
            Ok(q) => q,
            Err(e) => {
                fault = Some(format!("任务文件读取失败，禁止覆盖：{e}"));
                Queue {
                    version: 1,
                    jobs: vec![],
                }
            }
        };
        let engine = Engine { tools, data: root };
        let missing = engine.missing_components();
        let ready = missing.is_empty();
        let tools = if ready {
            "组件文件已找到；可点击检查组件确认版本。".into()
        } else {
            format!("缺少 {}；打开组件目录查看。", missing.join("、"))
        };
        let (tx, rx) = mpsc::channel();
        let rows = Rc::new(VecModel::default());
        app.set_download_jobs(ModelRc::from(rows.clone()));
        let state = Rc::new(RefCell::new(State {
            queue,
            engine,
            tx,
            rx,
            active: None,
            autorun: false,
            fault,
            message: "下载任务绑定创建时的工程；关闭面板可继续剪辑。".into(),
            tone: 0,
            tools,
            tools_ready: ready,
            filter: 0,
            query: String::new(),
            form: Form {
                parts: "1".into(),
                ..Default::default()
            },
            current: None,
            rows,
            dirty: true,
            _lock: lock,
        }));
        macro_rules! bind {($callback:ident, |$s:ident $(,$arg:ident)*| $body:block)=>{{let held=state.clone();app.$callback(move |$($arg),*|{let mut $s=held.borrow_mut();$body});}};}
        {
            let weak = app.as_weak();
            let state = state.clone();
            app.on_download_open(move || {
                if let Some(app) = weak.upgrade() {
                    state.borrow_mut().publish(&app);
                    app.set_download_opened(true);
                }
            });
        }
        {
            let weak = app.as_weak();
            app.on_download_close(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_download_opened(false);
                }
            });
        }
        bind!(
            on_download_form_changed,
            |s, input, parts, quality, _lower, rights| {
                s.form = Form {
                    input: input.to_string(),
                    parts: parts.to_string(),
                    quality,
                    rights: rights.to_string(),
                };
                Shell::with(|_, app| s.publish(&app));
            }
        );
        bind!(on_download_parse, |s, input| {
            s.launch_parse(input.to_string());
        });
        bind!(
            on_download_enqueue,
            |s, input, parts, quality, lower, rights| {
                s.enqueue(
                    input.to_string(),
                    parts.to_string(),
                    quality,
                    lower,
                    rights.to_string(),
                );
            }
        );
        bind!(on_download_pause, |s, id| {
            s.action(&id, Action::Pause);
        });
        bind!(on_download_resume, |s, id| {
            s.action(&id, Action::Resume);
        });
        bind!(on_download_cancel, |s, id| {
            s.action(&id, Action::Cancel);
        });
        bind!(on_download_remove, |s, id| {
            s.action(&id, Action::Remove);
        });
        bind!(on_download_import, |s, id| {
            s.prepare_import(&id);
        });
        bind!(on_download_reveal, |s, id| {
            s.reveal(&id);
        });
        bind!(on_download_switch_project, |s, id| {
            s.switch_project(&id);
        });
        bind!(on_download_pause_all, |s| {
            s.pause_all();
        });
        bind!(on_download_resume_all, |s| {
            s.resume_all();
        });
        bind!(on_download_stop_parse, |s| {
            if let Some(a) = &s.active {
                if a.kind == Work::Parse {
                    a.cancel.store(true, Ordering::SeqCst);
                    s.dirty = true;
                }
            }
        });
        bind!(on_download_check_tools, |s| {
            s.check();
        });
        bind!(on_download_reveal_tools, |s| {
            let tools = if s.engine.tools.is_dir() {
                s.engine.tools.clone()
            } else {
                s.engine
                    .tools
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_path_buf()
            };
            if let Err(e) = open_folder(&tools) {
                s.tell(e, 2);
            }
        });
        bind!(on_download_filter_changed, |s, filter, query| {
            s.filter = filter;
            s.query = query.to_string();
            s.dirty = true;
        });
        bind!(on_download_save_cookie, |s, cookie| {
            if s.writable() {
                if s.active.is_some() {
                    s.tell("任务处理期间不能替换账号。", 2);
                } else {
                    match cutflow_download::credentials::save(&s.engine.data, &cookie) {
                        Ok(()) => s.tell("凭据已加密保存；请重新解析以检查实际权限。", 1),
                        Err(e) => s.tell(e, 2),
                    }
                }
            }
        });
        bind!(on_download_clear_cookie, |s| {
            if s.writable() {
                if s.active.is_some() {
                    s.tell("请先停止任务，再退出账号。", 2);
                } else {
                    match cutflow_download::credentials::clear(&s.engine.data) {
                        Ok(()) => s.tell("已退出账号，本地媒体与工程保留。", 1),
                        Err(e) => s.tell(e, 2),
                    }
                }
            }
        });
        {
            let weak = app.as_weak();
            let state = state.clone();
            app.on_download_choose_project(move |create| {
                if state.borrow().active.is_some() {
                    state.borrow_mut().tell("任务处理结束后再切换工程。", 0);
                    return;
                }
                if let Err(e) = save_current() {
                    state.borrow_mut().tell(format!("当前工程保存失败：{e}"), 2);
                    return;
                }
                if let Some(app) = weak.upgrade() {
                    app.set_download_opened(false);
                    if create {
                        app.invoke_start_compose();
                    } else {
                        app.invoke_start_open();
                    }
                }
            });
        }
        let timer = slint::Timer::default();
        {
            let held = state.clone();
            let weak = app.as_weak();
            timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(250),
                move || {
                    if let Some(app) = weak.upgrade() {
                        held.borrow_mut().tick(&app);
                    }
                },
            );
        }
        state.borrow_mut().publish(app);
        Self { state, timer }
    }
    pub fn shutdown(&self) {
        self.timer.stop();
        let mut s = self.state.borrow_mut();
        s.autorun = false;
        s.queue.pause_all();
        if let Some(active) = s.active.take() {
            active.cancel.store(true, Ordering::SeqCst);
            s.persist();
            let _ = active.worker.join();
        } else {
            s.persist();
        }
    }
}
