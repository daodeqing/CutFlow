//! Harness for the real coordinator and UI, with an explicitly simulated media host.
pub mod ui {
    include!(concat!(env!("OUT_DIR"), "/bridge.rs"));
}
#[path = "../../../editor/src/crates/concat/src/downloads.rs"]
pub mod downloads;
pub mod host {
    use concat_project::{Command, Project};
    use std::{cell::RefCell, rc::Rc};
    pub struct Studio {
        pub session: Option<concat_host::Session>,
        empty: Project,
    }
    impl Default for Studio {
        fn default() -> Self {
            Self {
                session: None,
                empty: Project::default(),
            }
        }
    }
    impl Studio {
        pub fn flush_commit(&mut self) {}
        pub fn project(&self) -> &Project {
            self.session
                .as_ref()
                .map(|s| &s.project)
                .unwrap_or(&self.empty)
        }
        pub fn apply(&mut self, cmd: Command) -> Option<String> {
            if let Some(s) = &mut self.session {
                fn apply(project: &mut Project, cmd: Command) {
                    match cmd {
                        Command::AddMedia { item } => project.media.push(item),
                        Command::Batch { commands } => {
                            for c in commands {
                                apply(project, c)
                            }
                        }
                    }
                }
                apply(&mut s.project, cmd);
            }
            None
        }
        pub fn refresh_art(&mut self) {}
        pub fn notify(&mut self, _: &str, _: bool) {}
        pub fn publish(&self, _: &crate::ui::App, _: &()) {}
        pub fn close_project(&mut self) {
            if self.session.as_mut().is_none_or(|s| s.save(None).is_ok()) {
                self.session = None;
            }
        }
        pub fn open_project(
            &mut self,
            info: concat_host::projects::ProjectInfo,
        ) -> Result<(), String> {
            self.session = Some(concat_host::Session {
                path: info.path,
                project: Project::default(),
            });
            Ok(())
        }
    }
    pub struct Shell {
        pub studio: RefCell<Studio>,
        pub models: (),
    }
    thread_local! {static SHELL:RefCell<Option<Rc<Shell>>>=const{RefCell::new(None)};}
    impl Shell {
        pub fn install(shell: Rc<Shell>) {
            SHELL.with(|s| *s.borrow_mut() = Some(shell));
        }
        pub fn with(body: impl FnOnce(&Shell, crate::ui::App)) {
            SHELL.with(|s| {
                let held = s.borrow().clone();
                APP.with(|app| {
                    if let (Some(shell), Some(app)) =
                        (held, app.borrow().as_ref().and_then(slint::Weak::upgrade))
                    {
                        body(&shell, app);
                    }
                })
            });
        }
    }
    thread_local! {static APP:RefCell<Option<slint::Weak<crate::ui::App>>>=const{RefCell::new(None)};}
    pub fn install_app(app: &crate::ui::App) {
        use slint::ComponentHandle;
        APP.with(|a| *a.borrow_mut() = Some(app.as_weak()));
    }
}
