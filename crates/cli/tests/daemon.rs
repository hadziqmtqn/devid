//! End to end: start the real daemon in a temporary DEVID_HOME, talk to it with the CLI, stop it.

use std::{
    path::PathBuf,
    process::{Command, Output},
    thread::sleep,
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_devid");

fn devid(home: &PathBuf, args: &[&str]) -> Output {
    Command::new(BIN).args(args).env("DEVID_HOME", home).output().unwrap()
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out waiting for {what}");
        sleep(Duration::from_millis(50));
    }
}

#[test]
fn daemon_lifecycle() {
    let home = std::env::temp_dir().join(format!("devid-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let sock = home.join("run/devid.sock");

    // Without a daemon, status fails with a helpful message.
    let out = devid(&home, &["status"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("daemon is not running"));

    let mut daemon = Command::new(BIN).args(["daemon", "run"]).env("DEVID_HOME", &home).spawn().unwrap();
    wait_for("socket", || sock.exists());

    let out = devid(&home, &["status", "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["ok"], true);
    assert_eq!(status["server"], "nginx");
    assert_eq!(status["pid"], daemon.id());
    assert!(home.join("config.toml").exists(), "config is created on first start");

    // A second daemon refuses to start while the first one runs.
    let out = devid(&home, &["daemon", "run"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already running"));

    assert!(devid(&home, &["daemon", "stop"]).status.success());
    wait_for("daemon exit", || daemon.try_wait().unwrap().is_some());
    assert!(!sock.exists(), "socket is removed on stop");

    // A stale socket file (daemon crashed) does not block the next start.
    std::fs::write(&sock, b"").unwrap();
    let mut daemon = Command::new(BIN).args(["daemon", "run"]).env("DEVID_HOME", &home).spawn().unwrap();
    wait_for("status after stale socket", || devid(&home, &["status"]).status.success());
    assert!(devid(&home, &["daemon", "stop"]).status.success());
    wait_for("daemon exit", || daemon.try_wait().unwrap().is_some());

    std::fs::remove_dir_all(&home).unwrap();
}
