//! The daemon process: a blocking unix socket server, one thread per connection.

use crate::{
    ca,
    config::Config,
    dns, helper, logs, nginx,
    paths::Paths,
    php,
    protocol::Request,
    releases,
    sites::{self, Site},
    supervisor::{ProcState, Spec, Supervisor},
    webserver,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct State {
    /// "listening on …" or the bind error, shown by `devid status`.
    dns: String,
    paths: Paths,
    config: Mutex<Config>,
    supervisor: Supervisor,
    started: Instant,
    stopping: AtomicBool,
    /// Services the owner stopped on purpose; `apply` leaves them stopped. Not persisted: a
    /// daemon restart (e.g. login) brings everything back.
    paused: Mutex<BTreeSet<String>>,
}

fn php_service(version: &str) -> String {
    format!("php-{version}")
}

impl State {
    /// Every service Devid would run now: php-fpm for the default and site versions (if installed)
    /// and the active web server (if installed). Stop/start accept only these names.
    fn services(&self) -> BTreeSet<String> {
        let config = self.config.lock().unwrap();
        let installed = php::installed(&self.paths);
        let mut out: BTreeSet<String> = sites::resolve(&config)
            .iter()
            .map(|s| s.php.clone())
            .chain([config.default_php.clone()])
            .filter(|v| installed.contains(v))
            .map(|v| php_service(&v))
            .collect();
        if webserver::installed(&self.paths, &config.server) {
            out.insert(config.server.clone());
        }
        out
    }

    /// Bring every service in line with `config`: php-fpm for the default version and every
    /// version a site uses (if installed), and nginx with a freshly rendered config.
    /// Fails without changing anything if nginx rejects the new config.
    fn apply(&self, config: &Config) -> Result<(), String> {
        let sites = sites::resolve(config);
        let installed = php::installed(&self.paths);
        let versions: BTreeSet<&str> =
            sites.iter().map(|s| s.php.as_str()).chain([config.default_php.as_str()]).collect();
        let mut wanted: BTreeMap<String, Spec> = versions
            .into_iter()
            .filter(|v| installed.iter().any(|i| i == v))
            .map(|v| (php_service(v), php::spec(&self.paths, v)))
            .collect();

        // Every site gets a certificate from the Devid CA (issued once, renewed before expiry).
        for site in &sites {
            ca::ensure_site_cert(&self.paths, &site.host).map_err(|e| format!("certificate for {}: {e}", site.host))?;
        }

        // Only the active server is wanted, so switching servers stops the old one (D-005).
        let server = config.server.as_str();
        let mut reload = None;
        if webserver::installed(&self.paths, server) {
            let changed = webserver::write_config(&self.paths, server, config, &sites).map_err(|e| e.to_string())?;
            let running = matches!(self.supervisor.state().get(server), Some(ProcState::Running(_)));
            // Reload only a server that is already up; a fresh one reads the new config anyway.
            if changed && running {
                reload = Some(webserver::reload(server));
            }
            wanted.insert(server.into(), webserver::spec(&self.paths, server));
        }
        {
            let paused = self.paused.lock().unwrap();
            wanted.retain(|name, _| !paused.contains(name));
        }
        self.supervisor.reconcile(&wanted);
        // A just-started php-fpm needs a moment to create its socket; reloading nginx or replying
        // before that gives the first request a 502.
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        for name in wanted.keys().filter_map(|n| n.strip_prefix("php-")) {
            let sock = php::socket(&self.paths, name);
            while !sock.exists() && Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        match reload {
            Some(webserver::Reload::Signal(sig)) => {
                self.supervisor.signal(server, sig);
            }
            Some(webserver::Reload::Restart) => self.supervisor.restart(server),
            None => {}
        }
        Ok(())
    }

    /// Change the config, apply it, and save it only if applying worked.
    fn mutate(&self, change: impl FnOnce(&mut Config, &[Site]) -> Result<Value, String>) -> Result<Value, String> {
        let mut current = self.config.lock().unwrap();
        let mut next = current.clone();
        let reply = change(&mut next, &sites::resolve(&current))?;
        if let Err(e) = self.apply(&next) {
            // Put services back in line with the config that stays in force.
            let _ = self.apply(&current);
            return Err(e);
        }
        next.save(&self.paths.config()).map_err(|e| e.to_string())?;
        *current = next;
        Ok(reply)
    }
}

/// SIGTERM (launchd stopping the agent), SIGINT (Ctrl+C) and SIGHUP all mean "shut down cleanly".
const STOP_SIGNALS: [libc::c_int; 3] = [libc::SIGTERM, libc::SIGINT, libc::SIGHUP];

fn stop_signal_set() -> libc::sigset_t {
    // SAFETY: sigemptyset/sigaddset only write to the set we own.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for sig in STOP_SIGNALS {
            libc::sigaddset(&mut set, sig);
        }
        set
    }
}

/// Block the stop signals in this thread before any other thread exists, so every thread inherits
/// the mask and only the dedicated `sigwait` thread receives them. Child processes would inherit
/// it too; the supervisor clears it before exec (supervisor::start).
fn block_stop_signals() {
    let set = stop_signal_set();
    // SAFETY: valid set; a null old-set pointer is allowed.
    unsafe {
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}

/// Wait for a stop signal, then stop the daemon the same way a `stop` request does.
fn spawn_signal_thread(state: Arc<State>) {
    std::thread::spawn(move || {
        let set = stop_signal_set();
        let mut sig: libc::c_int = 0;
        // SAFETY: valid set and out pointer.
        if unsafe { libc::sigwait(&set, &mut sig) } == 0 {
            eprintln!("devid: received signal {sig}; stopping");
            state.stopping.store(true, Ordering::SeqCst);
            let _ = UnixStream::connect(state.paths.socket());
        }
    });
}

/// Run the daemon until a `stop` request or a stop signal arrives.
pub fn run(paths: Paths) -> io::Result<()> {
    block_stop_signals();
    let config = Config::load_or_create(&paths.config())?;
    fs::create_dir_all(paths.run_dir())?;
    let listener = bind(&paths)?;
    if ca::ensure_ca(&paths, &config.tld)? {
        eprintln!("devid: created the Devid CA at {}", ca::ca_cert(&paths).display());
    }
    ca::write_cacert(&paths)?;

    let dns = match dns::spawn(config.dns_port, config.tld.clone()) {
        Ok(()) => format!("listening on 127.0.0.1:{}", config.dns_port),
        Err(e) => {
            eprintln!("devid: DNS on 127.0.0.1:{} failed: {e}", config.dns_port);
            format!("failed: {e}")
        }
    };
    let state = Arc::new(State {
        dns,
        supervisor: Supervisor::new(),
        paths,
        config: Mutex::new(config.clone()),
        started: Instant::now(),
        stopping: AtomicBool::new(false),
        paused: Mutex::new(BTreeSet::new()),
    });
    spawn_signal_thread(Arc::clone(&state));
    if let Err(e) = state.apply(&config) {
        // Keep the daemon reachable so `devid status` can show the problem.
        eprintln!("devid: {e}");
    }

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
    state.supervisor.stop_all();
    let _ = fs::remove_file(state.paths.socket());
    Ok(())
}

/// Bind the socket, refusing to start twice and cleaning up a stale socket file.
fn bind(paths: &Paths) -> io::Result<UnixListener> {
    let sock = paths.socket();
    // macOS limits unix socket paths to 104 bytes (sun_path); say so instead of "SUN_LEN".
    if sock.as_os_str().len() >= 104 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("socket path {} is too long for a unix socket; use a shorter DEVID_HOME", sock.display()),
        ));
    }
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
        if state.stopping.load(Ordering::SeqCst) {
            // Reply sent; now wake the accept loop so it sees the flag and shuts down.
            let _ = UnixStream::connect(state.paths.socket());
            break;
        }
    }
    Ok(())
}

fn existing_dir(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("{} is not a folder", path.display()));
    }
    Ok(path)
}

/// The site named `name`, or else the one containing `cwd`.
fn pick_site(sites: &[Site], name: Option<String>, cwd: &str) -> Result<String, String> {
    match name {
        Some(name) => {
            sites.iter().find(|s| s.name == name).map(|s| s.name.clone()).ok_or(format!("no site named {name}"))
        }
        None => sites::find_by_dir(sites, Path::new(cwd))
            .map(|s| s.name.clone())
            .ok_or(format!("{cwd} is not inside a site; pass --site <name>")),
    }
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
                "server_status": service_status(state, &config.server, webserver::installed(&state.paths, &config.server)),
                "http_port": config.http_port,
                "https_port": config.https_port,
                "dns_port": config.dns_port,
                "dns": state.dns,
                "tld": config.tld,
                "standard_ports": standard_ports(&config, &state.paths),
                "ca": ca::ca_cert(&state.paths).display().to_string(),
                "sites": sites::resolve(&config).len(),
                "default_php": config.default_php,
                "paused": state.paused.lock().unwrap().iter().collect::<Vec<_>>(),
                "php": php_list(state),
            }))
        }
        Request::Stop => {
            // handle() wakes the accept loop only after this reply is written; waking it here let
            // the daemon exit before replying, so `devid daemon stop` sometimes reported failure.
            state.stopping.store(true, Ordering::SeqCst);
            Ok(json!({"ok": true}))
        }
        Request::PhpList => Ok(json!({"ok": true, "php": php_list(state)})),
        Request::PhpInstall { version, from } => {
            let installed = install_php(state, &version, from, None)?;
            Ok(json!({"ok": true, "installed": version, "version": installed}))
        }
        Request::PhpExt { version, ext, enabled } => {
            php::check_supported(&version).map_err(|e| e.to_string())?;
            if !php::installed(&state.paths).contains(&version) {
                return Err(format!("PHP {version} is not installed"));
            }
            php::set_extension(&state.paths, &version, &ext, enabled).map_err(|e| e.to_string())?;
            state.supervisor.restart(&php_service(&version));
            Ok(json!({"ok": true, "version": version, "ext": ext, "enabled": enabled}))
        }
        Request::Restart { service } => {
            if !state.supervisor.state().contains_key(&service) {
                let running: Vec<String> = state.supervisor.state().into_keys().collect();
                return Err(format!("{service} is not running (running: {})", running.join(", ")));
            }
            state.supervisor.restart(&service);
            Ok(json!({"ok": true, "restarted": service}))
        }
        Request::ServiceStop { service } | Request::ServiceStart { service }
            if !state.services().contains(&service) =>
        {
            Err(format!(
                "unknown service {service:?}; services: {}",
                state.services().into_iter().collect::<Vec<_>>().join(", ")
            ))
        }
        Request::ServiceStop { service } => {
            state.paused.lock().unwrap().insert(service.clone());
            let config = state.config.lock().unwrap();
            state.apply(&config)?;
            Ok(json!({"ok": true, "stopped": service}))
        }
        Request::ServiceStart { service } => {
            state.paused.lock().unwrap().remove(&service);
            let config = state.config.lock().unwrap();
            state.apply(&config)?;
            Ok(json!({"ok": true, "started": service}))
        }
        Request::StopAll => {
            *state.paused.lock().unwrap() = state.services();
            let config = state.config.lock().unwrap();
            state.apply(&config)?;
            Ok(json!({"ok": true}))
        }
        Request::StartAll => {
            state.paused.lock().unwrap().clear();
            let config = state.config.lock().unwrap();
            state.apply(&config)?;
            Ok(json!({"ok": true}))
        }
        Request::Logs => Ok(json!({"ok": true, "logs": logs::list(&state.paths)})),
        Request::LogRead { name, offset } => {
            let (text, offset) = logs::read(&state.paths, &name, offset).map_err(|e| e.to_string())?;
            Ok(json!({"ok": true, "text": text, "offset": offset}))
        }
        Request::LogClear { name } => {
            logs::clear(&state.paths, &name).map_err(|e| e.to_string())?;
            Ok(json!({"ok": true}))
        }
        Request::PhpUse { version } => {
            php::check_supported(&version).map_err(|e| e.to_string())?;
            if !php::installed(&state.paths).contains(&version) {
                return Err(format!("PHP {version} is not installed; run `devid php install {version}`"));
            }
            state.mutate(|c, _| {
                c.default_php = version.clone();
                Ok(json!({"ok": true, "default_php": version}))
            })
        }
        Request::ServerInstall { server, from } => {
            webserver::check(&server)?;
            install_server(state, &server, from, None)?;
            Ok(json!({"ok": true, "installed": server}))
        }
        Request::Update { check } => {
            let manifest = releases::fetch_manifest(&state.paths).map_err(|e| e.to_string())?;
            let mut changes = Vec::new();
            // Installed but the version could not be read (e.g. the probe timed out): never treat that
            // as "older", which would offer and run a needless reinstall.
            let mut unknown = Vec::new();
            for version in php::installed(&state.paths) {
                let Some(a) = releases::artifact(&manifest, "php", Some(&version)) else { continue };
                let Some(current) = php::full_version(&state.paths, &version) else {
                    unknown.push(format!("php {version}"));
                    continue;
                };
                if releases::newer(&a.version, &current) {
                    changes.push(json!({"what": format!("php {version}"), "from": current, "to": a.version}));
                    if !check {
                        install_php(state, &version, None, Some(&manifest))?;
                    }
                }
            }
            for server in ["nginx", "caddy"] {
                if !webserver::installed(&state.paths, server) {
                    continue;
                }
                let Some(a) = releases::artifact(&manifest, server, None) else { continue };
                let Some(current) = webserver::installed_version(&state.paths, server) else {
                    unknown.push(server.to_string());
                    continue;
                };
                if releases::newer(&a.version, &current) {
                    changes.push(json!({"what": server, "from": current, "to": a.version}));
                    if !check {
                        install_server(state, server, None, Some(&manifest))?;
                    }
                }
            }
            Ok(
                json!({"ok": true, "changes": changes, "unknown": unknown, "applied": !check, "release": manifest["tag"]}),
            )
        }
        Request::ServerUse { server } => {
            webserver::check(&server)?;
            if !webserver::installed(&state.paths, &server) {
                return Err(format!("{server} is not installed; run `devid server install {server}`"));
            }
            state.mutate(|c, _| {
                c.server = server.clone();
                Ok(json!({"ok": true, "server": server}))
            })
        }
        Request::ServerList => {
            let config = state.config.lock().unwrap().clone();
            let list: Vec<Value> = webserver::SERVERS
                .iter()
                .map(|s| {
                    let installed = webserver::installed(&state.paths, s);
                    let config_file = match *s {
                        "nginx" => nginx::conf(&state.paths),
                        "caddy" => crate::caddy::conf(&state.paths),
                        _ => crate::apache::conf(&state.paths),
                    };
                    json!({"server": s, "installed": installed, "active": *s == config.server,
                           "status": if *s == config.server { service_status(state, s, installed) } else if installed { "stopped" } else { "not installed" },
                           "paused": state.paused.lock().unwrap().contains(*s),
                           "version": webserver::installed_version(&state.paths, s),
                           "config": config_file.display().to_string()})
                })
                .collect();
            Ok(json!({"ok": true, "servers": list}))
        }
        Request::Sites => {
            let config = state.config.lock().unwrap().clone();
            let installed = php::installed(&state.paths);
            let list: Vec<Value> = sites::resolve(&config)
                .into_iter()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "url": if standard_ports(&config, &state.paths) {
                            format!("https://{}", s.host)
                        } else {
                            format!("https://{}:{}", s.host, config.https_port)
                        },
                        "http_url": format!("http://{}:{}", s.host, config.http_port),
                        "path": s.path.display().to_string(),
                        "docroot": s.docroot.display().to_string(),
                        "php": s.php,
                        "php_installed": installed.contains(&s.php),
                        "isolated": s.isolated,
                        "linked": s.linked,
                    })
                })
                .collect();
            Ok(json!({"ok": true, "sites": list, "parked": config.parked}))
        }
        Request::Link { name, path } => {
            let path = existing_dir(&path)?;
            let folder = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let name = match name {
                Some(n) if sites::valid_name(&n) => n,
                Some(n) => return Err(format!("{n:?} is not a valid site name (a-z, 0-9, -)")),
                None => sites::name_from_folder(&folder)
                    .ok_or(format!("cannot derive a site name from {folder:?}; pass a name"))?,
            };
            state.mutate(|c, _| {
                c.links.insert(name.clone(), path.display().to_string());
                Ok(json!({"ok": true, "site": name}))
            })
        }
        Request::Unlink { name } => state.mutate(|c, _| {
            c.links.remove(&name).ok_or(format!("{name} is not a linked site"))?;
            Ok(json!({"ok": true}))
        }),
        Request::Park { path } => {
            let path = existing_dir(&path)?.display().to_string();
            state.mutate(|c, _| {
                if !c.parked.contains(&path) {
                    c.parked.push(path.clone());
                }
                Ok(json!({"ok": true, "parked": path}))
            })
        }
        Request::Unpark { path } => state.mutate(|c, _| {
            let before = c.parked.len();
            c.parked.retain(|p| *p != path);
            if c.parked.len() == before {
                return Err(format!("{path} is not parked"));
            }
            Ok(json!({"ok": true}))
        }),
        Request::Isolate { version, site, cwd } => {
            php::check_supported(&version).map_err(|e| e.to_string())?;
            if !php::installed(&state.paths).contains(&version) {
                return Err(format!("PHP {version} is not installed; run `devid php install {version}`"));
            }
            state.mutate(|c, sites| {
                let name = pick_site(sites, site, &cwd)?;
                c.isolated.insert(name.clone(), version.clone());
                Ok(json!({"ok": true, "site": name, "php": version}))
            })
        }
        Request::Unisolate { site, cwd } => state.mutate(|c, sites| {
            let name = pick_site(sites, site, &cwd)?;
            c.isolated.remove(&name);
            Ok(json!({"ok": true, "site": name}))
        }),
    }
}

fn service_status(state: &State, name: &str, installed: bool) -> &'static str {
    match state.supervisor.state().get(name) {
        Some(ProcState::Running(_)) => "running",
        Some(ProcState::Starting) => "starting",
        Some(ProcState::Failed) => "failed",
        None if installed => "stopped",
        None => "not installed",
    }
}

fn php_list(state: &State) -> Value {
    let default = state.config.lock().unwrap().default_php.clone();
    let procs = state.supervisor.state();
    let versions: Vec<Value> = php::installed(&state.paths)
        .into_iter()
        .map(|v| {
            let (status, pid) = match procs.get(&php_service(&v)) {
                Some(ProcState::Running(pid)) => ("running", Some(*pid)),
                Some(ProcState::Starting) => ("starting", None),
                Some(ProcState::Failed) => ("failed", None),
                None => ("stopped", None),
            };
            let extensions: serde_json::Map<String, Value> = php::EXTENSIONS
                .iter()
                .map(|e| (e.to_string(), json!(php::extension_enabled(&state.paths, &v, e))))
                .collect();
            json!({"version": v, "default": v == default, "status": status, "pid": pid,
                   "socket": php::socket(&state.paths, &v).display().to_string(),
                   "extensions": extensions,
                   "ini": php::etc(&state.paths, &v).join("php.ini").display().to_string()})
        })
        .collect();
    json!(versions)
}

/// True when the helper's pf redirect for these ports is installed, so URLs need no port.
fn standard_ports(config: &Config, paths: &Paths) -> bool {
    let plan = helper::Plan {
        tld: config.tld.clone(),
        dns_port: config.dns_port,
        http_port: config.http_port,
        https_port: config.https_port,
        ca_cert: ca::ca_cert(paths),
    };
    fs::read_to_string(helper::PF_FILE).is_ok_and(|c| c == helper::pf_rules(&plan))
}

/// Install PHP `version` from `from` (a build folder) or else from the release manifest
/// (downloaded, SHA-256-verified, unpacked). Restarts that version if it runs. Returns the full version.
fn install_php(state: &State, version: &str, from: Option<String>, manifest: Option<&Value>) -> Result<String, String> {
    php::check_supported(version).map_err(|e| e.to_string())?;
    let downloaded = from.is_none();
    let dir = match from {
        Some(from) => std::path::PathBuf::from(from),
        None => {
            let fetched;
            let manifest = match manifest {
                Some(m) => m,
                None => {
                    fetched = releases::fetch_manifest(&state.paths).map_err(|e| e.to_string())?;
                    &fetched
                }
            };
            let artifact = releases::artifact(manifest, "php", Some(version))
                .ok_or(format!("the release has no PHP {version} for this Mac"))?;
            releases::download(&state.paths, &artifact).map_err(|e| e.to_string())?
        }
    };
    let result = php::install(&state.paths, version, &dir).map_err(|e| e.to_string());
    if downloaded {
        // The files are copied into ~/.devid/php/<ver>; the unpacked download (~130 MB) is not needed.
        let _ = std::fs::remove_dir_all(&dir);
    }
    result?;
    // A running master restarts on the new binaries; otherwise apply starts it if wanted.
    state.supervisor.restart(&php_service(version));
    let config = state.config.lock().unwrap();
    state.apply(&config)?;
    Ok(php::full_version(&state.paths, version).unwrap_or_default())
}

/// Install a web server from `from` or else from the release manifest (Apache: the system httpd).
fn install_server(state: &State, server: &str, from: Option<String>, manifest: Option<&Value>) -> Result<(), String> {
    let downloaded = from.is_none() && server != "apache";
    let bin = match (from, server) {
        (Some(from), _) => Some(std::path::PathBuf::from(from)),
        (None, "apache") => None,
        (None, _) => {
            let fetched;
            let manifest = match manifest {
                Some(m) => m,
                None => {
                    fetched = releases::fetch_manifest(&state.paths).map_err(|e| e.to_string())?;
                    &fetched
                }
            };
            let artifact = releases::artifact(manifest, server, None)
                .ok_or(format!("the release has no {server} for this Mac"))?;
            Some(releases::download(&state.paths, &artifact).map_err(|e| e.to_string())?.join(server))
        }
    };
    let result = webserver::install(&state.paths, server, bin.as_deref()).map_err(|e| e.to_string());
    if let Some(unpacked) = bin.as_deref().filter(|_| downloaded).and_then(std::path::Path::parent) {
        let _ = std::fs::remove_dir_all(unpacked);
    }
    result?;
    // If it is the active server, restart it on the new binary; otherwise this is a no-op.
    state.supervisor.restart(server);
    let config = state.config.lock().unwrap();
    state.apply(&config)
}
