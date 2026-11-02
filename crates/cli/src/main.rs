//! `devid`: thin client of the daemon (D-012). `devid daemon run` is the daemon itself.

use clap::{Parser, Subcommand};
use devid_daemon::{paths::Paths, protocol::Request, server};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
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
}
