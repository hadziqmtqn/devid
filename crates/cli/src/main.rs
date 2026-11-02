//! `devid`: thin client of the daemon (D-012). `devid daemon run` is the daemon itself.

mod setup;
mod shim;

use clap::{Parser, Subcommand};
use devid_daemon::{paths::Paths, protocol::Request, server};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::ExitCode,
};

#[derive(Parser)]
#[command(name = "devid", version, about = "Local PHP development for macOS")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    /// Install Devid for this user: binary, PATH, LaunchAgent, composer, then the system setup
    Setup {
        /// Only print what would change
        #[arg(long)]
        dry_run: bool,
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
    /// Install a web server binary (nginx for now)
    Install {
        server: String,
        #[arg(long)]
        from: PathBuf,
    },
}

#[derive(Subcommand)]
enum PhpAction {
    /// List installed versions and whether their php-fpm runs
    List,
    /// Install a version from a build folder containing php, php-fpm and modules/
    Install {
        version: String,
        #[arg(long)]
        from: PathBuf,
    },
    /// Make a version the default
    Use { version: String },
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
    match run(Cli::parse().command, &Paths::from_env()) {
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
            send(Request::PhpInstall { version: version.clone(), from: abs(Some(from))? })
                .map(|_| println!("PHP {version} installed"))
        }
        Command::Php { action: PhpAction::Use { version } } => {
            send(Request::PhpUse { version: version.clone() }).map(|_| println!("default PHP is now {version}"))
        }
        Command::Server { action: ServerAction::Install { server, from } } => {
            send(Request::ServerInstall { server: server.clone(), from: abs(Some(from))? })
                .map(|_| println!("{server} installed"))
        }
        Command::Helper { action } => helper(action, paths),
        Command::Setup { dry_run } => setup::setup(paths, dry_run),
        Command::Doctor => setup::doctor(paths),
        Command::Uninstall { dry_run, purge } => setup::uninstall(paths, dry_run, purge),
        Command::Sites { json } => send(Request::Sites).map(|v| print_sites(&v["sites"], json)),
        Command::Link { name, path } => send(Request::Link { name, path: abs(path)? })
            .map(|v| println!("linked {}", v["site"].as_str().unwrap_or("?"))),
        Command::Unlink { name } => send(Request::Unlink { name: name.clone() }).map(|_| println!("unlinked {name}")),
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

/// Send one request and return the daemon's JSON reply.
fn request(paths: &Paths, req: &Request) -> Result<serde_json::Value, String> {
    let sock = paths.socket();
    let mut stream = UnixStream::connect(&sock).map_err(|_| {
        format!("daemon is not running (no socket at {}); start it with `devid daemon run`", sock.display())
    })?;
    let mut line = serde_json::to_string(req).expect("request serializes");
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&reply).map_err(|e| format!("bad reply from daemon: {e}"))?;
    if value["ok"] == false {
        return Err(value["error"].as_str().unwrap_or("daemon returned an error").to_string());
    }
    Ok(value)
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
    print_php(&v["php"]);
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

fn print_php(list: &serde_json::Value) {
    let versions = list.as_array().map(Vec::as_slice).unwrap_or_default();
    if versions.is_empty() {
        println!("  no PHP installed; run `devid php install <version> --from <dir>`");
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
