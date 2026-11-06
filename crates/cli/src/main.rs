//! `devid`: thin client of the daemon (D-012). `devid daemon run` is the daemon itself.

mod setup;
mod shim;
mod tui;

use clap::{Parser, Subcommand};
use devid_daemon::{
    paths::Paths,
    protocol::{Request, request},
    server,
};
use std::{io::Write, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(name = "devid", version, about = "Local PHP development for macOS")]
struct Cli {
    /// Without a command, `devid` opens the TUI (in a terminal).
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the TUI (also what `devid` with no command does in a terminal)
    Tui,
    /// Show whether the daemon runs and its settings
    Status {
        /// Print the raw JSON response
        #[arg(long)]
        json: bool,
    },
    /// Manage the daemon process
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Manage PHP versions (8.3, 8.4, 8.5)
    Php {
        #[command(subcommand)]
        action: PhpAction,
    },
    /// Install Devid for this user: binary, PATH, LaunchAgent, composer, the system setup, then PHP and a web server
    Setup {
        /// Only print what would change
        #[arg(long)]
        dry_run: bool,
        /// PHP version to install (repeatable; the first becomes the default). Default: 8.5 if none is installed
        #[arg(long, value_parser = devid_daemon::php::SUPPORTED)]
        php: Vec<String>,
        /// Web server to install and use. Default: the active one (nginx on a new install)
        #[arg(long, value_parser = devid_daemon::webserver::SERVERS)]
        server: Option<String>,
        /// Do not install PHP
        #[arg(long, conflicts_with = "php")]
        no_php: bool,
        /// Do not install a web server
        #[arg(long, conflicts_with = "server")]
        no_server: bool,
        /// Also install the Devid desktop app into ~/Applications (an installed one is always updated)
        #[arg(long)]
        gui: bool,
    },
    /// Check every part of the installation and say how to fix problems
    Doctor,
    /// Undo `devid setup` (keeps ~/.devid data unless --purge)
    Uninstall {
        #[arg(long)]
        dry_run: bool,
        /// Also delete ~/.devid (PHP, config, CA, certificates)
        #[arg(long)]
        purge: bool,
    },
    /// One-time system setup that needs your password (.test DNS, ports 80/443, CA trust)
    Helper {
        #[command(subcommand)]
        action: HelperAction,
    },
    /// Restart a service: php-8.4, nginx, caddy or apache
    Restart { service: String },
    /// Stop a service (or `all`) until `devid start`; a daemon restart brings everything back
    Stop { service: String },
    /// Start a service stopped with `devid stop` (or `all`)
    Start { service: String },
    /// List logs, or show one: `devid logs php85-fpm.log -n 50 --follow`
    Logs {
        name: Option<String>,
        /// Lines to show from the end
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
        /// Keep printing new lines (ctrl-c to stop)
        #[arg(short, long)]
        follow: bool,
        /// Empty the log
        #[arg(long)]
        clear: bool,
    },
    /// Update devid itself to the newest release
    SelfUpdate {
        /// Only show whether a newer devid exists
        #[arg(long)]
        check: bool,
    },
    /// Update installed PHP, nginx and Caddy to the newest versions in Devid's releases
    Update {
        /// Only show what would be updated
        #[arg(long)]
        check: bool,
    },
    /// Manage the web server binary
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// List sites with their URL, folder and PHP version
    Sites {
        /// Print the raw JSON response
        #[arg(long)]
        json: bool,
    },
    /// Serve a folder (default: the current one) as <name>.test
    Link {
        /// Site name (default: the folder name)
        name: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Stop serving a linked site
    Unlink { name: String },
    /// Stop serving a parked site; its folder is not touched (`unignore` serves it again)
    Ignore { site: String },
    /// Serve an ignored parked site again
    Unignore { site: String },
    /// Serve sites at login: start nginx and PHP when the daemon starts, or wait for `devid start all`
    Autostart {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// Open the Devid app (its menu-bar icon) at login
    LoginItem {
        #[arg(value_parser = ["on", "off"])]
        state: String,
        /// The app's binary (default: ~/Applications/Devid.app)
        #[arg(long)]
        app: Option<PathBuf>,
    },
    /// Serve every subfolder of a folder (default: the current one) as <subfolder>.test
    Park { path: Option<PathBuf> },
    /// Stop serving a parked folder
    Unpark { path: Option<PathBuf> },
    /// Pin a site (default: the one containing the current folder) to a PHP version
    Isolate {
        version: String,
        #[arg(long)]
        site: Option<String>,
    },
    /// Return a site to the default PHP version
    Unisolate {
        #[arg(long)]
        site: Option<String>,
    },
}

#[derive(Subcommand)]
enum HelperAction {
    /// Set up .test DNS, ports 80/443 and CA trust (asks for your password once)
    Install {
        /// Only print what would change
        #[arg(long)]
        dry_run: bool,
    },
    /// Undo everything `helper install` did
    Uninstall {
        #[arg(long)]
        dry_run: bool,
    },
    /// Show which system pieces are in place
    Status,
    /// Internal: runs as root via sudo
    #[command(hide = true)]
    Apply {
        mode: String,
        #[arg(long)]
        tld: String,
        #[arg(long)]
        dns_port: u16,
        #[arg(long)]
        http_port: u16,
        #[arg(long)]
        https_port: u16,
        #[arg(long)]
        ca: PathBuf,
    },
}

#[derive(Subcommand)]
enum ServerAction {
    /// Install a web server: nginx or caddy (from Devid's releases, or --from <binary>), or apache (/usr/sbin/httpd)
    Install {
        server: String,
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Switch the active web server (only one runs at a time)
    Use { server: String },
    /// Show the web servers and which one is active
    List,
}

#[derive(Subcommand)]
enum PhpAction {
    /// List installed versions and whether their php-fpm runs
    List,
    /// Install a version from Devid's releases (or --from a build folder with php, php-fpm, modules/)
    Install {
        version: String,
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Make a version the default
    Use { version: String },
    /// Extensions. `devid php ext 8.5 xdebug on|off` (xdebug, pcov); catalog extensions:
    /// `available <ver>`, `install <ver> <name>...`, `remove <ver> <name>`, `list <ver>`
    Ext {
        #[arg(required = true, num_args = 2.., value_name = "ARGS")]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Run the daemon in the foreground (used by the LaunchAgent)
    Run,
    /// Ask the running daemon to stop
    Stop,
}

fn main() -> ExitCode {
    if let Some(code) = shim::dispatch() {
        return code;
    }
    let paths = Paths::from_env();
    let command = match Cli::parse().command {
        Some(command) => command,
        // Scripts and pipes get the usual help instead of a full-screen UI.
        None if std::io::IsTerminal::is_terminal(&std::io::stdout()) => Command::Tui,
        None => {
            let _ = <Cli as clap::CommandFactory>::command().print_help();
            return ExitCode::SUCCESS;
        }
    };
    match run(command, &paths) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("devid: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Absolute form of `path` (default: the current folder); the daemon has a different cwd.
fn abs(path: Option<PathBuf>) -> Result<String, String> {
    let path = path.unwrap_or_else(|| PathBuf::from("."));
    let abs = std::path::absolute(&path).map_err(|e| e.to_string())?;
    // Resolve `.`/`..` and symlinks when the folder exists, so links match the real path.
    Ok(abs.canonicalize().unwrap_or(abs).display().to_string())
}

fn cwd() -> Result<String, String> {
    abs(None)
}

fn run(command: Command, paths: &Paths) -> Result<(), String> {
    let send = |req: Request| request(paths, &req);
    match command {
        Command::Daemon { action: DaemonAction::Run } => server::run(paths.clone()).map_err(|e| e.to_string()),
        Command::Daemon { action: DaemonAction::Stop } => send(Request::Stop).map(|_| println!("daemon stopped")),
        Command::Status { json } => send(Request::Status).map(|v| print_status(&v, json)),
        Command::Php { action: PhpAction::List } => send(Request::PhpList).map(|v| print_php(&v["php"])),
        Command::Php { action: PhpAction::Install { version, from } } => {
            let from = match from {
                Some(f) => Some(abs(Some(f))?),
                None => {
                    println!("downloading PHP {version} from Devid's releases (sha256-checked)…");
                    None
                }
            };
            send(Request::PhpInstall { version: version.clone(), from }).map(|v| {
                println!("PHP {} installed", v["version"].as_str().filter(|s| !s.is_empty()).unwrap_or(&version))
            })
        }
        Command::Php { action: PhpAction::Ext { args } } => php_ext(paths, &send, &args),
        Command::Stop { service } if service == "all" => {
            send(Request::StopAll).map(|_| println!("stopped all services"))
        }
        Command::Stop { service } => {
            send(Request::ServiceStop { service: service.clone() }).map(|_| println!("stopped {service}"))
        }
        Command::Start { service } if service == "all" => {
            send(Request::StartAll).map(|_| println!("started all services"))
        }
        Command::Start { service } => {
            send(Request::ServiceStart { service: service.clone() }).map(|_| println!("started {service}"))
        }
        Command::Logs { name: None, .. } => send(Request::Logs).map(|v| {
            for l in v["logs"].as_array().map(Vec::as_slice).unwrap_or_default() {
                println!("{:<24} {:>10} bytes", l["name"].as_str().unwrap_or("?"), l["size"]);
            }
        }),
        Command::Logs { name: Some(name), clear: true, .. } => {
            send(Request::LogClear { name: name.clone() }).map(|_| println!("cleared {name}"))
        }
        Command::Logs { name: Some(name), lines, follow, .. } => {
            let first = send(Request::LogRead { name: name.clone(), offset: None })?;
            let text = first["text"].as_str().unwrap_or_default();
            let all: Vec<&str> = text.lines().collect();
            for line in &all[all.len().saturating_sub(lines)..] {
                println!("{line}");
            }
            if !follow {
                return Ok(());
            }
            let mut offset = first["offset"].as_u64();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let next = send(Request::LogRead { name: name.clone(), offset })?;
                print!("{}", next["text"].as_str().unwrap_or_default());
                std::io::Write::flush(&mut std::io::stdout()).map_err(|e| e.to_string())?;
                offset = next["offset"].as_u64();
            }
        }
        Command::Restart { service } => {
            send(Request::Restart { service: service.clone() }).map(|_| println!("restarted {service}"))
        }
        Command::Update { check } => send(Request::Update { check }).map(|v| {
            let changes = v["changes"].as_array().map(Vec::as_slice).unwrap_or_default();
            let release = v["release"].as_str().unwrap_or("?");
            let unknown = v["unknown"].as_array().map(Vec::as_slice).unwrap_or_default();
            if changes.is_empty() && unknown.is_empty() {
                println!("everything is up to date (release {release})");
            }
            for u in unknown {
                println!(
                    "skipped: {} (could not read the installed version; try again, or reinstall it)",
                    u.as_str().unwrap_or("?")
                );
            }
            for c in changes {
                let verb = if check { "available" } else { "updated" };
                println!(
                    "{verb}: {} {} -> {}",
                    c["what"].as_str().unwrap_or("?"),
                    c["from"].as_str().unwrap_or("?"),
                    c["to"].as_str().unwrap_or("?")
                );
            }
            if check && !changes.is_empty() {
                println!("run `devid update` to install");
            }
        }),
        Command::Php { action: PhpAction::Use { version } } => {
            send(Request::PhpUse { version: version.clone() }).map(|_| println!("default PHP is now {version}"))
        }
        Command::Server { action: ServerAction::Install { server, from } } => {
            let from = match from {
                Some(f) => Some(abs(Some(f))?),
                None => None,
            };
            send(Request::ServerInstall { server: server.clone(), from }).map(|_| println!("{server} installed"))
        }
        Command::Server { action: ServerAction::Use { server } } => {
            send(Request::ServerUse { server: server.clone() }).map(|_| println!("now serving with {server}"))
        }
        Command::Server { action: ServerAction::List } => send(Request::ServerList).map(|v| {
            for s in v["servers"].as_array().map(Vec::as_slice).unwrap_or_default() {
                let mark = if s["active"] == true { "*" } else { " " };
                println!("{mark} {:<7} {}", s["server"].as_str().unwrap_or("?"), s["status"].as_str().unwrap_or("?"));
            }
        }),
        Command::Helper { action } => helper(action, paths),
        Command::Setup { dry_run, php, server, no_php, no_server, gui } => {
            setup::setup(paths, dry_run, setup::Pick { php, server, no_php, no_server, gui })
        }
        Command::Doctor => setup::doctor(paths),
        Command::Tui => match tui::run(paths)? {
            // Settings actions that need the plain terminal (sudo, daemon restart) run after the TUI closed.
            Some(args) => {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let status = std::process::Command::new(exe).args(&args).status().map_err(|e| e.to_string())?;
                if status.success() { Ok(()) } else { Err(format!("devid {} failed ({status})", args.join(" "))) }
            }
            None => Ok(()),
        },
        Command::SelfUpdate { check } => setup::self_update(paths, check),
        Command::Uninstall { dry_run, purge } => setup::uninstall(paths, dry_run, purge),
        Command::Sites { json } => send(Request::Sites).map(|v| {
            print_sites(&v["sites"], json);
            let ignored: Vec<&str> =
                v["ignored"].as_array().into_iter().flatten().filter_map(|i| i["name"].as_str()).collect();
            if !json && !ignored.is_empty() {
                println!("ignored (`devid unignore <name>` serves one again): {}", ignored.join(", "));
            }
        }),
        Command::Link { name, path } => send(Request::Link { name, path: abs(path)? })
            .map(|v| println!("linked {}", v["site"].as_str().unwrap_or("?"))),
        Command::Unlink { name } => send(Request::Unlink { name: name.clone() }).map(|_| println!("unlinked {name}")),
        Command::Ignore { site } => send(Request::Ignore { site: site.clone() })
            .map(|v| println!("ignored {site}; the folder is still in {}", v["parked"].as_str().unwrap_or("?"))),
        Command::Unignore { site } => {
            send(Request::Unignore { site: site.clone() }).map(|_| println!("serving {site} again"))
        }
        Command::Autostart { state } => send(Request::Autostart { enabled: state == "on" }).map(|_| {
            if state == "on" {
                println!("sites are served at login");
            } else {
                println!("after login, nginx and PHP wait for `devid start all`");
            }
        }),
        Command::LoginItem { state, app } => {
            let app = app.map(|p| abs(Some(p))).transpose()?;
            send(Request::LoginItem { enabled: state == "on", app }).map(|v| match v["login_item"].as_str() {
                Some(app) => println!("Devid opens at login ({app})"),
                None => println!("Devid no longer opens at login"),
            })
        }
        Command::Park { path } => {
            send(Request::Park { path: abs(path)? }).map(|v| println!("parked {}", v["parked"].as_str().unwrap_or("?")))
        }
        Command::Unpark { path } => send(Request::Unpark { path: abs(path)? }).map(|_| println!("unparked")),
        Command::Isolate { version, site } => send(Request::Isolate { version, site, cwd: cwd()? }).map(|v| {
            println!("{} now uses PHP {}", v["site"].as_str().unwrap_or("?"), v["php"].as_str().unwrap_or("?"))
        }),
        Command::Unisolate { site } => send(Request::Unisolate { site, cwd: cwd()? })
            .map(|v| println!("{} uses the default PHP again", v["site"].as_str().unwrap_or("?"))),
    }
}

fn print_status(v: &serde_json::Value, json: bool) {
    if json {
        println!("{v}");
        return;
    }
    println!(
        "devid daemon {} running (pid {}, up {}s)",
        v["version"].as_str().unwrap_or("?"),
        v["pid"],
        v["uptime_secs"]
    );
    println!("  home:        {}", v["home"].as_str().unwrap_or("?"));
    println!(
        "  web server:  {} {} (http {}, https {})",
        v["server"].as_str().unwrap_or("?"),
        v["server_status"].as_str().unwrap_or("?"),
        v["http_port"],
        v["https_port"]
    );
    println!("  CA:          {}", v["ca"].as_str().unwrap_or("?"));
    println!("  sites:       {}", v["sites"]);
    println!("  default PHP: {}", v["default_php"].as_str().unwrap_or("?"));
    println!(
        "  at login:    sites {}, app {}",
        if v["autostart"] == false { "wait for `devid start all`" } else { "served" },
        if v["login_item"].is_string() { "opens" } else { "does not open" }
    );
    if let Some(line) = updates_line(&v["updates"]) {
        println!("  updates:     {line}");
    }
    print_php(&v["php"]);
}

/// The last automatic update check (D-028), for `devid status`; None before the first check.
pub fn updates_line(u: &serde_json::Value) -> Option<String> {
    let checked = u["checked_at"].as_u64()?;
    let mut found: Vec<String> = u["devid"].as_str().map(|d| format!("devid {d}")).into_iter().collect();
    found.extend(
        u["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| format!("{} {}", c["what"].as_str().unwrap_or("?"), c["to"].as_str().unwrap_or("?"))),
    );
    let ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().saturating_sub(checked))
        / 60;
    Some(match found.is_empty() {
        true => format!("none (checked {ago} min ago)"),
        false => format!("{} (checked {ago} min ago); `devid update` or `devid self-update`", found.join(", ")),
    })
}

fn print_sites(list: &serde_json::Value, json: bool) {
    if json {
        println!("{list}");
        return;
    }
    let sites = list.as_array().map(Vec::as_slice).unwrap_or_default();
    if sites.is_empty() {
        println!("no sites; run `devid park` in your projects folder or `devid link` in a project");
    }
    for s in sites {
        let php = s["php"].as_str().unwrap_or("?");
        let note = match (s["isolated"] == true, s["php_installed"] == true) {
            (_, false) => format!("PHP {php} (not installed!)"),
            (true, _) => format!("PHP {php} (isolated)"),
            (false, _) => format!("PHP {php}"),
        };
        println!("{:<40} {:<26} {}", s["url"].as_str().unwrap_or("?"), note, s["path"].as_str().unwrap_or("?"));
    }
}

/// One line for a running `PhpExtInstall` (`PhpExtProgress`), e.g. "installing 2/3 imagick… 52%".
pub fn ext_progress_text(p: &serde_json::Value) -> String {
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("?").to_string();
    let (i, n, pct) = (&p["index"], &p["total"], &p["percent"]);
    match p["phase"].as_str() {
        Some("downloading") => format!("installing {i}/{n} {}… {pct}%", s(&p["name"])),
        Some("checking") => format!("checking {i}/{n} {}… {pct}%", s(&p["name"])),
        Some("restarting") => format!("restarting php-fpm {}… {pct}%", s(&p["version"])),
        _ => format!("starting… {pct}%"),
    }
}

/// Summary of a `PhpExtInstall` reply; true when any extension failed.
pub fn ext_install_summary(reply: &serde_json::Value) -> (String, bool) {
    let results = reply["results"].as_array().map(Vec::as_slice).unwrap_or_default();
    let name = |r: &serde_json::Value| r["name"].as_str().unwrap_or("?").to_string();
    let ok: Vec<String> = results
        .iter()
        .filter(|r| r["ok"] == true)
        .map(|r| format!("{} {}", name(r), r["installed"].as_str().unwrap_or("")))
        .collect();
    let failed: Vec<String> = results
        .iter()
        .filter(|r| r["ok"] != true)
        .map(|r| format!("{} ({})", name(r), r["error"].as_str().unwrap_or("?").lines().next().unwrap_or("?")))
        .collect();
    let mut parts = Vec::new();
    if !ok.is_empty() {
        parts.push(format!("installed {}", ok.join(", ")));
    }
    if reply["restarted"] == true {
        parts.push(format!("php-fpm {} restarted", reply["version"].as_str().unwrap_or("?")));
    }
    if !failed.is_empty() {
        parts.push(format!("failed: {}", failed.join(", ")));
    }
    (parts.join(" · "), !failed.is_empty())
}

/// Send `PhpExtInstall` from another thread and report its progress every 200 ms until it replies.
pub fn ext_install_with_progress(
    paths: &Paths,
    version: &str,
    names: &[String],
    mut on_progress: impl FnMut(&str),
) -> Result<serde_json::Value, String> {
    let req = Request::PhpExtInstall { version: version.to_string(), names: names.to_vec() };
    let worker = {
        let paths = paths.clone();
        std::thread::spawn(move || request(&paths, &req))
    };
    while !worker.is_finished() {
        if let Ok(v) = request(paths, &Request::PhpExtProgress)
            && v["progress"].is_object()
        {
            on_progress(&ext_progress_text(&v["progress"]));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    worker.join().map_err(|_| "the install thread panicked".to_string())?
}

/// `devid php ext …`: xdebug/pcov on/off, or the catalog commands (Phase 6).
fn php_ext(
    paths: &Paths,
    send: &dyn Fn(Request) -> Result<serde_json::Value, String>,
    args: &[String],
) -> Result<(), String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("?").to_string();
    match a.as_slice() {
        ["available", version] => send(Request::PhpExtCatalog { version: version.to_string() }).map(|v| {
            println!("catalog for PHP {version} (release {}):", s(&v["release"]));
            let list = v["extensions"].as_array().map(Vec::as_slice).unwrap_or_default();
            if list.is_empty() {
                println!("  nothing for PHP {version} in this release");
            }
            for e in list {
                let mark = e["installed"].as_str().map_or(String::new(), |i| format!("installed {i}"));
                println!("  {:<10} {:<9} {:<12} {mark}", s(&e["name"]), s(&e["version"]), s(&e["license"]));
            }
        }),
        ["install", version, names @ ..] if !names.is_empty() => {
            use std::io::IsTerminal;
            let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
            println!("downloading {} for PHP {version} from Devid's releases (sha256-checked)…", names.join(", "));
            let tty = std::io::stdout().is_terminal();
            let reply = ext_install_with_progress(paths, version, &names, |line| {
                if tty {
                    print!("\r\x1b[K{line}");
                    let _ = std::io::stdout().flush();
                }
            });
            if tty {
                print!("\r\x1b[K");
            }
            let reply = reply?;
            let (summary, failed) = ext_install_summary(&reply);
            println!("{summary}");
            for r in reply["results"].as_array().into_iter().flatten().filter(|r| r["ok"] != true) {
                eprintln!("{}: {}", s(&r["name"]), s(&r["error"]));
            }
            if failed { Err("not every extension was installed".into()) } else { Ok(()) }
        }
        ["remove", version, name] => {
            send(Request::PhpExtRemove { version: version.to_string(), name: name.to_string() })
                .map(|_| println!("{name} removed from PHP {version} (php-fpm restarted)"))
        }
        ["list", version] => send(Request::PhpExtList { version: version.to_string() }).map(|v| {
            let names = |l: &serde_json::Value| -> Vec<String> {
                l.as_array().map(Vec::as_slice).unwrap_or_default().iter().map(s).collect()
            };
            println!("PHP {version}");
            println!("  bundled: {}", names(&v["bundled"]).join(", "));
            let shipped: Vec<String> = v["shipped"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, on)| format!("{k} {}", if on == true { "on" } else { "off" }))
                .collect();
            println!("  shipped: {}", shipped.join(", "));
            let catalog: Vec<String> = v["catalog"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|e| format!("{} {}", s(&e["name"]), s(&e["version"])))
                .collect();
            if catalog.is_empty() {
                println!("  catalog: none (see `devid php ext available {version}`)");
            } else {
                println!("  catalog: {}", catalog.join(", "));
            }
        }),
        [version, ext, state @ ("on" | "off")] => {
            send(Request::PhpExt { version: version.to_string(), ext: ext.to_string(), enabled: *state == "on" })
                .map(|_| println!("{ext} {state} for PHP {version} (php-fpm restarted)"))
        }
        _ => Err("usage: devid php ext <ver> xdebug|pcov on|off, or devid php ext available|list <ver>, \
                  or devid php ext install <ver> <name>..., or devid php ext remove <ver> <name>"
            .into()),
    }
}

fn print_php(list: &serde_json::Value) {
    let versions = list.as_array().map(Vec::as_slice).unwrap_or_default();
    if versions.is_empty() {
        println!("  no PHP installed; run `devid php install <version>`");
    }
    for p in versions {
        let pid = p["pid"].as_u64().map(|pid| format!(" (pid {pid})")).unwrap_or_default();
        let default = if p["default"] == true { "  default" } else { "" };
        println!(
            "  PHP {}  {}{pid}{default}",
            p["version"].as_str().unwrap_or("?"),
            p["status"].as_str().unwrap_or("?")
        );
    }
}

/// The helper plan from config.toml, so it works even when the daemon is not running.
fn helper_plan(paths: &Paths) -> Result<devid_daemon::helper::Plan, String> {
    let config = devid_daemon::config::Config::load_or_create(&paths.config()).map_err(|e| e.to_string())?;
    Ok(devid_daemon::helper::Plan {
        tld: config.tld,
        dns_port: config.dns_port,
        http_port: config.http_port,
        https_port: config.https_port,
        ca_cert: devid_daemon::ca::ca_cert(paths),
    })
}

fn helper(action: HelperAction, paths: &Paths) -> Result<(), String> {
    use devid_daemon::helper::{self as h, Plan};
    match action {
        HelperAction::Status => {
            for (name, ok) in h::status(&helper_plan(paths)?) {
                println!("{} {name}", if ok { "ok     " } else { "missing" });
            }
            Ok(())
        }
        HelperAction::Install { dry_run } | HelperAction::Uninstall { dry_run } => {
            let install = matches!(action, HelperAction::Install { .. });
            let plan = helper_plan(paths)?;
            if install && !plan.ca_cert.is_file() {
                return Err("no Devid CA yet; start the daemon once (`devid daemon run`) to create it".into());
            }
            let actions = if install { h::install_actions(&plan) } else { h::uninstall_actions(&plan) };
            println!("devid helper will (as root):");
            for a in &actions {
                println!("  - {}", a.describe());
            }
            if dry_run {
                println!("dry run: nothing changed");
                return Ok(());
            }
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let status = std::process::Command::new("sudo")
                .arg(exe)
                .args(["helper", "apply", if install { "install" } else { "uninstall" }])
                .args(["--tld", &plan.tld])
                .args(["--dns-port", &plan.dns_port.to_string()])
                .args(["--http-port", &plan.http_port.to_string()])
                .args(["--https-port", &plan.https_port.to_string()])
                .arg("--ca")
                .arg(&plan.ca_cert)
                .status()
                .map_err(|e| format!("could not run sudo: {e}"))?;
            if status.success() { Ok(()) } else { Err(format!("helper failed ({status})")) }
        }
        HelperAction::Apply { mode, tld, dns_port, http_port, https_port, ca } => {
            // SAFETY: geteuid has no preconditions.
            if unsafe { libc::geteuid() } != 0 {
                return Err("`helper apply` must run as root; use `devid helper install`".into());
            }
            let plan = Plan { tld, dns_port, http_port, https_port, ca_cert: ca };
            h::validate(&plan).map_err(|e| e.to_string())?;
            let actions = match mode.as_str() {
                "install" => h::install_actions(&plan),
                "uninstall" => h::uninstall_actions(&plan),
                other => return Err(format!("unknown helper mode {other:?}")),
            };
            h::apply(&actions, |step| println!("  {step}")).map_err(|e| e.to_string())?;
            println!("helper {mode} done");
            Ok(())
        }
    }
}
