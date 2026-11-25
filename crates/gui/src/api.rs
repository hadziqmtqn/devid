//! The web view's whole API (D-023). It has two kinds of command:
//! - **Daemon:** one command that sends a daemon request, from an allowlist.
//! - **macOS actions:** each one checks its target against the daemon's own data, so a page can
//!   open only Devid's sites, folders and files.
//!
//! Every command is async, so it runs off the main thread.

use devid_daemon::{
    db::{self, Engine},
    install,
    paths::Paths,
    protocol::{Request, request},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
    if let (Request::MailMessage { .. }, Ok(v)) = (&req, &reply) {
        crate::mailview::opened(v);
    }
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
            | Request::MailList { .. }
            | Request::MailHeaders { .. }
            | Request::MailRaw { .. }
            | Request::DbList { .. }
            | Request::DbUserList { .. }
            | Request::DbEnv { .. }
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
    /// A site, a parked folder or a removed site's folder, in Terminal.
    Terminal { path: String },
    /// A site or parked folder, the data folder, a server config or a log file, selected in Finder.
    Finder { path: String },
    /// A PHP version's php.ini, in the default text editor.
    PhpIni { version: String },
    /// Mailpit's own page (D-049), or one message there, in the default browser.
    Inbox { id: Option<String> },
}

/// The inbox of a running Mail as the daemon reports it, and only Mailpit's address on 127.0.0.1.
pub fn inbox_url(paths: &Paths) -> Result<String, String> {
    let status = request(paths, &Request::Status)?;
    let m = &status["mail"];
    if m["state"] != "running" {
        return Err(if m["enabled"] == true { "Mail is not running".into() } else { "Mail is off".into() });
    }
    m["inbox_url"]
        .as_str()
        .filter(|u| u.starts_with("http://127.0.0.1:"))
        .map(String::from)
        .ok_or_else(|| "the daemon gave no inbox address".into())
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
        Open::Inbox { id } => {
            let mut url = inbox_url(&paths)?;
            if let Some(id) = id {
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    return Err(format!("{id:?} is not a message ID"));
                }
                url = format!("{url}view/{id}");
            }
            cmd.arg(&url);
            format!("Opened {url} in the browser")
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
    pbcopy(&url).map(|_| format!("Copied {url}"))
}

/// Copies the `.env` lines a Laravel app needs to send to Mail (D-049), with the daemon's port.
#[tauri::command]
pub async fn copy_mail_env() -> Result<String, String> {
    let status = request(&Paths::from_env(), &Request::Status)?;
    let port = status["mail"]["port"].as_u64().ok_or("this daemon has no Mail; update Devid")?;
    pbcopy(&format!("MAIL_MAILER=smtp\nMAIL_HOST=127.0.0.1\nMAIL_PORT={port}\n"))
        .map(|_| "Copied the .env lines".into())
}

/// Copies the full path of a log the daemon lists.
#[tauri::command]
pub async fn copy_log_path(name: String) -> Result<String, String> {
    let paths = Paths::from_env();
    let path = log_paths(&paths)?
        .into_iter()
        .find(|p| p.ends_with(&format!("/{name}")))
        .ok_or(format!("no log named {name}"))?;
    pbcopy(&path).map(|_| format!("Copied {path}"))
}

/// Copies an account's `.env` lines, or with `password` its password alone, as `devid db env`
/// gives them (D-061): the main account's when `name` is None.
#[tauri::command]
pub async fn copy_db(engine: String, name: Option<String>, password: bool) -> Result<String, String> {
    let v = request(&Paths::from_env(), &Request::DbEnv { engine, name })?;
    let who = v["name"].as_str().unwrap_or("?").to_string();
    if password {
        let pw =
            v["password"].as_str().filter(|p| !p.is_empty()).ok_or(format!("{who} has no password from this Mac"))?;
        return pbcopy(pw).map(|_| format!("Copied {who}'s password"));
    }
    let lines: Vec<&str> = items(&v["env"]).filter_map(Value::as_str).collect();
    pbcopy(&format!("{}\n", lines.join("\n"))).map(|_| format!("Copied {who}'s .env lines"))
}

fn pbcopy(text: &str) -> Result<(), String> {
    let mut child = Command::new("/usr/bin/pbcopy").stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    child.stdin.take().expect("piped stdin").write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    match child.wait() {
        Ok(s) if s.success() => Ok(()),
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

/// `devid self-update --no-sudo` in the background, as an app updates itself: download, check,
/// install, restart the daemon and swap Devid.app; `relaunch` then opens the new version. A step
/// that needs sudo is not run; the error says so ("needs your password") and the page offers
/// Terminal for it.
#[tauri::command]
pub async fn self_update() -> Result<Value, String> {
    // A step left by an update that was cut off must not show as this one's.
    let (step, headers) = devid_daemon::releases::self_update_files(&Paths::from_env());
    let _ = std::fs::remove_file(step);
    let _ = std::fs::remove_file(headers);
    update_reply(output(Command::new(devid_bin()).args(["self-update", "--no-sudo"]), Duration::from_secs(600))?)
}

/// How far a running `devid self-update` is: its step and target version, and the bytes of the
/// download in progress (`got`) out of its announced `size`. Null when none runs.
#[tauri::command]
pub fn self_update_progress() -> Value {
    let paths = Paths::from_env();
    let Some(mut step) = std::fs::read_to_string(devid_daemon::releases::self_update_files(&paths).0)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return Value::Null;
    };
    step["got"] = step["file"].as_str().and_then(|f| std::fs::metadata(f).ok()).map(|m| m.len()).into();
    step["size"] = devid_daemon::releases::self_update_size(&paths).into();
    step
}

/// What the page needs from `devid self-update`'s output: the version it installed (none when
/// devid was already the newest), or its last error line.
fn update_reply(out: Output) -> Result<Value, String> {
    if !out.ok {
        let lines: Vec<&str> = out.stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let last = lines.last().copied().unwrap_or("`devid self-update` failed");
        return Err(last.strip_prefix("devid: ").unwrap_or(last).to_string());
    }
    // "downloading devid 0.13.0 (sha256-checked)…", or "devid 0.12.0 is the newest version".
    let installed = out
        .stdout
        .lines()
        .find_map(|l| l.strip_prefix("downloading devid "))
        .and_then(|rest| rest.split_whitespace().next())
        .map(String::from);
    Ok(json!({"installed": installed, "text": first_line(&out.stdout, "done")}))
}

/// Restart this app as the bundle now at its path, with its window on `page` (after an update).
#[tauri::command]
pub fn relaunch(app: tauri::AppHandle, page: Option<String>) {
    let page = page.and_then(|p| crate::start_page(&p));
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || match page {
        Some(p) => crate::restart(&handle, &["--page", p]),
        None => crate::restart(&handle, &[]),
    });
}

/// Quit Devid from the window's menu, as the menu-bar menu does; the sites keep running.
#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
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
    let devid = sh_quote(&devid_bin().display().to_string());
    std::fs::write(&script, format!("#!/bin/sh\nexec {devid} {action}\n")).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    run(Command::new("/usr/bin/open").arg(&script))?;
    Ok(format!("Opened Terminal for `devid {action}`"))
}

/// What macOS calls a `.command` file, and the type it belongs to.
const SCRIPT_TYPES: [&str; 2] = ["com.apple.terminal.shell-script", "public.shell-script"];

/// The terminal apps in the Applications folders, Terminal first: apps whose document types run
/// `.command` files (the `Shell` role), as Terminal, iTerm2 and cmux declare. Editors that open
/// such a file as text declare another role.
#[tauri::command]
pub async fn terminals() -> Vec<Value> {
    find_terminals(&[
        PathBuf::from("/System/Applications/Utilities"),
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        devid_daemon::paths::user_home().join("Applications"),
    ])
}

fn find_terminals(dirs: &[PathBuf]) -> Vec<Value> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for dir in dirs {
        let mut apps: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
        apps.sort();
        for app in apps {
            match terminal_name(&app) {
                Some(name) if !found.iter().any(|(n, _)| *n == name) => found.push((name, app)),
                _ => {}
            }
        }
    }
    found.sort_by_key(|(name, _)| (name != "Terminal", name.to_lowercase()));
    found.into_iter().map(|(name, app)| json!({"name": name, "path": app.display().to_string()})).collect()
}

/// An app's name when it runs `.command` files. A look at the bytes rules out most apps first: the
/// role's name is in the Info.plist as text, XML or binary.
fn terminal_name(app: &Path) -> Option<String> {
    if app.extension()? != "app" {
        return None;
    }
    let plist = app.join("Contents/Info.plist");
    if !std::fs::read(&plist).ok()?.windows(5).any(|w| w == b"Shell") {
        return None;
    }
    let mut cmd = Command::new("/usr/bin/plutil");
    cmd.args(["-extract", "CFBundleDocumentTypes", "json", "-o", "-"]).arg(&plist);
    let types: Value = serde_json::from_str(&output(&mut cmd, Duration::from_secs(5)).ok()?.stdout).ok()?;
    match runs_scripts(&types) {
        true => Some(app.file_stem()?.to_string_lossy().into_owned()),
        false => None,
    }
}

/// Whether an app's `CFBundleDocumentTypes`, as JSON, run `.command` files.
fn runs_scripts(types: &Value) -> bool {
    items(types).any(|t| {
        t["CFBundleTypeRole"] == "Shell"
            && (items(&t["LSItemContentTypes"]).any(|c| c.as_str().is_some_and(|c| SCRIPT_TYPES.contains(&c)))
                || items(&t["CFBundleTypeExtensions"]).any(|e| e.as_str() == Some("command")))
    })
}

/// Opens a running database's client in a terminal app (one that `terminals` lists), as the
/// engine's main account (D-061). A `.command` file runs Devid's shim, which reaches the server. MongoDB's `root`
/// logs in through a script only the user can read, and the script deletes itself.
#[tauri::command]
pub async fn open_db_client(engine: String, app: String) -> Result<String, String> {
    let paths = Paths::from_env();
    let eng = Engine::parse(&engine).ok_or(format!("unknown database {engine}"))?;
    let terminal = terminal_name(Path::new(&app)).ok_or(format!("{app} is not a terminal app"))?;
    let status = request(&paths, &Request::Status)?;
    if !items(&status["databases"]).any(|d| d["engine"] == eng.name() && d["state"] == "running") {
        return Err(format!("{} is not running", eng.label()));
    }
    let client = match eng {
        Engine::Mysql => "mysql",
        Engine::Postgresql => "psql",
        Engine::Mongodb => "mongosh",
    };
    let shim = install::bin_dir(&paths).join(client);
    if !shim.is_file() {
        return Err(format!("{} is missing", shim.display()));
    }
    let login = match eng {
        Engine::Mongodb => {
            let user = db::main_account(eng);
            let v = request(&paths, &Request::DbEnv { engine: engine.clone(), name: Some(user.into()) })?;
            v["password"].as_str().filter(|p| !p.is_empty()).map(|p| (user, p.to_string()))
        }
        _ => None,
    };
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let js = paths.run_dir().join(format!("devid-mongosh-{stamp}.js"));
    let home = std::env::var_os("DEVID_HOME").filter(|h| !h.is_empty());
    let (sh, script) =
        client_files(&shim, home.as_deref(), login.as_ref().map(|(u, p)| (js.as_path(), *u, p.as_str())));
    if let Some(script) = script {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&js)
            .map_err(|e| e.to_string())?;
        file.write_all(script.as_bytes()).map_err(|e| e.to_string())?;
    }
    let file = paths.run_dir().join(format!("devid-{}.command", eng.name()));
    std::fs::write(&file, sh).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    if let Err(e) = run(Command::new("/usr/bin/open").arg("-a").arg(&app).arg(&file)) {
        let _ = std::fs::remove_file(&js);
        return Err(e);
    }
    Ok(format!("Opened {client} in {terminal}"))
}

/// The `.command` file that runs a client's shim and, with a login (MongoDB), the script that the
/// client runs first. The password stays out of every command line; a JSON string is also a
/// JavaScript string literal.
fn client_files(shim: &Path, home: Option<&OsStr>, login: Option<(&Path, &str, &str)>) -> (String, Option<String>) {
    let mut sh = String::from("#!/bin/sh\n");
    if let Some(home) = home {
        sh += &format!("export DEVID_HOME={}\n", sh_quote(&home.to_string_lossy()));
    }
    let shim = sh_quote(&shim.display().to_string());
    let Some((js, user, password)) = login else {
        sh += &format!("exec {shim}\n");
        return (sh, None);
    };
    let file = js.display().to_string();
    sh += &format!("{shim} --shell --file {0}\nrm -f {0}\n", sh_quote(&file));
    let lit = |text: &str| Value::from(text).to_string();
    let script = format!(
        "try {{ require('fs').unlinkSync({file}); }} catch (e) {{}}\n\
         try {{ db.getSiblingDB('admin').auth({user}, {password}); }} \
         catch (e) {{ print('Could not log in as ' + {user} + ': ' + e.message); }}\n",
        file = lit(&file),
        user = lit(user),
        password = lit(password),
    );
    (sh, Some(script))
}

/// A word for `sh`, in single quotes.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn devid_bin() -> std::path::PathBuf {
    install::bin_dir(&Paths::from_env()).join("devid")
}

fn items(list: &Value) -> impl Iterator<Item = &Value> {
    list.as_array().into_iter().flatten()
}

pub(crate) fn site_url(paths: &Paths, name: &str) -> Result<String, String> {
    let sites = request(paths, &Request::Sites)?;
    items(&sites["sites"])
        .find(|s| s["name"] == name)
        .and_then(|s| s["url"].as_str())
        .map(String::from)
        .ok_or(format!("no site named {name}"))
}

/// The full paths of the logs the daemon lists.
fn log_paths(paths: &Paths) -> Result<Vec<String>, String> {
    let logs = request(paths, &Request::Logs)?;
    Ok(items(&logs["logs"])
        .filter_map(|l| l["name"].as_str())
        .map(|n| paths.log_dir().join(n).display().to_string())
        .collect())
}

/// Only paths the daemon knows: sites, parked folders and removed sites' folders, plus (for Finder) the data folder, the
/// server configs, the logs and the databases' data folders.
fn known(paths: &Paths, path: &str, finder: bool) -> Result<(), String> {
    let sites = request(paths, &Request::Sites)?;
    let mut ok: Vec<&str> = items(&sites["sites"]).filter_map(|s| s["path"].as_str()).collect();
    ok.extend(items(&sites["parked"]).filter_map(Value::as_str));
    ok.extend(items(&sites["ignored"]).filter_map(|i| i["path"].as_str()));
    let home = paths.home.display().to_string();
    let mail = format!("{home}/mail");
    let servers = if finder { request(paths, &Request::ServerList)? } else { Value::Null };
    let logs = if finder { log_paths(paths)? } else { Vec::new() };
    let status = if finder { request(paths, &Request::Status)? } else { Value::Null };
    if finder {
        ok.push(&home);
        ok.push(&mail);
        ok.extend(items(&servers["servers"]).filter_map(|s| s["config"].as_str()));
        ok.extend(logs.iter().map(String::as_str));
        ok.extend(items(&status["databases"]).filter_map(|d| d["data"].as_str()));
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
    fn self_update_output_becomes_the_pages_reply() {
        let out = |ok: bool, stdout: &str, stderr: &str| Output { ok, stdout: stdout.into(), stderr: stderr.into() };
        let installed = update_reply(out(
            true,
            "downloading devid 0.13.0 (sha256-checked)…\ndevid setup:\n  ok  install …\n\ndone. Next: …\n",
            "",
        ))
        .unwrap();
        assert_eq!(installed["installed"], "0.13.0", "relaunch only after a new version");

        let newest = update_reply(out(true, "devid 0.12.1 is the newest version\n", "")).unwrap();
        assert_eq!(
            (newest["installed"].is_null(), newest["text"].as_str()),
            (true, Some("devid 0.12.1 is the newest version"))
        );

        let sudo = update_reply(out(
            false,
            "downloading devid 0.13.0 (sha256-checked)…\n  !!  system setup … needs your password\n",
            "devid: system setup … needs your password\ndevid: devid 0.13.0 is installed, but its system setup (.test DNS, ports 80/443, CA trust) needs your password: run `devid setup` in Terminal\n",
        ))
        .unwrap_err();
        assert!(sudo.starts_with("devid 0.13.0 is installed, but") && sudo.contains("needs your password"), "{sudo}");

        assert_eq!(
            update_reply(out(false, "", "")).unwrap_err(),
            "`devid self-update` failed",
            "no output still says what failed"
        );
    }

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
        assert!(is_read(&Request::MailList { query: String::new(), unread: false, limit: None }));
        assert!(!is_read(&Request::MailMessage { id: "a1".into() }), "opening a message marks it read");
    }

    #[test]
    fn terminal_apps_run_command_files() {
        let runs = |types: Value| runs_scripts(&types);
        assert!(runs(
            json!([{"CFBundleTypeRole": "Shell", "LSItemContentTypes": ["com.apple.terminal.shell-script"]}])
        ));
        assert!(runs(json!([{"CFBundleTypeRole": "Shell", "LSItemContentTypes": ["public.shell-script"]}])));
        assert!(runs(json!([{"CFBundleTypeRole": "Shell", "CFBundleTypeExtensions": ["command", "tool"]}])));
        assert!(
            !runs(json!([{"CFBundleTypeRole": "Editor", "LSItemContentTypes": ["public.shell-script"]}])),
            "an editor"
        );
        assert!(
            !runs(json!([
                {"CFBundleTypeRole": "Viewer", "LSItemContentTypes": ["com.apple.terminal.shell-script"]},
                {"CFBundleTypeRole": "Shell", "LSItemContentTypes": ["public.unix-executable"]}
            ])),
            "the role and the type belong to one entry"
        );
        assert!(!runs(Value::Null));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn terminals_come_from_the_app_folders_terminal_first() {
        let root = std::env::temp_dir().join(format!("devid-terminals-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (dir, app, role) in [
            ("a", "Zeta", "Shell"),
            ("a", "Terminal", "Shell"),
            ("a", "Notes", "Editor"),
            ("b", "Zeta", "Shell"),
            ("b", "alpha", "Shell"),
        ] {
            let contents = root.join(dir).join(format!("{app}.app/Contents"));
            std::fs::create_dir_all(&contents).unwrap();
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
                 <key>CFBundleDocumentTypes</key><array><dict><key>CFBundleTypeRole</key><string>{role}</string>\
                 <key>LSItemContentTypes</key><array><string>com.apple.terminal.shell-script</string></array>\
                 </dict></array></dict></plist>\n"
            );
            std::fs::write(contents.join("Info.plist"), plist).unwrap();
        }
        std::fs::create_dir_all(root.join("a/NoPlist.app")).unwrap();
        let found = find_terminals(&[root.join("a"), root.join("b"), root.join("missing")]);
        let names: Vec<&str> = found.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["Terminal", "alpha", "Zeta"]);
        assert_eq!(found[2]["path"], root.join("a/Zeta.app").display().to_string(), "the first folder wins");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_client_runs_through_its_shim() {
        let (sh, js) = client_files(Path::new("/Users/o'neil/.devid/bin/psql"), None, None);
        assert_eq!(sh, "#!/bin/sh\nexec '/Users/o'\\''neil/.devid/bin/psql'\n");
        assert_eq!(js, None);
        let (sh, _) = client_files(Path::new("/t/bin/mysql"), Some(OsStr::new("/t")), None);
        assert_eq!(sh, "#!/bin/sh\nexport DEVID_HOME='/t'\nexec '/t/bin/mysql'\n", "a test home goes along");
    }

    #[test]
    fn mongodb_logs_in_from_a_file_not_the_command_line() {
        let secret = db::password();
        let (sh, js) =
            client_files(Path::new("/h/bin/mongosh"), None, Some((Path::new("/h/run/in.js"), "root", &secret)));
        assert_eq!(sh, "#!/bin/sh\n'/h/bin/mongosh' --shell --file '/h/run/in.js'\nrm -f '/h/run/in.js'\n");
        let js = js.unwrap();
        assert!(js.starts_with("try { require('fs').unlinkSync(\"/h/run/in.js\"); }"), "{js}");
        assert!(js.contains(&format!("db.getSiblingDB('admin').auth(\"root\", \"{secret}\")")), "{js}");
    }
}
