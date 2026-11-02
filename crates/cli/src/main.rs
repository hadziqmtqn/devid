//! `devid`: thin client of the daemon (D-012). `devid daemon run` is the daemon itself.

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
    let paths = Paths::from_env();
    let result = match Cli::parse().command {
        Command::Daemon { action: DaemonAction::Run } => server::run(paths).map_err(|e| e.to_string()),
        Command::Daemon { action: DaemonAction::Stop } => request(&paths, &Request::Stop).map(|_| println!("daemon stopped")),
        Command::Status { json } => request(&paths, &Request::Status).map(|v| print_status(&v, json)),
        Command::Php { action: PhpAction::List } => request(&paths, &Request::PhpList).map(|v| print_php(&v["php"])),
        Command::Php { action: PhpAction::Install { version, from } } => std::path::absolute(&from)
            .map_err(|e| e.to_string())
            .and_then(|from| request(&paths, &Request::PhpInstall { version: version.clone(), from: from.display().to_string() }))
            .map(|_| println!("PHP {version} installed")),
        Command::Php { action: PhpAction::Use { version } } => {
            request(&paths, &Request::PhpUse { version: version.clone() }).map(|_| println!("default PHP is now {version}"))
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("devid: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Send one request and return the daemon's JSON reply.
fn request(paths: &Paths, req: &Request) -> Result<serde_json::Value, String> {
    let sock = paths.socket();
    let mut stream = UnixStream::connect(&sock)
        .map_err(|_| format!("daemon is not running (no socket at {}); start it with `devid daemon run`", sock.display()))?;
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
    println!("devid daemon {} running (pid {}, up {}s)", v["version"].as_str().unwrap_or("?"), v["pid"], v["uptime_secs"]);
    println!("  home:        {}", v["home"].as_str().unwrap_or("?"));
    println!("  web server:  {}", v["server"].as_str().unwrap_or("?"));
    println!("  default PHP: {}", v["default_php"].as_str().unwrap_or("?"));
    print_php(&v["php"]);
}

fn print_php(list: &serde_json::Value) {
    let versions = list.as_array().map(Vec::as_slice).unwrap_or_default();
    if versions.is_empty() {
        println!("  no PHP installed; run `devid php install <version> --from <dir>`");
    }
    for p in versions {
        let pid = p["pid"].as_u64().map(|pid| format!(" (pid {pid})")).unwrap_or_default();
        let default = if p["default"] == true { "  default" } else { "" };
        println!("  PHP {}  {}{pid}{default}", p["version"].as_str().unwrap_or("?"), p["status"].as_str().unwrap_or("?"));
    }
}
