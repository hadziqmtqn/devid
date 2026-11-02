//! PHP manager end to end, with fake `php`/`php-fpm` scripts so it runs without real PHP builds.

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Output},
    thread::sleep,
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_devid");

fn devid(home: &Path, args: &[&str]) -> Output {
    Command::new(BIN).args(args).env("DEVID_HOME", home).output().unwrap()
}

fn json(home: &Path, args: &[&str]) -> Value {
    let out = devid(home, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        sleep(Duration::from_millis(50));
    }
}

const IDLE_FPM: &str = "#!/bin/sh\ntrap 'exit 0' QUIT TERM\nwhile :; do sleep 0.1; done\n";
/// Like real php-fpm during startup: signals arriving in the first 1.5 s are lost.
const SLOW_START_FPM: &str = "#!/bin/sh\ntrap '' QUIT TERM\nsleep 1.5\ntrap 'exit 0' QUIT TERM\nwhile :; do sleep 0.1; done\n";

/// A build folder whose `php` reports `version` and whose `php-fpm` runs `fpm`.
fn fake_build(root: &Path, version: &str) -> PathBuf {
    fake_build_with(root, version, IDLE_FPM)
}

fn fake_build_with(root: &Path, version: &str, fpm: &str) -> PathBuf {
    let dir = root.join(format!("build-{version}"));
    fs::create_dir_all(dir.join("modules")).unwrap();
    let scripts = [("php", format!("#!/bin/sh\necho {version}\n")), ("php-fpm", fpm.to_string())];
    for (name, body) in scripts {
        fs::write(dir.join(name), body).unwrap();
        fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(dir.join("modules/xdebug.so"), b"").unwrap();
    dir
}

/// (status, pid) of one version in `devid php list --json`-style output.
fn proc_of(status: &Value, version: &str) -> (String, Option<u64>) {
    let p = status["php"].as_array().unwrap().iter().find(|p| p["version"] == version).expect("version listed");
    (p["status"].as_str().unwrap().to_string(), p["pid"].as_u64())
}

fn alive(pid: u64) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success()
}

#[test]
fn php_install_use_supervise() {
    let root = std::env::temp_dir().join(format!("devid-php-it-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let (b84, b85) = (fake_build(&root, "8.4"), fake_build(&root, "8.5"));

    let mut daemon: Child = Command::new(BIN).args(["daemon", "run"]).env("DEVID_HOME", &home).spawn().unwrap();
    wait_for("daemon", || devid(&home, &["status"]).status.success());
    assert!(home.join("ca/cacert.pem").exists(), "cacert.pem is created at start");

    // Validation.
    assert!(stderr(&devid(&home, &["php", "use", "8.4"])).contains("not installed"));
    assert!(stderr(&devid(&home, &["php", "install", "8.1", "--from", b84.to_str().unwrap()])).contains("not supported"));
    assert!(stderr(&devid(&home, &["php", "install", "8.4", "--from", b85.to_str().unwrap()])).contains("not 8.4"));

    // Install both; the default (8.5) starts, 8.4 stays stopped.
    for (v, b) in [("8.4", &b84), ("8.5", &b85)] {
        assert!(devid(&home, &["php", "install", v, "--from", b.to_str().unwrap()]).status.success());
    }
    let ini = fs::read_to_string(home.join("php/8.5/etc/php.ini")).unwrap();
    assert!(ini.contains(&format!("curl.cainfo = {}", home.join("ca/cacert.pem").display())));
    assert!(fs::read_to_string(home.join("php/8.5/etc/php-fpm.conf")).unwrap().contains("pm = ondemand"));
    assert!(home.join("php/8.5/modules/xdebug.so").exists());

    wait_for("8.5 running", || proc_of(&json(&home, &["status", "--json"]), "8.5").0 == "running");
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.4").0, "stopped");

    // Switch the default: 8.4 starts, 8.5 stops, and the choice is saved.
    let (_, old85) = proc_of(&json(&home, &["status", "--json"]), "8.5");
    assert!(devid(&home, &["php", "use", "8.4"]).status.success());
    wait_for("8.4 running", || proc_of(&json(&home, &["status", "--json"]), "8.4").0 == "running");
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.5").0, "stopped");
    assert!(!alive(old85.unwrap()), "old 8.5 master was stopped");
    assert!(fs::read_to_string(home.join("config.toml")).unwrap().contains("default_php = \"8.4\""));

    // A crashed master is restarted with a new pid.
    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.4");
    Command::new("kill").args(["-9", &pid.unwrap().to_string()]).status().unwrap();
    wait_for("8.4 restarted", || {
        let (s, p) = proc_of(&json(&home, &["status", "--json"]), "8.4");
        s == "running" && p != pid
    });

    // Stopping the daemon stops PHP too.
    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.4");
    assert!(devid(&home, &["daemon", "stop"]).status.success());
    wait_for("daemon exit", || daemon.try_wait().unwrap().is_some());
    wait_for("php-fpm exit", || !alive(pid.unwrap()));

    fs::remove_dir_all(&root).unwrap();
}

/// Regression: stopping right after php-fpm starts must not hang the daemon (signals lost at startup).
#[test]
fn stop_during_php_startup_does_not_hang() {
    let root = std::env::temp_dir().join(format!("devid-php-stop-it-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let build = fake_build_with(&root, "8.3", SLOW_START_FPM);

    let mut daemon = Command::new(BIN).args(["daemon", "run"]).env("DEVID_HOME", &home).spawn().unwrap();
    wait_for("daemon", || devid(&home, &["status"]).status.success());
    assert!(devid(&home, &["php", "install", "8.3", "--from", build.to_str().unwrap()]).status.success());
    assert!(devid(&home, &["php", "use", "8.3"]).status.success());
    wait_for("8.3 running", || proc_of(&json(&home, &["status", "--json"]), "8.3").0 == "running");
    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.3");

    let stop = Instant::now();
    assert!(devid(&home, &["daemon", "stop"]).status.success());
    wait_for("daemon exit", || daemon.try_wait().unwrap().is_some());
    assert!(stop.elapsed() < Duration::from_secs(5), "stop took {:?}", stop.elapsed());
    assert!(!alive(pid.unwrap()));

    fs::remove_dir_all(&root).unwrap();
}
