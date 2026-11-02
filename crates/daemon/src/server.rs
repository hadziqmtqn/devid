//! The daemon process: a blocking unix socket server, one thread per connection.

use crate::{
    config::Config,
    paths::Paths,
    php,
    protocol::Request,
    supervisor::{ProcState, Supervisor},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct State {
    paths: Paths,
    config: Mutex<Config>,
    supervisor: Supervisor,
    started: Instant,
    stopping: AtomicBool,
}

impl State {
    /// M2: only the default version runs. M3 adds the versions that sites use.
    fn wanted(&self) -> BTreeSet<String> {
        let default = self.config.lock().unwrap().default_php.clone();
        php::installed(&self.paths).into_iter().filter(|v| *v == default).collect()
    }

    fn reconcile(&self) {
        self.supervisor.reconcile(&self.wanted());
    }
}

/// Run the daemon until a `stop` request arrives.
pub fn run(paths: Paths) -> io::Result<()> {
    let config = Config::load_or_create(&paths.config())?;
    fs::create_dir_all(paths.run_dir())?;
    let listener = bind(&paths)?;
    php::ensure_cacert(&paths)?;

    let state = Arc::new(State {
        supervisor: Supervisor::new(paths.clone()),
        paths,
        config: Mutex::new(config),
        started: Instant::now(),
        stopping: AtomicBool::new(false),
    });
    state.reconcile();

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
    // shortcut: only a `stop` request shuts PHP down cleanly; SIGTERM handling comes with the LaunchAgent (M6).
    state.supervisor.stop_all();
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
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(req) => dispatch(req, state).unwrap_or_else(|e| json!({"ok": false, "error": e})),
            Err(e) => json!({"ok": false, "error": format!("bad request: {e}")}),
        };
        serde_json::to_writer(&mut writer, &reply)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn dispatch(req: Request, state: &State) -> Result<Value, String> {
    match req {
        Request::Status => {
            let config = state.config.lock().unwrap().clone();
            Ok(json!({
                "ok": true,
                "version": crate::VERSION,
                "pid": std::process::id(),
                "uptime_secs": state.started.elapsed().as_secs(),
                "home": state.paths.home.display().to_string(),
                "server": config.server,
                "default_php": config.default_php,
                "php": php_list(state),
            }))
        }
        Request::Stop => {
            state.stopping.store(true, Ordering::SeqCst);
            // Wake the accept loop so it sees the flag.
            let _ = UnixStream::connect(state.paths.socket());
            Ok(json!({"ok": true}))
        }
        Request::PhpList => Ok(json!({"ok": true, "php": php_list(state)})),
        Request::PhpInstall { version, from } => {
            php::install(&state.paths, &version, Path::new(&from)).map_err(|e| e.to_string())?;
            // A reinstalled version restarts on the new binaries.
            let mut without = state.wanted();
            without.remove(&version);
            state.supervisor.reconcile(&without);
            state.reconcile();
            Ok(json!({"ok": true, "installed": version}))
        }
        Request::PhpUse { version } => {
            php::check_supported(&version).map_err(|e| e.to_string())?;
            if !php::installed(&state.paths).contains(&version) {
                return Err(format!("PHP {version} is not installed; run `devid php install {version} --from <dir>`"));
            }
            {
                let mut config = state.config.lock().unwrap();
                config.default_php = version.clone();
                config.save(&state.paths.config()).map_err(|e| e.to_string())?;
            }
            state.reconcile();
            Ok(json!({"ok": true, "default_php": version}))
        }
    }
}

fn php_list(state: &State) -> Value {
    let default = state.config.lock().unwrap().default_php.clone();
    let procs = state.supervisor.state();
    let versions: Vec<Value> = php::installed(&state.paths)
        .into_iter()
        .map(|v| {
            let (status, pid) = match procs.get(&v) {
                Some(ProcState::Running(pid)) => ("running", Some(*pid)),
                Some(ProcState::Starting) => ("starting", None),
                Some(ProcState::Failed) => ("failed", None),
                None => ("stopped", None),
            };
            json!({"version": v, "default": v == default, "status": status, "pid": pid,
                   "socket": php::socket(&state.paths, &v).display().to_string()})
        })
        .collect();
    json!(versions)
}
