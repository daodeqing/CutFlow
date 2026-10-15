fn slint_import_path(relative: &str) -> String {
    let p = std::path::PathBuf::from(relative).canonicalize().unwrap();
    let s = p.to_string_lossy().to_string();
    let stripped = s.strip_prefix(r"\\?\").unwrap_or(&s);
    stripped.replace('\\', "/")
}

fn main() {
    let compile = |path| {
        let config = slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
        slint_build::compile_with_config(path, config).unwrap();
    };
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let font_import = std::env::var("CUTFLOW_UI_FONT")
        .ok()
        .map(|font| {
            let clean = font.strip_prefix(r"\\?\").unwrap_or(&font).replace('\\', "/");
            format!("import \"{clean}\";\n")
        })
        .unwrap_or_default();
    let mut preview = std::fs::read_to_string("ui.slint").unwrap();
    for file in ["dialogs/downloads.slint", "theme/theme.slint"] {
        let relative = format!("../../editor/src/crates/concat/ui/{file}");
        let absolute = slint_import_path(&relative);
        preview = preview.replace(
            &format!("\"{relative}\""),
            &format!("\"{absolute}\""),
        );
    }
    let preview_file = out_dir.join("ui.slint");
    std::fs::write(&preview_file, format!("{font_import}{preview}")).unwrap();
    compile(preview_file);
    println!("cargo:rerun-if-env-changed=CUTFLOW_UI_FONT");
    println!("cargo:rerun-if-changed=ui.slint");
    // Reuse production callback/property declarations and its exact DownloadDialog wiring.
    let source = std::fs::read_to_string("../../editor/src/crates/concat/ui/app.slint").unwrap();
    let start = source
        .find("    in-out property <bool> download-opened;")
        .unwrap();
    let end = source[start..].find("    in-out property <bool> workbench-opened;").unwrap() + start;
    let props = &source[start..end];
    let start = source
        .find("    download-sheet := DownloadDialog {")
        .unwrap();
    let end = source[start..].find("    SettingsDialog {").unwrap() + start;
    let dialog = &source[start..end];
    let dialog_file = slint_import_path("../../editor/src/crates/concat/ui/dialogs/downloads.slint");
    let theme = slint_import_path("../../editor/src/crates/concat/ui/theme/theme.slint");
    let text = format!(
        "{font_import}import {{DownloadDialog,DownloadJobData}} from \"{dialog_file}\";\nimport {{Theme}} from \"{theme}\";\nexport {{DownloadJobData,Theme}}\nexport component App inherits Window {{width:1120px;height:900px; callback start-compose(); callback start-open();\n{props}\n{dialog}\n}}",
    );
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("bridge.slint");
    std::fs::write(&out, text).unwrap();
    compile(out);
    println!("cargo:rerun-if-changed=../../editor/src/crates/concat/ui/app.slint");
}
