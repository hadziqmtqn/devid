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

/// Share (D-037) with a fake cloudflared that prints its quick-tunnel URL the way the real one
/// does; nothing reaches the internet.
#[test]
fn share_runs_cloudflared_and_reads_its_url() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp_root("devid-share-it");
    let home = root.join("home");
    fs::create_dir_all(root.join("projects/app")).unwrap();
    let fake = root.join("cloudflared");
    fs::write(
        &fake,
        "#!/bin/sh\necho \"$*\" > \"$DEVID_HOME/cloudflared-args\"\nsleep 1\n\
         echo '2026-11-08T10:00:02Z INF |  https://test-lamp-river.trycloudflare.com    |' >&2\nsleep 300\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let env = [("SHELL", "/bin/sh"), ("DEVID_CLOUDFLARED", fake.to_str().unwrap())];
    let daemon = start_daemon_env(&home, &env);
    ok(&home, &["park", root.join("projects").to_str().unwrap()]);

    let out = devid(&home, &["share", "app"]);
    assert!(!out.status.success() && stderr(&out).contains("is not running"), "no web server: {}", stderr(&out));
    let nginx = fake_nginx(&root, &home);
    ok(&home, &["server", "install", "nginx", "--from", nginx.to_str().unwrap()]);

    let out = devid(&home, &["share", "app"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("https://test-lamp-river.trycloudflare.com  →  app"));
    let args = fs::read_to_string(home.join("cloudflared-args")).unwrap();
    assert_eq!(
        args.trim(),
        format!("tunnel --no-autoupdate --url http://127.0.0.1:{TEST_HTTP_PORT} --http-host-header app.test")
    );
    let share = site(&home)["share"].clone();
    assert_eq!((share["state"].as_str(), share["log"].as_str()), (Some("running"), Some("share-app.log")));
    let status = json(&home, &["status", "--json"]);
    assert_eq!(status["shares"][0]["url"], "https://test-lamp-river.trycloudflare.com");
    assert_eq!(status["commands_running"], 0, "a share is not a command");

    ok(&home, &["cmd", "stop", "app"]);
    assert_eq!(site(&home)["share"]["state"], "running", "stopping the site's commands leaves its share");

    // A site with a local port is shared through that port, not the web server's main one.
    ok(&home, &["share", "app", "--stop"]);
    ok(&home, &["app", "port", "app"]);
    let port = site(&home)["port"].as_u64().unwrap();
    ok(&home, &["share", "app"]);
    let args = fs::read_to_string(home.join("cloudflared-args")).unwrap();
    assert!(args.contains(&format!("--url http://127.0.0.1:{port} --http-host-header app.test")), "{args}");
    let pid =
        String::from_utf8_lossy(&Command::new("pgrep").args(["-f", fake.to_str().unwrap()]).output().unwrap().stdout)
            .lines()
            .next()
            .unwrap()
            .to_string();
    let out = devid(&home, &["share", "app", "--stop"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("stopped sharing app"));
    assert!(site(&home)["share"].is_null());
    // A zombie answers kill -0 until the daemon's watcher reaps it (within a second).
    wait_for("cloudflared to be gone", || !Command::new("kill").args(["-0", &pid]).status().unwrap().success());
    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}

/// A pid file left by a crashed daemon names a process group; if that pid now belongs to another
/// program (a different start time), the next daemon must leave it alone and drop the file.
#[test]
fn a_reused_pid_is_never_stopped() {
    use std::os::unix::process::CommandExt;
    let root = temp_root("devid-pidreuse-it");
    let home = root.join("home");
    fs::create_dir_all(home.join("run")).unwrap();
    // Another program, leading its own process group like a command would.
    let mut other = Command::new("sleep").arg("300").process_group(0).spawn().unwrap();
    let pid = other.id();
    fs::write(home.join("run/app-shop-dev.pid"), format!("{pid} Mon Jan  1 00:00:00 2001\n")).unwrap();

    let daemon = start_daemon_env(&home, &[("SHELL", "/bin/sh")]);
    assert!(!home.join("run/app-shop-dev.pid").exists(), "the stale pid file is removed");
    // The daemon decides before it answers; a wrong decision would signal at once. Watch for a second.
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until {
        assert!(other.try_wait().unwrap().is_none(), "the other program was stopped");
        std::thread::sleep(Duration::from_millis(50));
    }
    daemon.stop();
    let _ = other.kill();
    let _ = other.wait();
    fs::remove_dir_all(&root).unwrap();
}

/// When the login shell's environment cannot be read, the command still starts with the daemon's,
/// and its log says why, instead of failing silently.
#[test]
fn a_broken_login_shell_is_named_in_the_log() {
    let root = temp_root("devid-badshell-it");
    let home = root.join("home");
    fs::create_dir_all(root.join("projects/app")).unwrap();
    let daemon = start_daemon_env(&home, &[("SHELL", "/usr/bin/false")]);
    ok(&home, &["park", root.join("projects").to_str().unwrap()]);
    ok(&home, &["cmd", "add", "app", "dev", "--", "echo hello"]);
    ok(&home, &["cmd", "start", "app", "dev"]);
    wait_for("dev to end", || command(&home, "dev")["state"] == "exited");
    let log = fs::read_to_string(home.join("log/app-app-dev.log")).unwrap();
    assert!(log.starts_with("devid: could not read your login shell's environment (/usr/bin/false -l failed"), "{log}");
    assert!(log.contains("$ echo hello"));
    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}

/// A cloudflared that gives up (no quick tunnel) is reported with the end of its log.
#[test]
fn a_failed_share_reports_cloudflareds_error() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp_root("devid-sharefail-it");
    let home = root.join("home");
    fs::create_dir_all(root.join("projects/app")).unwrap();
    let fake = root.join("cloudflared");
    fs::write(
        &fake,
        "#!/bin/sh\necho 'ERR failed to request quick Tunnel: Post \"https://api.trycloudflare.com/tunnel\": dial tcp: i/o timeout' >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let daemon = start_daemon_env(&home, &[("SHELL", "/bin/sh"), ("DEVID_CLOUDFLARED", fake.to_str().unwrap())]);
    ok(&home, &["park", root.join("projects").to_str().unwrap()]);
    let nginx = fake_nginx(&root, &home);
    ok(&home, &["server", "install", "nginx", "--from", nginx.to_str().unwrap()]);

    let out = devid(&home, &["share", "app"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("cloudflared stopped") && err.contains("failed to request quick Tunnel"), "{err}");
    assert!(!err.contains("trycloudflare.com  →"), "the API endpoint is not mistaken for a link");
    assert_eq!(site(&home)["share"]["state"], "exited");
    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}

/// Starting a command of a proxy site whose dev server port another program holds warns at once,
/// instead of leaving only EADDRINUSE in the command's log. With one of the site's own commands
/// running, the port may be that command's, so there is no warning.
#[test]
fn a_busy_dev_server_port_is_named_when_a_command_starts() {
    let root = temp_root("devid-portwarn-it");
    let home = root.join("home");
    fs::create_dir_all(root.join("projects/app")).unwrap();
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port().to_string();
    let daemon = start_daemon_env(&home, &[("SHELL", "/bin/sh")]);
    ok(&home, &["park", root.join("projects").to_str().unwrap()]);
    ok(&home, &["app", "set", "app", "--mode", "proxy", "--upstream", &port]);
    ok(&home, &["cmd", "add", "app", "dev", "--", "sleep 300"]);
    ok(&home, &["cmd", "add", "app", "queue", "--", "sleep 300"]);

    let out = devid(&home, &["cmd", "start", "app", "dev"]);
    assert!(out.status.success(), "a warning does not stop the start: {}", stderr(&out));
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(printed.contains(&format!("warning: port {port} is already in use by")), "{printed}");
    assert!(printed.contains("which Devid did not start"), "{printed}");
    assert_eq!(command(&home, "dev")["state"], "running");

    let out = devid(&home, &["cmd", "start", "app", "queue"]);
    assert!(!String::from_utf8_lossy(&out.stdout).contains("warning:"), "the port may be dev's");
    drop(held);
    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
