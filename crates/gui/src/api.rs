//! The web view's whole API (D-023). It has two kinds of command:
//! - **Daemon:** one command that sends a daemon request, from an allowlist.
//! - **macOS actions:** each one checks its target against the daemon's own data, so a page can
//!   open only Devid's sites, folders and files.
//!
//! Every command is async, so it runs off the main thread.

use devid_daemon::{
    install,
    paths::Paths,
    protocol::{Request, request},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Any daemon request the CLI and TUI send, except `stop` (the daemon itself) and `login_item`
/// (which names a program to run at login; `login_item` below sends it for this app only).
/// Installs always download from Devid's release; installing from a local archive (`from`)
/// stays a CLI option. After a change, the menu-bar icon shows the new state right away.
#[tauri::command]
pub async fn daemon(app: tauri::AppHandle, req: Request) -> Result<Value, String> {
    let read = is_read(&req);
    let req = match req {
        Request::Stop => return Err("stopping the daemon is not available in the app".into()),
        Request::LoginItem { .. } => return Err("use the app's login_item command".into()),
        Request::PhpInstall { version, .. } => Request::PhpInstall { version, from: None },
        Request::ServerInstall { server, .. } => Request::ServerInstall { server, from: None },
        req => req,
    };
    let paths = Paths::from_env();
    let reply = request(&paths, &req).map_err(|e| {
        too_old(e, || {
            let status = request(&paths, &Request::Status).unwrap_or_default();
            status["version"].as_str().unwrap_or("?").to_string()
        })
    });
    if !read {
        crate::tray::refresh(&app);
    }
    reply
}

fn is_read(req: &Request) -> bool {
    matches!(
        req,
        Request::Status
            | Request::Sites
            | Request::PhpList
            | Request::ServerList
            | Request::Logs
            | Request::LogRead { .. }
            | Request::PhpExtCatalog { .. }
            | Request::PhpExtList { .. }
            | Request::PhpExtProgress
            | Request::Update { check: true }
    )
}

/// Opens this app's menu-bar icon at login, or stops doing so (D-031). The program is always
/// this app itself, never one the page names.
#[tauri::command]
pub async fn login_item(enabled: bool) -> Result<Value, String> {
    let app = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
    request(&Paths::from_env(), &Request::LoginItem { enabled, app: Some(app) })
}

/// An older daemon rejects a request it does not know with a serde error; say what to do instead.
fn too_old(err: String, running: impl FnOnce() -> String) -> String {
    if !err.starts_with("bad request: unknown variant") {
        return err;
    }
    format!(
        "The running daemon is Devid {}, which does not know this action yet. Update it with `devid self-update` (this app is {}).",
        running(),
        env!("CARGO_PKG_VERSION")
    )
}

#[derive(Deserialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum Open {
    /// A site's URL, in the default browser.
    Site { name: String },
    /// A site or parked folder, in Terminal.
    Terminal { path: String },
    /// A site or parked folder, the data folder or a server config, selected in Finder.
    Finder { path: String },
    /// A PHP version's php.ini, in the default text editor.
    PhpIni { version: String },
}

/// Opens something with macOS's `open`; returns the message to show, in the TUI's wording.
#[tauri::command]
pub async fn open(target: Open) -> Result<String, String> {
    let paths = Paths::from_env();
    // Full paths: an app started from a terminal inherits its PATH, where a terminal's own `open`
    // (cmux puts one first) can send URLs to its in-app browser instead of the default browser.
    let mut cmd = Command::new("/usr/bin/open");
    let done = match target {
        Open::Site { name } => {
            let url = site_url(&paths, &name)?;
            cmd.arg(&url);
            format!("Opened {url}")
        }
        Open::Terminal { path } => {
            known(&paths, &path, false)?;
            cmd.args(["-a", "Terminal"]).arg(&path);
            format!("Opened {path} in Terminal")
        }
        Open::Finder { path } => {
            known(&paths, &path, true)?;
            cmd.arg("-R").arg(&path);
            format!("Showed {path} in Finder")
        }
        Open::PhpIni { version } => {
            let list = request(&paths, &Request::PhpList)?;
            let ini = items(&list["php"])
                .find(|p| p["version"] == version.as_str())
                .and_then(|p| p["ini"].as_str())
                .ok_or(format!("PHP {version} is not installed"))?
                .to_string();
            cmd.arg("-t").arg(&ini);
            format!("Opened {ini}")
        }
    };
    run(&mut cmd)?;
    Ok(done)
}

/// Copies a site's URL, or with `share` its trycloudflare.com link (D-037), both as the daemon
/// reports them.
#[tauri::command]
pub async fn copy_url(name: String, share: Option<bool>) -> Result<String, String> {
    let paths = Paths::from_env();
    let url = match share {
        Some(true) => {
            let sites = request(&paths, &Request::Sites)?;
            items(&sites["sites"])
                .find(|s| s["name"] == name.as_str())
                .and_then(|s| s["share"]["url"].as_str())
                .map(String::from)
                .ok_or(format!("{name} is not shared"))?
        }
        _ => site_url(&paths, &name)?,
    };
    let mut child = Command::new("/usr/bin/pbcopy").stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    child.stdin.take().expect("piped stdin").write_all(url.as_bytes()).map_err(|e| e.to_string())?;
    match child.wait() {
        Ok(s) if s.success() => Ok(format!("Copied {url}")),
        _ => Err("Could not copy to the clipboard".into()),
    }
}

/// The active or chosen web server's generated config, read-only.
#[tauri::command]
pub async fn server_config(server: String) -> Result<Value, String> {
    let list = request(&Paths::from_env(), &Request::ServerList)?;
    let path = items(&list["servers"])
        .find(|s| s["server"] == server.as_str())
        .and_then(|s| s["config"].as_str())
        .ok_or(format!("unknown web server {server}"))?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(json!({"path": path, "text": text}))
}

/// macOS's folder picker. `None` when the user cancels.
#[tauri::command]
pub async fn choose_folder(window: tauri::WebviewWindow, purpose: String) -> Result<Option<String>, String> {
    // Fixed prompts: nothing from the page reaches the AppleScript source.
    let prompt = match purpose.as_str() {
        "link" => "Choose a folder to serve as a site",
        "park" => "Choose a folder whose folders become sites",
        _ => return Err(format!("unknown folder purpose {purpose}")),
    };
    let script = format!("POSIX path of (choose folder with prompt \"{prompt}\")");
    let out = Command::new("/usr/bin/osascript").args(["-e", &script]).output().map_err(|e| e.to_string());
    // The picker belongs to osascript. When it closes, macOS activates the app that was active
    // before, never this Accessory app, so bring the window back.
    let _ = window.set_focus();
    let out = out?;
    if !out.status.success() {
        return Ok(None); // cancelled
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(Some(if path.len() > 1 { path.trim_end_matches('/').to_string() } else { path }))
}

/// `devid doctor`'s checks. They look at PATH as Terminal sees it, so they run in the user's
/// interactive login shell, as `devid doctor` would in Terminal.
#[tauri::command]
pub async fn doctor() -> Result<Value, String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let out = output(Command::new(shell).args(["-ilc", "\"$0\" doctor"]).arg(devid_bin()), Duration::from_secs(30))?;
    let checks = doctor_checks(&out.stdout);
    if checks.is_empty() {
        return Err(first_line(&out.stderr, "`devid doctor` printed no checks"));
    }
    Ok(Value::Array(checks))
}

fn doctor_checks(text: &str) -> Vec<Value> {
    let mut checks: Vec<Value> = Vec::new();
    for line in text.lines() {
        if let Some(what) = line.strip_prefix("ok       ") {
            checks.push(json!({"ok": true, "what": what, "fix": ""}));
        } else if let Some(what) = line.strip_prefix("PROBLEM  ") {
            checks.push(json!({"ok": false, "what": what, "fix": ""}));
        } else if let (Some(fix), Some(last)) = (line.trim_start().strip_prefix("fix: "), checks.last_mut()) {
            last["fix"] = fix.into();
        }
    }
    checks
}

/// `devid self-update --check`: its one line, as the CLI prints it.
#[tauri::command]
pub async fn devid_check() -> Result<String, String> {
    let out = output(Command::new(devid_bin()).args(["self-update", "--check"]), Duration::from_secs(60))?;
    if out.ok {
        Ok(first_line(&out.stdout, "no answer"))
    } else {
        Err(first_line(&out.stderr, "`devid self-update --check` failed"))
    }
}

/// Runs `devid setup`, `self-update` or `uninstall` in Terminal: they may ask for the sudo
/// password and print their steps, as from the TUI. macOS runs a `.command` file in Terminal.
#[tauri::command]
pub async fn in_terminal(action: String) -> Result<String, String> {
    if !["setup", "self-update", "uninstall"].contains(&action.as_str()) {
        return Err(format!("unknown action {action}"));
    }
    let paths = Paths::from_env();
    let script = paths.run_dir().join(format!("devid-{action}.command"));
    let quoted = devid_bin().display().to_string().replace('\'', r"'\''");
    std::fs::write(&script, format!("#!/bin/sh\nexec '{quoted}' {action}\n")).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    run(Command::new("/usr/bin/open").arg(&script))?;
    Ok(format!("Opened Terminal for `devid {action}`"))
}

fn devid_bin() -> std::path::PathBuf {
    install::bin_dir(&Paths::from_env()).join("devid")
}

fn items(list: &Value) -> impl Iterator<Item = &Value> {
    list.as_array().into_iter().flatten()
}

fn site_url(paths: &Paths, name: &str) -> Result<String, String> {
    let sites = request(paths, &Request::Sites)?;
    items(&sites["sites"])
        .find(|s| s["name"] == name)
        .and_then(|s| s["url"].as_str())
        .map(String::from)
        .ok_or(format!("no site named {name}"))
}

/// Only paths the daemon knows: sites and parked folders, plus (for Finder) the data folder and
/// the server configs.
fn known(paths: &Paths, path: &str, finder: bool) -> Result<(), String> {
    let sites = request(paths, &Request::Sites)?;
    let mut ok: Vec<&str> = items(&sites["sites"]).filter_map(|s| s["path"].as_str()).collect();
    ok.extend(items(&sites["parked"]).filter_map(Value::as_str));
    let home = paths.home.display().to_string();
    let servers = if finder { request(paths, &Request::ServerList)? } else { Value::Null };
    if finder {
        ok.push(&home);
        ok.extend(items(&servers["servers"]).filter_map(|s| s["config"].as_str()));
    }
    if ok.contains(&path) { Ok(()) } else { Err(format!("{path} is not a Devid site, folder or file")) }
}

fn run(cmd: &mut Command) -> Result<(), String> {
    match cmd.stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("failed ({s})")),
        Err(e) => Err(e.to_string()),
    }
}

struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

/// Runs `cmd` and gives up after `limit`.
// shortcut: pipes are read only after the child exits, so a child printing more than the pipe
// buffer (64 KB) would time out; fine for doctor and `--check`, which print a few lines.
fn output(cmd: &mut Command, limit: Duration) -> Result<Output, String> {
    let mut child =
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if start.elapsed() > limit {
            let _ = child.kill();
            return Err(format!("no answer after {} s", limit.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

fn first_line(text: &str, otherwise: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(otherwise);
    line.strip_prefix("error: ").or_else(|| line.strip_prefix("devid: ")).unwrap_or(line).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_output_becomes_checks_with_their_fixes() {
        let text = "ok       daemon answers\nPROBLEM  DNS stopped\n         fix: change dns_port\nok       Devid CA exists\n\nall good\n";
        let checks = doctor_checks(text);
        assert_eq!(checks.len(), 3);
        assert_eq!(checks[1], json!({"ok": false, "what": "DNS stopped", "fix": "change dns_port"}));
        assert_eq!(checks[2]["fix"], "", "a fix belongs to the problem above it only");
    }

    #[test]
    fn an_older_daemon_gets_an_update_hint() {
        let old = too_old("bad request: unknown variant `ignore`, expected one of `status`".into(), || "0.4.1".into());
        assert!(old.starts_with("The running daemon is Devid 0.4.1"), "{old}");
        assert!(old.contains("`devid self-update`"));
        assert_eq!(
            too_old("no site named x".into(), || unreachable!()),
            "no site named x",
            "other errors pass through"
        );
    }

    #[test]
    fn only_changes_redraw_the_menu_bar() {
        assert!(is_read(&Request::Status) && is_read(&Request::Update { check: true }));
        assert!(!is_read(&Request::StopAll) && !is_read(&Request::Update { check: false }));
    }
}
