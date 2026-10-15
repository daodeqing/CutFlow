//! Renders the actual native dialog without FFmpeg or an OS desktop.
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::{ComponentHandle, PhysicalSize, Rgb8Pixel, SharedPixelBuffer};
use std::{cell::RefCell, rc::Rc};
include!(concat!(env!("OUT_DIR"), "/ui.rs"));
struct Headless {
    window: Rc<MinimalSoftwareWindow>,
}
impl slint::platform::Platform for Headless {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
}
fn main() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless {
        window: window.clone(),
    }))
    .unwrap();
    let app = Preview::new().unwrap();
    if let Ok(path) = std::env::var("CUTFLOW_UI_FONT") {
        slint::platform::WindowAdapter::renderer(&*window)
            .register_font_from_path(std::path::Path::new(&path))
            .unwrap();
    }
    app.global::<Theme>()
        .set_dark(std::env::var("CUTFLOW_UI_LIGHT").is_err());
    let events = Rc::new(RefCell::new(Vec::<String>::new()));
    {
        let events = events.clone();
        app.on_parsed(move |s| events.borrow_mut().push(format!("parse:{s}")));
    }
    {
        let weak = app.as_weak();
        app.on_changed(move |input, _, _, _, rights| {
            if let Some(app) = weak.upgrade() {
                app.set_valid(!input.is_empty() && !rights.is_empty());
            }
        });
    }
    let job = |id: &str, status: &str, detail: &str| DownloadJobData {
        id: id.into(),
        title: "BV1xx411c7mD · P1".into(),
        status: status.into(),
        detail: detail.into(),
        target: "城市光影".into(),
        hint: "".into(),
        ..Default::default()
    };
    let mut ready = job("ready", "待入库", "3840×2160 · hevc · 30000/1001 fps");
    ready.importable = true;
    ready.revealable = true;
    ready.cancellable = true;
    let mut failed = job("failed", "失败", "画质 120 · 不降档");
    failed.resumable = true;
    failed.removable = true;
    failed.cancellable = true;
    failed.tone = 2;
    failed.hint = "当前账号没有可用 4K，请检查权限或选择其他画质。".into();
    let mut imported = job("imported", "已入库", "3840×2160 · h264 · 30/1 fps");
    imported.removable = true;
    imported.revealable = true;
    app.set_jobs(slint::ModelRc::new(slint::VecModel::from(vec![
        ready, failed, imported,
    ])));
    for kind in ["import", "pause-all", "cancel"] {
        let events = events.clone();
        match kind {
            "import" => app.on_imported(move |id| events.borrow_mut().push(format!("import:{id}"))),
            "pause-all" => app.on_all_paused(move || events.borrow_mut().push("pause-all".into())),
            _ => app.on_cancelled(move |id| events.borrow_mut().push(format!("cancel:{id}"))),
        }
    }
    app.show().unwrap();
    let width = std::env::var("CUTFLOW_UI_WIDTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1120);
    let height = std::env::var("CUTFLOW_UI_HEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900);
    window.set_size(PhysicalSize::new(width, height));
    let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    window.draw_if_needed(|renderer| {
        renderer.render(pixels.make_mut_slice(), width as usize);
    });
    let file =
        std::env::var("CUTFLOW_UI_OUTPUT").unwrap_or_else(|_| "cutflow-native-ui.png".into());
    image::save_buffer(
        &file,
        pixels.as_bytes(),
        width,
        height,
        image::ColorType::Rgb8,
    )
    .unwrap();
    println!("Native Slint dialog rendered: {file} ({width}×{height})");
    if std::env::var("CUTFLOW_UI_CLICK_CHECK").is_ok() {
        assert_eq!(
            (width, height),
            (1120, 900),
            "Fixed viewport for hit testing"
        );
        // Pointer coordinates are checked against the reviewed native 1120×900 layout.
        app.window()
            .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
        app.set_opened(false);
        slint::platform::update_timers_and_animations();
        app.set_opened(true);
        slint::platform::update_timers_and_animations();
        app.invoke_focus_dialog();
        click(app.window(), 960.0, 279.0);
        assert!(events.borrow().iter().any(|e| e == "pause-all"));
        click(app.window(), 436.0, 467.0);
        assert!(events.borrow().iter().any(|e| e == "import:ready"));
        click(app.window(), 571.0, 467.0);
        assert!(
            !events.borrow().iter().any(|e| e.starts_with("cancel:")),
            "Confirmation is required before cancelling"
        );
        app.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Escape.into(),
            });
        assert!(
            app.get_opened(),
            "First Escape dismisses the confirmation only"
        );
        app.window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                text: slint::platform::Key::Escape.into(),
            });
        app.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Escape.into(),
            });
        assert!(!app.get_opened(), "Second Escape returns to the editor");
        println!(
            "PASS native pointer hit tests: pause-all, import, cancel confirmation, Escape focus"
        );
    }
}

fn click(window: &slint::Window, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
