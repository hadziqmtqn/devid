//! The daemon process: a blocking unix socket server, one thread per connection.

use crate::{
    config::Config,
    paths::Paths,
    protocol::{Request, Response, Status},
};
use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct State {
    paths: Paths,
    config: Config,
    started: Instant,
    stopping: AtomicBool,
}

/// Run the daemon until a `stop` request arrives.
pub fn run(paths: Paths) -> io::Result<()> {
    let config = Config::load_or_create(&paths.config())?;
    fs::create_dir_all(paths.run_dir())?;
    let listener = bind(&paths)?;

    let state = Arc::new(State { paths, config, started: Instant::now(), stopping: AtomicBool::new(false) });
    for stream in listener.incoming() {
        if state.stopping.load(Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                std::thread::spawn(move || {
                    if let Err(e) = handle(stream, &state) {
                        eprintln!("devid: connection error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("devid: accept failed: {e}"),
        }
    }
    let _ = fs::remove_file(state.paths.socket());
    Ok(())
}

/// Bind the socket, refusing to start twice and cleaning up a stale socket file.
fn bind(paths: &Paths) -> io::Result<UnixListener> {
    let sock = paths.socket();
    if sock.exists() {
        if UnixStream::connect(&sock).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "devid daemon is already running"));
        }
        fs::remove_file(&sock)?;
    }
    let listener = UnixListener::bind(&sock)?;
    // Only the owner may talk to the daemon.
    fs::set_permissions(&sock, fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn handle(stream: UnixStream, state: &State) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(Request::Status) => Response::Status(Status {
                ok: true,
                version: crate::VERSION.into(),
                pid: std::process::id(),
                uptime_secs: state.started.elapsed().as_secs(),
                home: state.paths.home.display().to_string(),
                server: state.config.server.clone(),
                default_php: state.config.default_php.clone(),
            }),
            Ok(Request::Stop) => {
                state.stopping.store(true, Ordering::SeqCst);
                // Wake the accept loop so it sees the flag.
                let _ = UnixStream::connect(state.paths.socket());
                Response::Ok { ok: true }
            }
            Err(e) => Response::error(format!("bad request: {e}")),
        };
        serde_json::to_writer(&mut writer, &response)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}
