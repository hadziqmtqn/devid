//! User commands (D-036, Phase 8 A4): saved per site, run by the daemon in their own process
//! group with output in a log, stopped completely, and stopped after a daemon crash.

mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn site(home: &Path) -> Value {
    let list = json(home, &["sites", "--json"]);
    list.as_array().unwrap().iter().find(|s| s["name"] == "app").unwrap().clone()
}

fn command(home: &Path, name: &str) -> Value {
    site(home)["commands"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap().clone()
}

/// Whether any process of the group is left.
fn group_alive(pgid: &Value) -> bool {
    Command::new("pgrep").args(["-g", &pgid.to_string()]).output().unwrap().status.success()
}

fn ok(home: &Path, args: &[&str]) {
    let out = devid(home, args);
    assert!(out.status.success(), "{args:?}: {}", stderr(&out));
}

#[test]
fn commands_start_stop_and_survive_nothing() {
    let root = temp_root("devid-cmd-it");
    let home = root.join("home");
    let app = root.join("projects/app");
    fs::create_dir_all(&app).unwrap();
    fs::write(app.join("package.json"), r#"{"scripts": {"dev": "x", "build": "x"}}"#).unwrap();
    // A plain login shell, so the result does not depend on the user's profile.
    let env = [("SHELL", "/bin/sh")];
    let mut daemon = start_daemon_env(&home, &env);
    ok(&home, &["park", root.join("projects").to_str().unwrap()]);
    assert_eq!(site(&home)["suggestions"], json!([{"name": "dev", "run": "npm run dev"}]));

    ok(&home, &["cmd", "add", "app", "srv", "--", "sleep 300"]);
    // Ignores Ctrl+C, and so does its child: only SIGTERM (after 5 s) stops them.
    ok(&home, &["cmd", "add", "app", "stubborn", "--", "trap '' INT; sleep 300 & wait"]);
    ok(&home, &["cmd", "add", "app", "fail", "--", "echo boom; exit 3"]);
    ok(&home, &["cmd", "add", "app", "env:show", "--", "echo", "dir=$PWD", "path=$PATH;", "sleep", "300"]);
    assert!(!devid(&home, &["cmd", "add", "app", "bad name", "--", "x"]).status.success());
    assert!(!devid(&home, &["cmd", "start", "app", "nope"]).status.success());
    assert_eq!(command(&home, "srv")["state"], "stopped");

    ok(&home, &["cmd", "start", "app"]);
    wait_for("fail to exit", || command(&home, "fail")["state"] == "exited");
    assert_eq!(command(&home, "fail")["code"], 3);
    let log = |name: &str| fs::read_to_string(home.join("log").join(format!("app-app-{name}.log"))).unwrap_or_default();
    assert_eq!(log("fail"), "$ echo boom; exit 3\nboom\n");
    wait_for("env:show to print", || log("env:show").contains("path="));
    let printed = log("env:show");
    assert!(printed.contains(&format!("dir={}", app.display())), "runs in the site's folder: {printed}");
    assert!(printed.contains(&format!("path={}:", home.join("bin").display())), "Devid's php first: {printed}");
    let pids: Vec<Value> = ["srv", "stubborn", "env:show"].iter().map(|n| command(&home, n)["pid"].clone()).collect();
    assert!(pids.iter().all(group_alive), "{pids:?}");
    assert!(home.join("run/app-app-srv.pid").exists());

    ok(&home, &["cmd", "stop", "app", "srv"]);
    assert_eq!(command(&home, "srv")["state"], "stopped");
    assert!(!group_alive(&pids[0]) && !home.join("run/app-app-srv.pid").exists());
    assert!(group_alive(&pids[1]), "the others keep running");

    let start = Instant::now();
    let out = devid(&home, &["cmd", "stop", "app"]);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(printed.contains("stopped env:show, stubborn"), "{printed}{}", stderr(&out));
    assert!(start.elapsed() >= Duration::from_secs(5), "stubborn needed SIGTERM");
    assert!(pids.iter().all(|p| !group_alive(p)), "nothing is left");
    assert_eq!(command(&home, "fail")["state"], "stopped", "stop all also clears an exited state");

    // A crashed daemon leaves its commands running; the next one stops them.
    ok(&home, &["cmd", "start", "app", "srv"]);
    let srv = command(&home, "srv")["pid"].clone();
    let daemon_pid = json(&home, &["status", "--json"])["pid"].to_string();
    assert!(Command::new("kill").args(["-9", &daemon_pid]).status().unwrap().success());
    wait_for("the daemon to die", || daemon.exited());
    assert!(group_alive(&srv), "its command outlives it");
    let daemon = start_daemon_env(&home, &env);
    wait_for("the leftover to stop", || !group_alive(&srv));
    assert!(!home.join("run/app-app-srv.pid").exists());
    assert_eq!(command(&home, "srv")["state"], "stopped");

    // `app reset` keeps the commands; `cmd rm` stops a running one first.
    ok(&home, &["app", "set", "app", "--mode", "static"]);
    ok(&home, &["app", "reset", "app"]);
    assert_eq!(site(&home)["commands"].as_array().unwrap().len(), 4);
    ok(&home, &["cmd", "start", "app", "srv"]);
    let srv = command(&home, "srv")["pid"].clone();
    ok(&home, &["cmd", "rm", "app", "srv"]);
    assert!(!group_alive(&srv));
    assert_eq!(site(&home)["commands"].as_array().unwrap().len(), 3);

    // Stopping the daemon stops every command.
    ok(&home, &["cmd", "start", "app", "env:show"]);
    let env_show = command(&home, "env:show")["pid"].clone();
    daemon.stop();
    assert!(!group_alive(&env_show));
    fs::remove_dir_all(&root).unwrap();
}
