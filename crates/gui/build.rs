fn main() {
    // The web view may call only these commands, and capabilities/main.json allows them.
    let attributes = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
        "daemon",
        "open",
        "copy_url",
        "copy_log_path",
        "server_config",
        "choose_folder",
        "doctor",
        "devid_check",
        "in_terminal",
        "login_item",
        "self_update",
        "relaunch",
        "quit",
    ]));
    tauri_build::try_build(attributes).expect("tauri-build failed");
}
