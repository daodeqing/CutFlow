fn main() {
    let path = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../editor/src/crates/concat/ui/app.slint");
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let config = slint_build::CompilerConfiguration::new()
                .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
            slint_build::compile_with_config(path, config).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
