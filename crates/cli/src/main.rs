//! `devid`: thin client of the daemon (D-012). `devid daemon run` is the daemon itself.

// These replace std's `println!` and `print!` in the whole crate (they are defined before the
// modules). Rust ignores SIGPIPE, so std's macros panic when stdout's reader has gone away
// (`devid sites | head -1`); these end the process quietly with success instead, see
// `stdout_failed`. SIGPIPE itself stays ignored: `devid daemon run` is the daemon, and a client
// that disconnects from its socket must not end it.
macro_rules! println {
    () => {
        $crate::out(format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::out(format_args!("{}\n", format_args!($($arg)*)))
    };
}

macro_rules! print {
    ($($arg:tt)*) => {
        $crate::out(format_args!($($arg)*))
    };
}

/// Write to stdout for `println!` and `print!`.
fn out(args: std::fmt::Arguments) {
    if let Err(e) = std::io::Write::write_fmt(&mut std::io::stdout(), args) {
        stdout_failed(e);
    }
}

/// Flush what `print!` left in stdout's buffer, like `out` on failure.
fn flush_stdout() {
    if let Err(e) = std::io::Write::flush(&mut std::io::stdout()) {
        stdout_failed(e);
    }
}

/// A broken pipe means the reader stopped reading (`head`, `grep -q`), which is not a failure of
/// devid: exit 0, so `devid cmd front | grep -q x` holds under `set -o pipefail`. Any other
/// write error (a full disk) is reported and fails.
fn stdout_failed(e: std::io::Error) -> ! {
    if e.kind() == std::io::ErrorKind::BrokenPipe {
        std::process::exit(0);
    }
    eprintln!("devid: cannot write to stdout: {e}");
    std::process::exit(1);
}

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
        /// Never ask for a password; a system setup step that needs sudo is skipped and reported
        #[arg(long)]
        no_sudo: bool,
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
        /// Never ask for a password; a system setup step that needs sudo is skipped and reported
        #[arg(long)]
        no_sudo: bool,
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
    /// Put Devid's commands on PATH for new terminals (php, php8.4, composer, nginx, caddy, mailpit…).
    /// With no state: whether they are
    #[command(name = "path")]
    ShellPath {
        #[arg(value_parser = ["on", "off"])]
        state: Option<String>,
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
    #[command(args_conflicts_with_subcommands = true)]
    Cmd {
        #[command(subcommand)]
        action: Option<CmdAction>,
        /// List this site's commands, as `devid cmd list <site>` does
        site: Option<String>,
    },
    /// Share a site on the internet through a Cloudflare quick tunnel (a temporary
    /// trycloudflare.com link; needs cloudflared). Default: the site containing the current folder
    Share {
        site: Option<String>,
        /// Stop sharing
        #[arg(long)]
        stop: bool,
    },
    /// Catch the mail your apps send, in an inbox on this Mac (Mailpit). With no action: its status
    Mail {
        #[command(subcommand)]
        action: Option<MailAction>,
    },
}

#[derive(Subcommand)]
enum MailAction {
    /// Turn Mail on (the first time, Devid downloads Mailpit)
    On,
    /// Turn Mail off; the mail it caught stays
    Off,
    /// The SMTP port apps send to (default 2525, as in Laravel's .env.example); the inbox takes the next one
    Port { port: u16 },
    /// How many messages to keep; the oldest go first (default 500)
    Keep { count: u32 },
    /// Whether PHP's mail() goes to Mail too, through sendmail_path in Devid's PHP (default on)
    PhpMail {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// Whether the Devid app posts a notification for new mail
    Notify {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// Send a test email to the inbox
    Test,
    /// Open the inbox in the browser
    Open,
    /// Delete all mail
    Clear,
    /// List the newest mail, or what matches a search (words, from:, to:, subject:, has:attachment…)
    List {
        query: Vec<String>,
        /// Only unread mail
        #[arg(long)]
        unread: bool,
        /// How many to list (default 20)
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Show one message: who sent it, to whom, its attachments and its text (marks it read)
    Show { id: String },
    /// Delete messages by ID
    Delete {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Save an attachment to ~/Downloads: `devid mail save <id> <part>` (`show` lists the parts)
    Save { id: String, part: String },
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
    /// Start a proxy site's command with its first request and stop it when the site is idle:
    /// `devid cmd on-demand [site] <name>`, or `off`
    #[command(name = "on-demand")]
    OnDemand {
        #[arg(num_args = 1..=2, required = true, value_name = "SITE NAME")]
        args: Vec<String>,
        /// Minutes without requests or open connections before it stops (default 10; 0 never)
        #[arg(long, value_name = "MINUTES")]
        idle: Option<u32>,
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
            // `setup --no-sudo` skipped a sudo step: `self-update --no-sudo` tells that apart.
            if e == setup::NEEDS_PASSWORD { ExitCode::from(setup::NEEDS_PASSWORD_EXIT) } else { ExitCode::FAILURE }
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
                flush_stdout();
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
        Command::Setup { dry_run, php, server, no_php, no_server, no_sudo, gui } => {
            setup::setup(paths, dry_run, setup::Pick { php, server, no_php, no_server, gui, no_sudo })
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
        Command::SelfUpdate { check, no_sudo } => setup::self_update(paths, check, no_sudo),
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
        Command::ShellPath { state } => shell_path(paths, state.as_deref()),
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
        Command::Cmd { action, site } => cmd(action.unwrap_or(CmdAction::List { site }), paths),
        Command::Share { site, stop: true } => send(Request::ShareStop { site, cwd: cwd()? }).map(|v| {
            let site = v["site"].as_str().unwrap_or("?");
            match v["stopped"] == true {
                true => println!("stopped sharing {site}"),
                false => println!("{site} was not shared"),
            }
        }),
        Command::Mail { action } => mail(action, paths),
        Command::Share { site, stop: false } => {
            println!("starting a Cloudflare quick tunnel…");
            let v = send(Request::ShareStart { site, cwd: cwd()? })?;
            let site = v["site"].as_str().unwrap_or("?");
            match v["url"].as_str() {
                Some(url) => {
                    println!("{url}  →  {site}");
                    println!("Anyone with the link can open the site until `devid share {site} --stop`.");
                    println!("Links the app builds from APP_URL still point at {site}'s own address.");
                }
                None => println!(
                    "cloudflared is still connecting; `devid share {site}` again shows the link (log: `devid logs {} -f`)",
                    v["log"].as_str().unwrap_or("?")
                ),
            }
            Ok(())
        }
    }
}

/// `devid path [on|off]` (D-053). Done here, not in the daemon, so it works while the daemon is down;
/// the app goes through the daemon's `shell_path`, which does the same.
fn shell_path(paths: &Paths, state: Option<&str>) -> Result<(), String> {
    use devid_daemon::install;
    let files = |list: &[PathBuf]| list.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" and ");
    let rcs = install::rc_files();
    match state {
        None => {
            let line = install::rc_line(paths);
            let on = rcs.iter().all(|rc| install::rc_has_line(rc, &line));
            println!(
                "path:      {} ({})",
                if on { "new terminals get Devid's commands" } else { "off; `devid path on` adds them" },
                files(&rcs)
            );
            println!("folder:    {}", install::bin_dir(paths).display());
            println!("commands:  {}", install::commands().join(", "));
        }
        Some(state) => {
            let on = state == "on";
            let changed = install::set_on_path(paths, on).map_err(|e| e.to_string())?;
            match (on, changed.is_empty()) {
                (true, true) => println!("Devid's commands are already on PATH ({})", files(&rcs)),
                (true, false) => {
                    println!("put {} first on PATH in {}", install::bin_dir(paths).display(), files(&changed));
                    println!("open a new terminal (or run `exec $SHELL`) to use: {}", install::commands().join(", "));
                }
                (false, true) => println!("Devid's PATH line is not in {}", files(&rcs)),
                (false, false) => {
                    println!("removed Devid's PATH line from {}", files(&changed));
                    println!("new terminals no longer find Devid's commands; ~/.devid/bin/devid still works");
                }
            }
        }
    }
    Ok(())
}

fn mail(action: Option<MailAction>, paths: &Paths) -> Result<(), String> {
    let send = |req: Request| request(paths, &req);
    let set = |enabled, port, keep, php_mail, notify| send(Request::MailSet { enabled, port, keep, php_mail, notify });
    let inbox = |v: &serde_json::Value| v["mail"]["inbox_url"].as_str().unwrap_or("?").to_string();
    match action {
        None => send(Request::Status).map(|v| print_mail(&v["mail"])),
        Some(MailAction::On) => {
            println!("turning Mail on (the first time, this downloads Mailpit, sha256-checked)…");
            let v = set(Some(true), None, None, None, None)?;
            let port = &v["mail"]["port"];
            println!("Mail is on: SMTP on 127.0.0.1:{port}, inbox {}", inbox(&v));
            println!("In a Laravel app's .env: MAIL_MAILER=smtp, MAIL_HOST=127.0.0.1, MAIL_PORT={port}");
            Ok(())
        }
        Some(MailAction::Off) => set(Some(false), None, None, None, None)
            .map(|_| println!("Mail is off; the mail it caught stays in ~/.devid/mail")),
        Some(MailAction::Port { port }) => set(None, Some(port), None, None, None).map(|v| {
            println!("Mail's SMTP port is {port}, its inbox {}; set MAIL_PORT={port} in your apps' .env", inbox(&v))
        }),
        Some(MailAction::Keep { count }) => {
            set(None, None, Some(count), None, None).map(|_| println!("Mail keeps the newest {count} messages"))
        }
        Some(MailAction::PhpMail { state }) => {
            set(None, None, None, Some(state == "on"), None).map(|_| match &*state {
                "on" => println!("PHP's mail() goes to Mail while Mail is on"),
                _ => println!("PHP's mail() no longer goes to Mail"),
            })
        }
        Some(MailAction::Notify { state }) => set(None, None, None, None, Some(state == "on")).map(|_| match &*state {
            "on" => println!("the Devid app notifies you of new mail"),
            _ => println!("no notifications for new mail"),
        }),
        Some(MailAction::Test) => send(Request::MailTest).map(|v| {
            let url = v["inbox_url"].as_str().unwrap_or("?");
            println!("sent a test email to {}; see it at {url}", v["to"].as_str().unwrap_or("?"))
        }),
        Some(MailAction::Open) => {
            let v = send(Request::Status)?;
            match v["mail"]["state"].as_str().unwrap_or("off") {
                "running" => {}
                "off" => return Err("Mail is off; `devid mail on` turns it on".into()),
                state => return Err(format!("Mail is {state}; `devid start mail` starts it")),
            }
            let url = inbox(&v);
            std::process::Command::new("/usr/bin/open").arg(&url).status().map_err(|e| e.to_string())?;
            println!("opened {url}");
            Ok(())
        }
        Some(MailAction::Clear) => send(Request::MailClear).map(|_| println!("deleted all mail")),
        Some(MailAction::List { query, unread, limit }) => {
            let v = send(Request::MailList { query: query.join(" "), unread, limit: Some(limit) })?;
            print_mail_list(&v);
            Ok(())
        }
        Some(MailAction::Show { id }) => send(Request::MailMessage { id }).map(|v| print_message(&v["message"])),
        Some(MailAction::Delete { ids }) => send(Request::MailDelete { ids })
            .map(|v| println!("deleted {}", plural(v["deleted"].as_u64().unwrap_or(0), "message"))),
        Some(MailAction::Save { id, part }) => {
            send(Request::MailSave { id, part }).map(|v| println!("saved {}", v["path"].as_str().unwrap_or("?")))
        }
    }
}

fn plural(n: u64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// "2026-11-16 21:46" from Mailpit's "2026-11-16T21:46:29.965+07:00".
fn short_time(v: &serde_json::Value) -> String {
    v.as_str().map(|t| t.chars().take(16).collect::<String>().replace('T', " ")).unwrap_or_default()
}

/// `devid mail list`: one line per message, unread ones marked with *.
fn print_mail_list(v: &serde_json::Value) {
    let shown = v["messages"].as_array().map_or(0, Vec::len) as u64;
    let count = v["count"].as_u64().unwrap_or(shown);
    let more = if count > shown { format!(", the newest {shown} here") } else { String::new() };
    println!("{}{more} · {} unread in all", plural(count, "message"), v["unread"]);
    for m in v["messages"].as_array().into_iter().flatten() {
        let to: Vec<&str> = m["to"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).collect();
        let files = match m["attachments"].as_u64().unwrap_or(0) {
            0 => String::new(),
            n => format!(" [{}]", plural(n, "file")),
        };
        println!(
            "{} {}  {}  {} → {}: {}{files}",
            if m["read"] == false { "*" } else { " " },
            short_time(&m["created"]),
            m["id"].as_str().unwrap_or("?"),
            m["from"].as_str().unwrap_or("?"),
            to.join(", "),
            m["subject"].as_str().unwrap_or(""),
        );
    }
    if shown > 0 {
        println!("`devid mail show <id>` reads one.");
    }
}

/// `devid mail show`: the header lines, the attachments, then the text.
fn print_message(m: &serde_json::Value) {
    let person = |p: &serde_json::Value| match (p["name"].as_str().filter(|n| !n.is_empty()), p["address"].as_str()) {
        (Some(name), Some(address)) => format!("{name} <{address}>"),
        (_, Some(address)) => address.to_string(),
        _ => "?".into(),
    };
    let list = |v: &serde_json::Value| v.as_array().into_iter().flatten().map(person).collect::<Vec<_>>().join(", ");
    println!("From:     {}", person(&m["from"]));
    println!("To:       {}", list(&m["to"]));
    if m["cc"].as_array().is_some_and(|c| !c.is_empty()) {
        println!("Cc:       {}", list(&m["cc"]));
    }
    println!("Date:     {}", short_time(&m["date"]));
    println!("Subject:  {}", m["subject"].as_str().unwrap_or(""));
    for a in m["attachments"].as_array().into_iter().flatten() {
        println!(
            "File:     {} {} ({}); `devid mail save {} {}`",
            a["part"].as_str().unwrap_or("?"),
            a["name"].as_str().unwrap_or("?"),
            crate::tui::human_size(a["size"].as_u64().unwrap_or(0)),
            m["id"].as_str().unwrap_or("?"),
            a["part"].as_str().unwrap_or("?"),
        );
    }
    println!();
    match m["text"].as_str().filter(|t| !t.trim().is_empty()) {
        Some(text) => println!("{}", text.trim_end()),
        None => println!("(This message has only HTML: open it in Devid's inbox, or `devid mail open`.)"),
    }
}

/// `devid mail`: what Mail does now, and how an app sends to it.
fn print_mail(m: &serde_json::Value) {
    let state = m["state"].as_str().unwrap_or("?");
    let port = &m["port"];
    if state == "off" {
        let first = if m["installed"] == true { "" } else { " (the first time, it downloads Mailpit, 11 MB)" };
        println!("mail:        off; `devid mail on` turns it on{first}");
        return;
    }
    let what = m["version"].as_str().map_or("Mailpit".to_string(), |v| format!("Mailpit {v}"));
    let pid = m["pid"].as_u64().map(|p| format!(", pid {p}")).unwrap_or_default();
    println!("mail:        {state} ({what}{pid})");
    if let Some(problem) = m["error"].as_str() {
        println!("problem:     {problem}");
    }
    println!("smtp:        127.0.0.1:{port}, any login, no TLS");
    println!("  .env:      MAIL_MAILER=smtp MAIL_HOST=127.0.0.1 MAIL_PORT={port}");
    println!("inbox:       {}", m["inbox_url"].as_str().unwrap_or("?"));
    match (m["total"].as_u64(), m["unread"].as_u64()) {
        (Some(total), Some(unread)) => {
            println!("messages:    {total}, {unread} unread (keeps the newest {})", m["keep"])
        }
        _ => println!("keeps:       the newest {} messages", m["keep"]),
    }
    println!("php mail():  {}", if m["php_mail"] == true { "goes to Mail" } else { "not caught" });
    println!("notify:      {}", if m["notify"] == true { "on" } else { "off" });
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
            if let Some(w) = v["warning"].as_str() {
                println!("warning: {w}");
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
            for n in v["sleeping"].as_array().into_iter().flatten().filter_map(|n| n.as_str()) {
                let site = v["site"].as_str().unwrap_or("?");
                println!(
                    "{n} is on demand: the next request starts it again (`devid cmd on-demand {site} off` keeps it off)"
                );
            }
            v["site"].as_str().map(String::from)
        }
        CmdAction::OnDemand { args, idle } => {
            let (site, name) = site_and_name(args);
            let off = name == "off";
            let v = send(Request::AppSet {
                site,
                cwd: cwd.clone(),
                preset: None,
                mode: None,
                root: None,
                upstream: None,
                spa: None,
                pass_host: None,
                on_demand: Some(if off { String::new() } else { name.clone() }),
                idle_stop: idle,
            })?;
            let site = v["site"].as_str().unwrap_or("?");
            match (off, v["idle_stop"].as_u64()) {
                (true, _) => println!("{site} is not on demand; its commands start and stop when you say"),
                (false, Some(0)) => println!("{name} starts with the first request to {site} and keeps running"),
                (false, minutes) => println!(
                    "{name} starts with the first request to {site} and stops after {} min without requests or open tabs",
                    minutes.unwrap_or(10)
                ),
            }
            Some(site.to_string())
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

/// D-054: a site's on-demand state, e.g. "on demand: sleeping; the next request starts it".
fn on_demand_text(od: &serde_json::Value) -> String {
    let stops = match od["idle_stop"].as_u64() {
        Some(0) => "it keeps running once started".to_string(),
        Some(m) => format!("stops after {m} min idle"),
        None => String::new(),
    };
    let idle =
        od["idle_for"].as_u64().filter(|s| *s >= 60).map(|s| format!(", idle {} min", s / 60)).unwrap_or_default();
    match od["state"].as_str() {
        Some("sleeping") => format!("on demand: sleeping; the next request starts it, and it {stops}"),
        Some("starting") => format!("on demand: starting; it {stops}"),
        Some("running") => format!("on demand: running{idle}; it {stops}"),
        Some("failed") => "on demand: it exited by itself; the next request starts it again".into(),
        _ => "on demand".into(),
    }
}

/// D-056: what to fix when a Laravel site's Vite stays on plain HTTP; None when nothing needs doing.
pub(crate) fn vite_note(vite: &serde_json::Value) -> Option<String> {
    (vite["tls"] == "app_url").then(|| {
        let now = vite["app_url"].as_str().map_or_else(|| "not set".to_string(), |u| format!("now {u}"));
        format!(
            "plain HTTP, which Safari blocks; set APP_URL={} in .env ({now}) and restart Vite",
            vite["want"].as_str().unwrap_or("https://<site>")
        )
    })
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
        let od = &s["on_demand"];
        if od["command"] == c["name"] {
            println!("  {:<width$}  {}", "", on_demand_text(od));
        }
    }
    if let Some(c) = saved.first() {
        println!("  logs: `devid logs {} -f` (one per command)", c["log"].as_str().unwrap_or("?"));
    }
    match (s["vite"]["tls"].as_str(), vite_note(&s["vite"])) {
        (_, Some(note)) => println!("  vite: {note}"),
        (Some("devid"), None) => println!("  vite: started from Devid, it serves HTTPS with this site's certificate"),
        _ => {}
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
                on_demand: None,
                idle_stop: None,
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
    if let Some(url) = s["share"]["url"].as_str() {
        println!("  shared:   {url} (`devid share {} --stop`)", s["name"].as_str().unwrap_or("?"));
    }
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
    if v["standard_ports_ok"] == false {
        println!("  ports:       80/443 do not reach it: {}", setup::REDIRECT_FIX);
    }
    println!("  CA:          {}", v["ca"].as_str().unwrap_or("?"));
    println!("  sites:       {}", v["sites"]);
    println!("  default PHP: {}", v["default_php"].as_str().unwrap_or("?"));
    println!(
        "  at login:    sites {}, app {}",
        if v["autostart"] == false { "wait for `devid start all`" } else { "served" },
        if v["login_item"].is_string() { "opens" } else { "does not open" }
    );
    match v["terminal"]["on"].as_bool() {
        Some(true) => println!("  terminal:    Devid's commands are on PATH (`devid path` lists them)"),
        Some(false) => println!("  terminal:    Devid's commands are not on PATH; `devid path on` adds them"),
        None => {} // an older daemon
    }
    if let Some(line) = updates_line(&v["updates"]) {
        println!("  updates:     {line}");
    }
    for share in v["shares"].as_array().into_iter().flatten() {
        println!(
            "  shared:      {} at {}",
            share["site"].as_str().unwrap_or("?"),
            share["url"].as_str().unwrap_or("(connecting)")
        );
    }
    let sleeping = v["sleeping"].as_array().map_or(0, Vec::len);
    if v["commands_running"].as_u64().is_some_and(|n| n > 0) || sleeping > 0 {
        let asleep = match sleeping {
            0 => String::new(),
            n => format!(", {n} on demand and sleeping"),
        };
        println!("  commands:    {} running{asleep} (`devid cmd` in a project lists them)", v["commands_running"]);
    }
    let mail = &v["mail"];
    if let Some(state) = mail["state"].as_str().filter(|s| *s != "off") {
        let unread = mail["unread"].as_u64().map(|n| format!(", {n} unread")).unwrap_or_default();
        println!("  mail:        {state} on 127.0.0.1:{}{unread} (`devid mail`)", mail["port"]);
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
        let note = match s["on_demand"]["state"].as_str() {
            Some("sleeping") => format!("{note}, on demand (sleeping)"),
            Some(_) => format!("{note}, on demand"),
            None => note,
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
