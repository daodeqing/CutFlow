//! Task actions shared by the native UI and queue coordinator.
use crate::{Job, Queue, Result, Status};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Pause,
    Resume,
    Cancel,
    Remove,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actions {
    pub pause: bool,
    pub resume: bool,
    pub cancel: bool,
    pub remove: bool,
    pub import: bool,
}

impl Job {
    /// Availability describes state, independent of widget enablement.
    pub fn actions(&self, active: bool) -> Actions {
        Actions {
            pause: matches!(self.status, Status::Queued | Status::Running),
            resume: !active
                && matches!(
                    self.status,
                    Status::Paused | Status::Failed | Status::Cancelled
                ),
            cancel: matches!(
                self.status,
                Status::Queued | Status::Running | Status::Paused | Status::Failed | Status::Ready
            ),
            remove: !active
                && matches!(
                    self.status,
                    Status::Imported | Status::Cancelled | Status::Failed
                ),
            import: !active && self.status == Status::Ready,
        }
    }
}

impl Queue {
    /// A single task action never changes the state of another task.
    pub fn action(&mut self, id: &str, action: Action, active: bool) -> Result<()> {
        let index = self
            .jobs
            .iter()
            .position(|j| j.id == id)
            .ok_or("任务不存在或已移除")?;
        let actions = self.jobs[index].actions(active);
        let allowed = match action {
            Action::Pause => actions.pause,
            Action::Resume => actions.resume,
            Action::Cancel => actions.cancel,
            Action::Remove => actions.remove,
        };
        if !allowed {
            return Err("该任务当前不能执行此操作，请等待当前处理结束".into());
        }
        match action {
            Action::Pause => self.jobs[index].status = Status::Paused,
            Action::Cancel => self.jobs[index].status = Status::Cancelled,
            Action::Resume => {
                self.jobs[index].status = if self.jobs[index].assets.is_empty() {
                    Status::Queued
                } else {
                    Status::Ready
                };
                self.jobs[index].error = None;
            }
            Action::Remove => {
                self.jobs.remove(index);
            }
        }
        Ok(())
    }
    /// Stops pending/running network tasks; completed files and failed states stay intact.
    pub fn pause_all(&mut self) -> usize {
        let mut count = 0;
        for job in &mut self.jobs {
            if matches!(job.status, Status::Running | Status::Queued) {
                job.status = Status::Paused;
                count += 1;
            }
        }
        count
    }
    /// Explicitly continues waiting tasks, without retrying failed/cancelled tasks.
    pub fn resume_waiting(&mut self) -> usize {
        let mut count = 0;
        for job in &mut self.jobs {
            if matches!(job.status, Status::Paused | Status::Queued) {
                job.status = if job.assets.is_empty() {
                    Status::Queued
                } else {
                    Status::Ready
                };
                job.error = None;
                count += 1;
            }
        }
        count
    }
}

/// Query filters are stable across UI refreshes. Search never changes a task.
pub fn matches_filter(job: &Job, filter: i32, query: &str) -> bool {
    let group = match filter {
        1 => matches!(job.status, Status::Queued | Status::Running),
        2 => job.status == Status::Ready,
        3 => matches!(job.status, Status::Imported | Status::Cancelled),
        4 => matches!(job.status, Status::Failed | Status::Paused),
        _ => true,
    };
    let query = query.trim().to_lowercase();
    group
        && (query.is_empty()
            || format!("{} {} {}", job.input, job.parts, job.target.display())
                .to_lowercase()
                .contains(&query))
}
