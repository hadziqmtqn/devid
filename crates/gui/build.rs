fn main() {
    // The web view may call only these commands, and capabilities/main.json allows them.
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&["status", "sites"]));
    tauri_build::try_build(attributes).expect("tauri-build failed");
}
