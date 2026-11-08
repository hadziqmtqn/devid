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
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

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
    /// Show or change a site's app type: Laravel, Vite, Node.js… and how it is served (php, static, proxy)
    App {
        #[command(subcommand)]
        action: AppAction,
    },
    /// A site's own commands (npm run dev, queue workers…), started and stopped by Devid
    Cmd {
        #[command(subcommand)]
        action: Option<CmdAction>,
    },
}

#[derive(Subcommand)]
enum CmdAction {
    /// List a site's commands (default: the site containing the current folder) and suggestions
    List { site: Option<String> },
    /// Save a command: `devid cmd add [site] <name> -- <command…>`; the same name again replaces it
    Add {
        #[arg(num_args = 1..=2, required = true, value_name = "SITE NAME")]
        args: Vec<String>,
        #[arg(last = true, required = true, value_name = "COMMAND")]
        run: Vec<String>,
    },
    /// Forget a command (a running one is stopped first): `devid cmd rm [site] <name>`
    Rm {
        #[arg(num_args = 1..=2, required = true, value_name = "SITE NAME")]
        args: Vec<String>,
    },
    /// Start a command, or all of a site's: `devid cmd start [site] [name]`
    Start {
        #[arg(num_args = 0..=2, value_name = "SITE NAME")]
        args: Vec<String>,
    },
    /// Stop a command, or all of a site's: `devid cmd stop [site] [name]`
    Stop {
        #[arg(num_args = 0..=2, value_name = "SITE NAME")]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum AppAction {
    /// Show a site's type (default: the site containing the current folder) and how it is served
    Show { site: Option<String> },
    /// Change how a site is served; only the options you pass change
    Set {
        site: Option<String>,
        /// App type; resets mode, root and upstream to its defaults unless you pass them too
        #[arg(long, value_parser = devid_daemon::sites::PRESETS.map(|p| p.key))]
        preset: Option<String>,
        /// php: PHP files through php-fpm; static: files only; proxy: forward to a dev server
        #[arg(long, value_parser = devid_daemon::sites::Mode::NAMES)]
        mode: Option<String>,
        /// Web root inside the project, for php and static (e.g. public, dist, .)
        #[arg(long)]
        root: Option<String>,
        /// Port of the dev server a proxy site forwards to
        #[arg(long)]
        upstream: Option<u16>,
        /// Static: serve /index.html for paths with no file (single-page apps)
        #[arg(long, value_parser = ["on", "off"])]
        spa: Option<String>,
        /// Proxy: send the .test Host and Origin unchanged (for apps that build links from Host)
        #[arg(long, value_parser = ["on", "off"])]
        pass_host: Option<String>,
    },
    /// Forget your changes; the detected type applies again
    Reset { site: Option<String> },
    /// Also serve a site on http://127.0.0.1:<port>, for a tunnel or a tool. Usage: `devid app port
    /// [site] [PORT|off]`; without a port, the site keeps its own or gets the first free one from 8000
    Port {
        #[arg(num_args = 0..=2, value_name = "SITE PORT")]
        args: Vec<String>,
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
        Command::App { action } => app(action, paths),
        Command::Cmd { action } => cmd(action.unwrap_or(CmdAction::List { site: None }), paths),
    }
}

fn cmd(action: CmdAction, paths: &Paths) -> Result<(), String> {
    let send = |req: Request| request(paths, &req);
    let cwd = cwd()?;
    let sites = send(Request::Sites)?;
    let list = sites["sites"].as_array().map(Vec::as_slice).unwrap_or_default();
    let here = site_at(list, &cwd);
    // [site] <name>: one argument is a name in the current site.
    let site_and_name = |args: Vec<String>| match <[String; 2]>::try_from(args) {
        Ok([site, name]) => (Some(site), name),
        Err(mut one) => (None, one.remove(0)),
    };
    // [site] [name]: one argument is a command of the current site if it has one by that name,
    // else a site.
    let site_or_name = |args: Vec<String>| match args.as_slice() {
        [] => (None, None),
        [one] if here.is_some_and(|s| s["commands"].as_array().into_iter().flatten().any(|c| c["name"] == *one)) => {
            (None, Some(one.clone()))
        }
        [one] => (Some(one.clone()), None),
        [site, name] => (Some(site.clone()), Some(name.clone())),
        _ => unreachable!("clap allows at most two"),
    };
    let site = match action {
        CmdAction::List { site } => site,
        CmdAction::Add { args, run } => {
            let (site, name) = site_and_name(args);
            let v = send(Request::CommandAdd { site, cwd: cwd.clone(), name: name.clone(), run: run.join(" ") })?;
            println!("saved {name}; start it with `devid cmd start {name}` in the project, or from Devid");
            v["site"].as_str().map(String::from)
        }
        CmdAction::Rm { args } => {
            let (site, name) = site_and_name(args);
            let v = send(Request::CommandRemove { site, cwd: cwd.clone(), name: name.clone() })?;
            println!("removed {name}");
            v["site"].as_str().map(String::from)
        }
        CmdAction::Start { args } => {
            let (site, name) = site_or_name(args);
            let v = send(Request::CommandStart { site, cwd: cwd.clone(), name })?;
            for r in v["results"].as_array().map(Vec::as_slice).unwrap_or_default() {
                let name = r["name"].as_str().unwrap_or("?");
                match r["ok"] == true {
                    true => println!(
                        "started {name} (pid {}); log: `devid logs {} -f`",
                        r["pid"],
                        r["log"].as_str().unwrap_or("?")
                    ),
                    false => println!("{name} did not start: {}", r["error"].as_str().unwrap_or("?")),
                }
            }
            v["site"].as_str().map(String::from)
        }
        CmdAction::Stop { args } => {
            let (site, name) = site_or_name(args);
            let v = send(Request::CommandStop { site, cwd: cwd.clone(), name: name.clone() })?;
            let stopped: Vec<&str> = v["stopped"].as_array().into_iter().flatten().filter_map(|n| n.as_str()).collect();
            match (stopped.is_empty(), name) {
                (false, _) => println!("stopped {}", stopped.join(", ")),
                (true, Some(n)) => println!("{n} was not running"),
                (true, None) => println!("nothing was running"),
            }
            v["site"].as_str().map(String::from)
        }
    };
    let sites = send(Request::Sites)?;
    let list = sites["sites"].as_array().map(Vec::as_slice).unwrap_or_default();
    let site = match &site {
        Some(n) => match list.iter().find(|s| s["name"] == n.as_str()) {
            Some(s) => s,
            None => return Ok(()),
        },
        None => site_at(list, &cwd).ok_or(format!("{cwd} is not inside a site; pass a site name"))?,
    };
    print_commands(site);
    Ok(())
}

/// The deepest site folder containing `cwd`, as the daemon picks it.
fn site_at<'a>(list: &'a [serde_json::Value], cwd: &str) -> Option<&'a serde_json::Value> {
    list.iter()
        .filter(|s| s["path"].as_str().is_some_and(|p| Path::new(cwd).starts_with(p)))
        .max_by_key(|s| s["path"].as_str().map_or(0, |p| Path::new(p).components().count()))
}

fn print_commands(s: &serde_json::Value) {
    let name = s["name"].as_str().unwrap_or("?");
    println!("{}", s["url"].as_str().unwrap_or("?"));
    let saved = s["commands"].as_array().map(Vec::as_slice).unwrap_or_default();
    let suggestions = s["suggestions"].as_array().map(Vec::as_slice).unwrap_or_default();
    let width = saved.iter().chain(suggestions).filter_map(|c| c["name"].as_str()).map(str::len).max().unwrap_or(0);
    if saved.is_empty() {
        println!("  no commands; add one with `devid cmd add {name} <name> -- <command>`");
    }
    for c in saved {
        let state = match c["state"].as_str() {
            Some("running") => format!("running (pid {})", c["pid"]),
            Some("exited") if c["code"].is_number() => format!("exited (code {})", c["code"]),
            Some("exited") => format!("exited (signal {})", c["signal"]),
            _ => "stopped".into(),
        };
        println!("  {:<width$}  {:<20} {}", c["name"].as_str().unwrap_or("?"), state, c["run"].as_str().unwrap_or("?"));
    }
    if let Some(c) = saved.first() {
        println!("  logs: `devid logs {} -f` (one per command)", c["log"].as_str().unwrap_or("?"));
    }
    if !suggestions.is_empty() {
        println!("  suggestions (`devid cmd add {name} <name> -- <command>`):");
        for c in suggestions {
            println!("    {:<width$}  {}", c["name"].as_str().unwrap_or("?"), c["run"].as_str().unwrap_or("?"));
        }
    }
}

fn app(action: AppAction, paths: &Paths) -> Result<(), String> {
    let send = |req: Request| request(paths, &req);
    let on = |v: Option<String>| v.map(|v| v == "on");
    let name = match action {
        AppAction::Show { site } => site,
        AppAction::Set { site, preset, mode, root, upstream, spa, pass_host } => {
            let req = Request::AppSet {
                site,
                cwd: cwd()?,
                preset,
                mode,
                root,
                upstream,
                spa: on(spa),
                pass_host: on(pass_host),
            };
            Some(send(req)?["site"].as_str().unwrap_or("?").to_string())
        }
        AppAction::Reset { site } => {
            let v = send(Request::AppReset { site, cwd: cwd()? })?;
            Some(v["site"].as_str().unwrap_or("?").to_string())
        }
        AppAction::Port { args } => {
            // One argument is a port when it looks like one ("8000", "off"), else a site name.
            let port = |a: &str| match a {
                "off" => Ok(Some(0)),
                _ => a.parse::<u16>().map(Some).map_err(|_| format!("{a:?} is not a port (1024–65535) or off")),
            };
            let (site, port) = match args.as_slice() {
                [] => (None, None),
                [one] if one == "off" || one.parse::<u16>().is_ok() => (None, port(one)?),
                [one] => (Some(one.clone()), None),
                [site, p] => (Some(site.clone()), port(p)?),
                _ => unreachable!("clap allows at most two"),
            };
            let v = send(Request::AppPort { site, cwd: cwd()?, port })?;
            Some(v["site"].as_str().unwrap_or("?").to_string())
        }
    };
    let cwd = cwd()?;
    let reply = send(Request::Sites)?;
    let list = reply["sites"].as_array().map(Vec::as_slice).unwrap_or_default();
    let site = match &name {
        Some(n) => list.iter().find(|s| s["name"] == n.as_str()).ok_or(format!("no site named {n}"))?,
        None => site_at(list, &cwd).ok_or(format!("{cwd} is not inside a site; pass a site name"))?,
    };
    print_app(site);
    Ok(())
}

fn print_app(s: &serde_json::Value) {
    let text = |k: &str| s[k].as_str().unwrap_or("?").to_string();
    let detected = s["detected"].as_str().and_then(devid_daemon::sites::find_preset).map(|p| p.label);
    let typ = match (s["app"]["preset"].is_string(), detected) {
        (true, Some(d)) => format!("{} (set by you; detected: {d})", text("preset_label")),
        (true, None) => format!("{} (set by you; nothing detected)", text("preset_label")),
        (false, Some(_)) => format!("{} (detected)", text("preset_label")),
        (false, None) => "PHP (nothing detected; served as before)".into(),
    };
    println!("{}", text("url"));
    println!("  type:     {typ}");
    println!("  serve as: {}", serving(s));
    if let Some(port) = s["port"].as_u64() {
        match s["port_busy"].as_str() {
            Some(who) => println!(
                "  port:     {port} is in use by {who}; Devid leaves it out until it is free, then `devid app port` takes it"
            ),
            None => println!("  port:     http://127.0.0.1:{port}"),
        }
    }
    // The port and the commands are not part of the type; `app reset` keeps them.
    let changed: Vec<&str> = s["app"]
        .as_object()
        .map(|o| o.keys().map(String::as_str).filter(|k| !matches!(*k, "port" | "commands")).collect())
        .unwrap_or_default();
    if !changed.is_empty() {
        println!("  changed:  {} (`devid app reset` goes back to the detected type)", changed.join(", "));
    }
}

/// How a site is served, in a few words: "PHP 8.4", "static dist/", "proxy to localhost:3000".
fn serving(s: &serde_json::Value) -> String {
    let path = s["path"].as_str().unwrap_or("");
    let docroot = s["docroot"].as_str().unwrap_or("");
    let root = match docroot.strip_prefix(path).map(|r| r.trim_start_matches('/')) {
        Some("") | None => String::new(),
        Some(r) => format!(" {r}/"),
    };
    match s["mode"].as_str() {
        Some("static") => format!("static{root}{}", if s["spa"] == true { " (SPA)" } else { "" }),
        Some("proxy") => format!(
            "proxy to localhost:{}{}",
            s["upstream"],
            if s["pass_host"] == true { " (Host passed through)" } else { "" }
        ),
        _ => format!("PHP {}{root}", s["php"].as_str().unwrap_or("?")),
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
    if v["commands_running"].as_u64().is_some_and(|n| n > 0) {
        println!("  commands:    {} running (`devid cmd` in a project lists them)", v["commands_running"]);
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
        let note = match (s["mode"].as_str(), s["isolated"] == true, s["php_installed"] == true) {
            (Some("static" | "proxy"), ..) => serving(s),
            (_, _, false) => format!("PHP {php} (not installed!)"),
            (_, true, _) => format!("PHP {php} (isolated)"),
            (_, false, _) => format!("PHP {php}"),
        };
        let note = match (s["port"].as_u64(), s["port_busy"].is_string()) {
            (Some(p), false) => format!("{note}, :{p}"),
            (Some(p), true) => format!("{note}, :{p} busy"),
            (None, _) => note,
        };
        let typ = s["preset_label"].as_str().unwrap_or("PHP");
        println!(
            "{:<40} {:<14} {:<26} {}",
            s["url"].as_str().unwrap_or("?"),
            typ,
            note,
            s["path"].as_str().unwrap_or("?")
        );
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
