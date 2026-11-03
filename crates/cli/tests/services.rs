//! T4: stop/start services (they stay stopped across site changes) and logs (list, tail, clear).

mod common;

use common::*;
use std::fs;

fn status_of(home: &std::path::Path) -> (String, String, Vec<String>) {
    let s = json(home, &["status", "--json"]);
    let php = proc_of(&s, "8.5").0;
    let paused = s["paused"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    (php, s["server_status"].as_str().unwrap().to_string(), paused)
}

#[test]
fn stop_start_and_logs() {
    let root = temp_root("devid-services-it");
    let home = root.join("home");
    fs::create_dir_all(root.join("projects/blog")).unwrap();
    let (php, nginx) = (fake_build(&root, "8.5"), fake_nginx(&root, &home));
    let daemon = start_daemon(&home);
    assert!(devid(&home, &["php", "install", "8.5", "--from", php.to_str().unwrap()]).status.success());
    assert!(devid(&home, &["server", "install", "nginx", "--from", nginx.to_str().unwrap()]).status.success());
    wait_for("all running", || status_of(&home) == ("running".into(), "running".into(), vec![]));

    // Stop one: it stays stopped even when a site change re-applies the config.
    assert!(devid(&home, &["stop", "php-8.5"]).status.success());
    wait_for("8.5 stopped", || status_of(&home).0 == "stopped");
    assert_eq!(status_of(&home).2, ["php-8.5"]);
    assert!(devid(&home, &["park", root.join("projects").to_str().unwrap()]).status.success());
    assert_eq!(status_of(&home).0, "stopped", "a site change does not restart a stopped service");
    assert!(stderr(&devid(&home, &["restart", "php-8.5"])).contains("is not running"));
    assert!(devid(&home, &["start", "php-8.5"]).status.success());
    wait_for("8.5 running again", || status_of(&home) == ("running".into(), "running".into(), vec![]));

    // Stop all / start all.
    assert!(devid(&home, &["stop", "all"]).status.success());
    wait_for("all stopped", || {
        let (p, n, _) = status_of(&home);
        p == "stopped" && n == "stopped"
    });
    assert!(devid(&home, &["start", "all"]).status.success());
    wait_for("all running", || status_of(&home) == ("running".into(), "running".into(), vec![]));
    assert!(stderr(&devid(&home, &["stop", "mysql"])).contains("unknown service"));

    // Logs.
    fs::write(home.join("log/app-test.log"), "first\nsecond\nthird\n").unwrap();
    let list = String::from_utf8_lossy(&devid(&home, &["logs"]).stdout).into_owned();
    assert!(list.contains("app-test.log") && list.contains("19 bytes"), "{list}");
    let tail = String::from_utf8_lossy(&devid(&home, &["logs", "app-test.log", "-n", "2"]).stdout).into_owned();
    assert_eq!(tail, "second\nthird\n");
    assert!(devid(&home, &["logs", "app-test.log", "--clear"]).status.success());
    assert_eq!(fs::read_to_string(home.join("log/app-test.log")).unwrap(), "");
    assert!(stderr(&devid(&home, &["logs", "../config.toml"])).contains("not a log name"));

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
