#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|argument| argument == "--version") {
        println!("CutFlow {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let root = cutflow_workbench::portable_root().expect("cannot resolve portable directory");
    for folder in ["temp", "webview", "logs"] {
        std::fs::create_dir_all(root.join(folder)).expect("cannot create portable directory");
    }
    unsafe {
        std::env::set_var("TEMP", root.join("temp"));
        std::env::set_var("TMP", root.join("temp"));
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", root.join("webview"));
        let runtime = root.parent().unwrap().join("WebView2");
        if runtime.join("msedgewebview2.exe").is_file() { std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime); }
    }
    if cutflow_workbench::server::call(&root, &cutflow_workbench::protocol::Call {
        method: "cutflow.hub.focus".into(), params: serde_json::json!({}),
    }).is_ok() { return; }
    let marker = root.join("layout-version.json");
    if !marker.exists() {
        let backup = root.join("migrations/before-0.4");
        std::fs::create_dir_all(&backup).expect("cannot create migration backup");
        for name in ["workbench.json", "operations.json", "queue.json", "bdl-state.json", "settings.json"] {
            let source = root.join(name);
            if source.exists() && !backup.join(name).exists() { std::fs::copy(source, backup.join(name)).expect("cannot backup portable configuration"); }
        }
        let operations = root.join("operations.json");
        let editor = root.join("editor");
        std::fs::create_dir_all(&editor).expect("cannot create editor state directory");
        if operations.is_file() && !editor.join("operations.json").exists() {
            std::fs::copy(operations, editor.join("operations.json")).expect("cannot migrate editor operations");
        }
        cutflow_workbench::atomic_json(&marker, &serde_json::json!({"version":"0.4.0"})).expect("cannot save portable layout version");
    }
    bdl_desktop_lib::run();
}
