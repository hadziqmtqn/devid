//! D-031: the two "at login" settings. `autostart` decides whether nginx and php-fpm start with
//! the daemon; `login-item` writes (never loads) the desktop app's LaunchAgent.
mod common;
use common::*;
use std::{fs, path::Path};

#[test]
fn autostart_and_login_item() {
    let root = temp_root("devid-login-it");
    let home = root.join("home");
    let user = root.join("user"); // the daemon's HOME, so ~/Library/LaunchAgents is a test folder
    fs::create_dir_all(&user).unwrap();
    let (b85, nginx) = (fake_build(&root, "8.5"), fake_nginx(&root, &home));
    let s = |p: &Path| p.to_str().unwrap().to_string();
    let env = [("HOME", s(&user))];
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let daemon = start_daemon_env(&home, &env);
    assert!(devid(&home, &["php", "install", "8.5", "--from", &s(&b85)]).status.success());
    assert!(devid(&home, &["server", "install", "nginx", "--from", &s(&nginx)]).status.success());
    wait_for("nginx running", || json(&home, &["status", "--json"])["server_status"] == "running");
    assert_eq!(json(&home, &["status", "--json"])["autostart"], true, "on by default");

    // The login item is written for the next login only, and removed again.
    let app = root.join("devid-gui");
    fs::write(&app, "").unwrap();
    let out = devid(&home, &["login-item", "on", "--app", &s(&app)]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Devid opens at login"));
    let plist = user.join("Library/LaunchAgents/dev.devid.app.plist");
    assert!(fs::read_to_string(&plist).unwrap().contains(&format!("<string>{}</string>", s(&app))));
    assert_eq!(json(&home, &["status", "--json"])["login_item"], s(&app));
    assert!(devid(&home, &["login-item", "off"]).status.success());
    assert!(!plist.exists());
    assert_eq!(json(&home, &["status", "--json"])["login_item"], serde_json::Value::Null);
    assert!(stderr(&devid(&home, &["login-item", "on"])).contains("not installed"), "default app missing");

    // Autostart off: after the daemon restarts (as at login), nginx and php-fpm wait for `start all`.
    assert!(devid(&home, &["autostart", "off"]).status.success());
    daemon.stop();
    let daemon = start_daemon_env(&home, &env);
    let status = json(&home, &["status", "--json"]);
    assert_eq!(status["autostart"], false);
    assert_ne!(status["server_status"], "running", "{status}");
    assert_eq!(proc_of(&status, "8.5").0, "stopped");
    assert!(devid(&home, &["start", "all"]).status.success());
    wait_for("nginx running", || json(&home, &["status", "--json"])["server_status"] == "running");
    assert!(devid(&home, &["autostart", "on"]).status.success());

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
