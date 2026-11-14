//! The daemon process: a blocking unix socket server, one thread per connection.

use crate::{
    ca,
    commands::{self, Commands},
    config::{App, AppCommand, Config},
    dns, helper, install, logs, nginx,
    paths::Paths,
    php, phpext, ports,
    protocol::Request,
    releases,
    sites::{self, Mode, Site},
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
    /// daemon restart (e.g. login) brings everything back, unless `autostart` is off (D-031).
    paused: Mutex<BTreeSet<String>>,
    /// The last update check (D-028): when, a newer devid, and the changes `update` would make.
    updates: Mutex<Value>,
    /// One update check or update at a time: they share the downloaded manifest.
    update_lock: Mutex<()>,
    /// The running catalog install, for `PhpExtProgress`; one at a time.
    ext_job: Mutex<Option<ExtJob>>,
    /// The last `update` that installed, for `UpdateProgress`: running, or how it ended.
    update_job: Mutex<Option<UpdateJob>>,
    /// Local ports (D-037) in the web server's last applied config: while a Devid web server runs,
    /// those are its own, not taken by another program.
    own_ports: Mutex<BTreeSet<u16>>,
    /// Sites whose local port another program holds: site name -> that program ("php (pid 38571)").
    busy_ports: Mutex<BTreeMap<String, String>>,
    /// The owner's commands (D-036) started from Devid.
    commands: Commands,
}

/// Progress of a `PhpExtInstall`. Downloads count for 0–90 % by bytes (the archive's size on disk
/// while it arrives), then each finished extension and the final restart share the last 10 %.
struct ExtJob {
    version: String,
    total: usize,
    index: usize,
    name: String,
    phase: &'static str,
    done_bytes: u64,
    total_bytes: u64,
    current: Option<(std::path::PathBuf, u64)>,
    finished: usize,
}

/// The bytes of the archive being downloaded: its size on disk while it arrives, all of it after.
fn arrived(current: &Option<(std::path::PathBuf, u64)>, downloading: bool) -> u64 {
    match current {
        Some((path, size)) if downloading => std::fs::metadata(path).map_or(0, |m| m.len()).min(*size),
        Some((_, size)) => *size,
        None => 0,
    }
}

/// Downloads count for 0–90 % by bytes, then the finished steps share the last 10 %, out of
/// `steps + 1`, so the end is never shown before the reply.
fn job_percent(done_bytes: u64, current: u64, total_bytes: u64, finished: usize, steps: usize) -> u64 {
    let bytes = (90 * (done_bytes + current)).checked_div(total_bytes).unwrap_or(0);
    (bytes + 10 * finished as u64 / (steps as u64 + 1)).min(99)
}

impl ExtJob {
    fn percent(&self) -> u64 {
        let current = arrived(&self.current, self.phase == "downloading");
        job_percent(self.done_bytes, current, self.total_bytes, self.finished, self.total)
    }

    fn json(&self) -> Value {
        json!({"version": self.version, "index": self.index, "total": self.total, "name": self.name,
               "phase": self.phase, "percent": self.percent()})
    }
}

/// Progress of an `update` that installs: what it found, in the order it installs them (`items`,
/// each with its download `size`), the one it is on and its phase, counted like `ExtJob`. It stays
/// after the update ends, as `done` or `failed` with the error, until the next one starts.
struct UpdateJob {
    items: Vec<Value>,
    index: usize,
    phase: &'static str,
    current: Option<(std::path::PathBuf, u64)>,
    done_bytes: u64,
    total_bytes: u64,
    finished: usize,
    state: &'static str,
    error: Option<String>,
}

impl UpdateJob {
    fn json(&self) -> Value {
        let downloading = self.phase == "downloading" && self.state == "running";
        let got = arrived(&self.current, downloading);
        let size = self.current.as_ref().map_or(0, |c| c.1);
        // A complete archive is being checked (sha256) and unpacked.
        let phase = if downloading && size > 0 && got >= size { "checking" } else { self.phase };
        let percent = match self.state {
            "done" => 100,
            _ => job_percent(self.done_bytes, got, self.total_bytes, self.finished, self.items.len()),
        };
        json!({"state": self.state, "error": self.error, "items": self.items, "index": self.index, "phase": phase,
               "got": got, "size": size, "done_bytes": self.done_bytes, "total_bytes": self.total_bytes,
               "percent": percent})
    }
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
            .filter(|s| s.mode == Mode::Php)
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
        let mut sites = sites::resolve(config);
        // D-037: a local port another program listens on is left out of the web server's config,
        // so it can never stop the web server and every other site with it. Ports the running server
        // already holds are its own.
        let states = self.supervisor.state();
        let serving = webserver::SERVERS.iter().any(|s| matches!(states.get(*s), Some(ProcState::Running(_))));
        let own = if serving { self.own_ports.lock().unwrap().clone() } else { BTreeSet::new() };
        let mut busy = BTreeMap::new();
        for site in &mut sites {
            if let Some(p) = site.port
                && !own.contains(&p)
                && ports::in_use(p)
            {
                busy.insert(site.name.clone(), ports::holder(p).unwrap_or_else(|| "another program".into()));
                site.port = None;
            }
        }
        let installed = php::installed(&self.paths);
        // Static and proxy sites need no php-fpm (D-035).
        let versions: BTreeSet<&str> = sites
            .iter()
            .filter(|s| s.mode == Mode::Php)
            .map(|s| s.php.as_str())
            .chain([config.default_php.as_str()])
            .collect();
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
        *self.own_ports.lock().unwrap() = sites.iter().filter_map(|s| s.port).collect();
        *self.busy_ports.lock().unwrap() = busy;
        Ok(())
    }

    /// Change the config, apply it, and save it only if applying worked.
    fn mutate(&self, change: impl FnOnce(&mut Config, &[Site]) -> Result<Value, String>) -> Result<Value, String> {
        let mut current = self.config.lock().unwrap();
        let mut next = current.clone();
        let reply = change(&mut next, &sites::resolve(&current))?;
        log_moved(sites::spread_ports(&current, &mut next, ports::in_use));
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

/// D-042: say in the daemon's log which new sites got their own dev server port.
fn log_moved(moved: Vec<(String, u16)>) {
    for (site, port) in moved {
        eprintln!("devid: {site} forwards to port {port}, because another site uses its type's default port");
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
    let mut config = Config::load_or_create(&paths.config())?;
    // D-042: projects added while the daemon was stopped.
    let moved = sites::spread_ports(&config.clone(), &mut config, ports::in_use);
    if !moved.is_empty() {
        config.save(&paths.config())?;
        log_moved(moved);
    }
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
    let commands = Commands::new(paths.clone());
    let state = Arc::new(State {
        dns,
        supervisor: Supervisor::new(),
        paths,
        config: Mutex::new(config.clone()),
        started: Instant::now(),
        stopping: AtomicBool::new(false),
        paused: Mutex::new(BTreeSet::new()),
        ext_job: Mutex::new(None),
        update_job: Mutex::new(None),
        updates: Mutex::new(Value::Null),
        update_lock: Mutex::new(()),
        own_ports: Mutex::new(BTreeSet::new()),
        busy_ports: Mutex::new(BTreeMap::new()),
        commands,
    });
    // D-036: commands a crashed daemon left running are stopped. Found before any request can
    // start a new one; stopped in the background, as that may take 10 s.
    let leftovers = state.commands.leftovers();
    if !leftovers.is_empty() {
        eprintln!("devid: stopping {} command(s) a previous daemon left running", leftovers.len());
        std::thread::spawn(move || commands::stop_groups(&leftovers));
    }
    spawn_signal_thread(Arc::clone(&state));
    spawn_update_checks(Arc::clone(&state));
    if !config.autostart {
        // D-031: nginx and php-fpm wait for `start all`; the daemon and DNS run as usual.
        *state.paused.lock().unwrap() = state.services();
    }
    if let Err(e) = state.apply(&config) {
        // Keep the daemon reachable so `devid status` can show the problem.
        eprintln!("devid: {e}");
    }
    // D-040: the commands a Devid-made restart (an update) interrupted run again; off the main
    // thread, since reading the login shell's environment can take a few seconds.
    let resuming = Arc::clone(&state);
    std::thread::spawn(move || resume_commands(&resuming));

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
    state.commands.stop_all();
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

/// A link's site name: the one given, or the folder's name made valid.
fn link_name(name: Option<String>, path: &Path) -> Result<String, String> {
    let folder = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match name {
        Some(n) if sites::valid_name(&n) => Ok(n),
        Some(n) => Err(format!("{n:?} is not a valid site name (a-z, 0-9, -)")),
        None => {
            sites::name_from_folder(&folder).ok_or(format!("cannot derive a site name from {folder:?}; pass a name"))
        }
    }
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
/// Whether `site` may use local port `port` (D-037). A program already listening there is not an
/// error: `apply` leaves the port out until it is free.
fn check_port(config: &Config, sites: &[Site], site: &str, port: u16) -> Result<(), String> {
    if port < 1024 {
        return Err(format!("{port} needs root; use a port from 1024 up"));
    }
    if [config.http_port, config.https_port, config.dns_port].contains(&port) {
        return Err(format!("{port} is one of Devid's own ports"));
    }
    if let Some(other) = sites.iter().find(|s| s.name != site && s.port == Some(port)) {
        return Err(format!("{}.{} already uses port {port}", other.name, config.tld));
    }
    if let Some(dev) = sites.iter().find(|s| s.mode == Mode::Proxy && s.upstream == port) {
        return Err(match dev.name == site {
            true => format!("{port} is this site's own dev server port; pick another"),
            false => format!("{port} is the dev server port of {}.{}", dev.name, config.tld),
        });
    }
    Ok(())
}

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
                "autostart": config.autostart,
                "login_item": install::app_agent_program(),
                "update_check_minutes": config.update_check_minutes,
                "updates": *state.updates.lock().unwrap(),
                "commands_running": state.commands.running().iter().filter(|(_, n)| n != commands::SHARE).count(),
                "running_commands": state.commands.running().into_iter()
                    .filter(|(_, n)| n != commands::SHARE)
                    .map(|(site, name)| json!({"site": site, "name": name}))
                    .collect::<Vec<_>>(),
                "shares": state.commands.running().into_iter()
                    .filter(|(_, n)| n == commands::SHARE)
                    .map(|(site, _)| json!({"site": site, "url": share_state(state, &site)["url"]}))
                    .collect::<Vec<_>>(),
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
            let installed = install_php(state, &version, from, None, &|| {})?;
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
        Request::PhpExtCatalog { version } => {
            php::check_supported(&version).map_err(|e| e.to_string())?;
            let manifest = releases::fetch_manifest(&state.paths).map_err(|e| e.to_string())?;
            let installed = phpext::installed(&state.paths, &version);
            let list: Vec<Value> = phpext::catalog(&manifest, &version)
                .into_iter()
                .map(|e| {
                    let have = installed.iter().find(|i| i.name == e.name).map(|i| i.version.clone());
                    json!({"name": e.name, "version": e.version, "license": e.license, "size": e.size, "installed": have})
                })
                .collect();
            Ok(json!({"ok": true, "version": version, "release": manifest["tag"], "extensions": list}))
        }
        Request::PhpExtInstall { version, names } => install_extensions(state, &version, &names),
        Request::PhpExtProgress => {
            let job = state.ext_job.lock().unwrap();
            Ok(json!({"ok": true, "progress": job.as_ref().map(ExtJob::json)}))
        }
        Request::PhpExtRemove { version, name } => {
            phpext::remove(&state.paths, &version, &name).map_err(|e| e.to_string())?;
            state.supervisor.restart(&php_service(&version));
            Ok(json!({"ok": true, "version": version, "name": name}))
        }
        Request::PhpExtList { version } => {
            php::check_supported(&version).map_err(|e| e.to_string())?;
            if !php::installed(&state.paths).contains(&version) {
                return Err(format!("PHP {version} is not installed"));
            }
            let bin = php::dir(&state.paths, &version).join("bin/php");
            let out = crate::webserver::probe(&bin, &["-n", "-m"]).map_err(|e| e.to_string())?;
            let bundled = phpext::module_names(&String::from_utf8_lossy(&out.stdout));
            let shipped: serde_json::Map<String, Value> = php::EXTENSIONS
                .iter()
                .map(|e| (e.to_string(), json!(php::extension_enabled(&state.paths, &version, e))))
                .collect();
            let catalog: Vec<Value> = phpext::installed(&state.paths, &version)
                .into_iter()
                .map(|i| json!({"name": i.name, "version": i.version}))
                .collect();
            Ok(json!({"ok": true, "version": version, "bundled": bundled, "shipped": shipped, "catalog": catalog}))
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
            install_server(state, &server, from, None, &|| {})?;
            Ok(json!({"ok": true, "installed": server}))
        }
        Request::Update { check } => {
            let _one = state.update_lock.lock().unwrap();
            if !check {
                // The last update's end is not this one's: it starts with no progress.
                *state.update_job.lock().unwrap() = None;
            }
            let reply = find_updates(state, check)?;
            remember_updates(state, &reply, None);
            Ok(reply)
        }
        Request::UpdateProgress => {
            let job = state.update_job.lock().unwrap();
            Ok(json!({"ok": true, "progress": job.as_ref().map(UpdateJob::json)}))
        }
        Request::Autostart { enabled } => state.mutate(|c, _| {
            c.autostart = enabled;
            Ok(json!({"ok": true, "autostart": enabled}))
        }),
        Request::LoginItem { enabled, app } => {
            let app = install::set_app_agent(enabled, app.as_deref())?;
            Ok(json!({"ok": true, "login_item": app}))
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
            let busy = state.busy_ports.lock().unwrap().clone();
            let list: Vec<Value> = sites::resolve(&config)
                .into_iter()
                .map(|s| {
                    let saved = config.apps.get(&s.name).map(|a| a.commands.as_slice()).unwrap_or_default();
                    let commands: Vec<Value> = saved
                        .iter()
                        .map(|c| {
                            let mut v = state.commands.state(&s.name, &c.name);
                            v["name"] = c.name.clone().into();
                            v["run"] = c.run.clone().into();
                            v["log"] = commands::log_name(&s.name, &c.name).into();
                            v
                        })
                        .collect();
                    let suggestions: Vec<Value> = sites::suggest_commands(&s.path, s.preset)
                        .into_iter()
                        .filter(|(name, run)| !saved.iter().any(|c| c.name == *name || c.run == *run))
                        .map(|(name, run)| json!({"name": name, "run": run}))
                        .collect();
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
                        "preset": s.preset,
                        "preset_label": sites::find_preset(s.preset).map_or(s.preset, |p| p.label),
                        "detected": s.detected,
                        "mode": s.mode,
                        "upstream": s.upstream,
                        "spa": s.spa,
                        "pass_host": s.pass_host,
                        "app": config.apps.get(&s.name),
                        "port": s.port,
                        "port_busy": busy.get(&s.name),
                        "commands": commands,
                        "suggestions": suggestions,
                        "share": share_state(state, &s.name),
                    })
                })
                .collect();
            let ignored: Vec<Value> =
                config.ignored.iter().map(|p| json!({"name": sites::ignored_name(p), "path": p})).collect();
            Ok(json!({"ok": true, "sites": list, "parked": config.parked, "ignored": ignored}))
        }
        Request::Link { name, path } => {
            let path = existing_dir(&path)?;
            let name = link_name(name, &path)?;
            state.mutate(|c, _| {
                c.links.insert(name.clone(), path.display().to_string());
                Ok(json!({"ok": true, "site": name}))
            })
        }
        Request::LinkPreview { name, path } => {
            let path = existing_dir(&path)?;
            let name = link_name(name, &path)?;
            let current = state.config.lock().unwrap().clone();
            let taken = sites::resolve(&current).into_iter().find(|s| s.name == name).map(|s| s.path);
            let mut next = current.clone();
            next.links.insert(name.clone(), path.display().to_string());
            sites::spread_ports(&current, &mut next, ports::in_use);
            let s =
                sites::resolve(&next).into_iter().find(|s| s.name == name).ok_or(format!("{name} did not resolve"))?;
            Ok(json!({
                "ok": true,
                "site": name,
                "host": s.host,
                "preset": s.preset,
                "preset_label": sites::find_preset(s.preset).map_or(s.preset, |p| p.label),
                "detected": s.detected,
                "mode": s.mode,
                "upstream": s.upstream,
                "php": s.php,
                "taken": taken,
            }))
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
        Request::Ignore { site } => state.mutate(|c, sites| {
            let s = sites.iter().find(|s| s.name == site).ok_or(format!("no site named {site}"))?;
            if s.linked {
                return Err(format!("{site} is linked; use `devid unlink {site}`"));
            }
            let path = s.path.display().to_string();
            let parked = c.parked.iter().find(|p| s.path.starts_with(p)).cloned().unwrap_or_default();
            c.ignored.push(path.clone());
            Ok(json!({"ok": true, "site": site, "path": path, "parked": parked}))
        }),
        Request::Unignore { site } => state.mutate(|c, _| {
            let before = c.ignored.len();
            c.ignored.retain(|p| sites::ignored_name(p) != site && *p != site);
            if c.ignored.len() == before {
                return Err(format!("{site} is not an ignored site"));
            }
            Ok(json!({"ok": true, "site": site}))
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
        Request::AppSet { site, cwd, preset, mode, root, upstream, spa, pass_host } => state.mutate(|c, sites| {
            let name = pick_site(sites, site, &cwd)?;
            let detected = sites.iter().find(|s| s.name == name).and_then(|s| s.detected);
            if let Some(p) = &preset {
                sites::find_preset(p).ok_or(format!(
                    "unknown preset {p:?}; choose one of {}",
                    sites::PRESETS.map(|p| p.key).join(", ")
                ))?;
            }
            if let Some(m) = &mode {
                Mode::parse(m).ok_or(format!("unknown mode {m:?}; choose one of {}", Mode::NAMES.join(", ")))?;
            }
            if let Some(r) = &root
                && !sites::safe_root(r)
            {
                return Err(format!("{r:?} must be a folder inside the project, like public or dist"));
            }
            if let Some(u) = upstream
                && (u == 0 || u == c.http_port || u == c.https_port)
            {
                return Err(format!("{u} cannot be a dev server's port (0, or Devid's own web server port)"));
            }
            let app = c.apps.entry(name.clone()).or_default();
            if let Some(p) = preset {
                // The old mode, root and upstream belonged to the previous type.
                if mode.is_none() {
                    (app.mode, app.root, app.upstream) = (None, None, None);
                }
                // Choosing what detection found is the same as following detection.
                app.preset = (Some(p.as_str()) != detected).then_some(p);
            }
            app.mode = mode.or(app.mode.take());
            app.root = root.or(app.root.take());
            app.upstream = upstream.or(app.upstream);
            app.spa = spa.or(app.spa);
            app.pass_host = pass_host.or(app.pass_host);
            if app.is_empty() {
                c.apps.remove(&name);
            }
            Ok(json!({"ok": true, "site": name, "app": c.apps.get(&name)}))
        }),
        Request::AppReset { site, cwd } => state.mutate(|c, sites| {
            let name = pick_site(sites, site, &cwd)?;
            // The type goes back to detection; the local port and the commands stay.
            if let Some(app) = c.apps.get_mut(&name) {
                *app = App { port: app.port, commands: std::mem::take(&mut app.commands), ..App::default() };
                if app.is_empty() {
                    c.apps.remove(&name);
                }
            }
            Ok(json!({"ok": true, "site": name}))
        }),
        Request::AppPort { site, cwd, port } => state.mutate(|c, sites| {
            let name = pick_site(sites, site, &cwd)?;
            let current = sites.iter().find(|s| s.name == name).and_then(|s| s.port);
            let port = match port {
                Some(0) => None,
                Some(p) => {
                    check_port(c, sites, &name, p)?;
                    Some(p)
                }
                None => match current {
                    Some(p) => Some(p),
                    None => Some(
                        ports::first_free(|p| check_port(c, sites, &name, p).is_err())
                            .ok_or("no free port from 8000 to 8999; pass one")?,
                    ),
                },
            };
            let app = c.apps.entry(name.clone()).or_default();
            app.port = port;
            if app.is_empty() {
                c.apps.remove(&name);
            }
            Ok(json!({"ok": true, "site": name, "port": port}))
        }),
        Request::CommandAdd { site, cwd, name, run } => {
            let run = run.trim().to_string();
            if !commands::valid_name(&name) {
                return Err(format!("{name:?} is not a command name (letters, digits, - _ . :; up to 40)"));
            }
            if run.is_empty() {
                return Err("the command line is empty".into());
            }
            state.mutate(|c, sites| {
                let site = pick_site(sites, site, &cwd)?;
                let app = c.apps.entry(site.clone()).or_default();
                match app.commands.iter_mut().find(|x| x.name == name) {
                    Some(saved) => saved.run = run,
                    None => app.commands.push(AppCommand { name: name.clone(), run }),
                }
                Ok(json!({"ok": true, "site": site, "name": name}))
            })
        }
        Request::CommandRemove { site, cwd, name } => {
            let (site, _, saved) = site_commands(state, site, &cwd)?;
            if !saved.iter().any(|c| c.name == name) {
                return Err(format!("{site} has no command named {name}"));
            }
            state.commands.stop(&site, Some(&name));
            state.mutate(|c, _| {
                if let Some(app) = c.apps.get_mut(&site) {
                    app.commands.retain(|x| x.name != name);
                    if app.is_empty() {
                        c.apps.remove(&site);
                    }
                }
                Ok(json!({"ok": true, "site": site}))
            })
        }
        Request::CommandStart { site, cwd, name } => {
            let (site, dir, saved) = site_commands(state, site, &cwd)?;
            let add = |n: &str| format!("add it with `devid cmd add {site} {n} -- <command>`");
            let chosen: Vec<AppCommand> = match &name {
                Some(n) => vec![
                    saved
                        .iter()
                        .find(|c| c.name == *n)
                        .cloned()
                        .ok_or(format!("{site} has no command named {n}; {}", add(n)))?,
                ],
                None if saved.is_empty() => return Err(format!("{site} has no commands; {}", add("<name>"))),
                None => saved,
            };
            let warning = port_warning(state, &site);
            let config = state.config.lock().unwrap().clone();
            let port = sites::resolve(&config).into_iter().find(|s| s.name == site).and_then(|s| s.dev_port());
            let results: Vec<Value> = chosen
                .iter()
                .map(|c| match state.commands.start(&site, &c.name, &c.run, &dir, port, None) {
                    Ok(pid) => {
                        json!({"name": c.name, "ok": true, "pid": pid, "log": commands::log_name(&site, &c.name)})
                    }
                    Err(e) => json!({"name": c.name, "ok": false, "error": e}),
                })
                .collect();
            if let [one] = results.as_slice()
                && name.is_some()
                && one["ok"] == false
            {
                return Err(one["error"].as_str().unwrap_or("could not start it").to_string());
            }
            Ok(json!({"ok": true, "site": site, "results": results, "warning": warning}))
        }
        Request::CommandStop { site, cwd, name } => {
            // A site that was ignored or unlinked meanwhile can still have commands running.
            let site = match site_commands(state, site.clone(), &cwd) {
                Ok((site, ..)) => site,
                Err(e) => match site {
                    Some(s) if state.commands.has_runs(&s) => s,
                    _ => return Err(e),
                },
            };
            let stopped = state.commands.stop(&site, name.as_deref());
            Ok(json!({"ok": true, "site": site, "stopped": stopped}))
        }
        Request::ShareStart { site, cwd } => {
            let (site, dir, _) = site_commands(state, site, &cwd)?;
            let bin = cloudflared().ok_or(
                "cloudflared is not installed. Get it from https://developers.cloudflare.com/cloudflare-one/connections/\
                 connect-networks/downloads/ (Devid uses an existing one and does not download it).",
            )?;
            let config = state.config.lock().unwrap().clone();
            if !matches!(state.supervisor.state().get(&config.server), Some(ProcState::Running(_))) {
                return Err(format!("{} is not running; start it first (`devid start all`)", config.server));
            }
            // Plain HTTP to the web server with the site's Host; cloudflared serves it over HTTPS.
            let local = sites::resolve(&config).into_iter().find(|s| s.name == site).and_then(|s| s.port);
            let local = local.filter(|_| !state.busy_ports.lock().unwrap().contains_key(&site));
            let run = format!(
                "exec '{}' tunnel --no-autoupdate --url http://127.0.0.1:{} --http-host-header {site}.{}",
                bin.display(),
                share_origin(local, standard_ports(&config, &state.paths), config.http_port),
                config.tld
            );
            state.commands.start(&site, commands::SHARE, &run, &dir, None, None)?;
            // The URL shows in cloudflared's log a few seconds after it starts.
            let deadline = Instant::now() + std::time::Duration::from_secs(30);
            loop {
                let share = share_state(state, &site);
                match share["state"].as_str() {
                    Some("running") => {
                        return Ok(json!({"ok": true, "site": site, "url": share["url"], "log": share["log"]}));
                    }
                    Some("exited") => {
                        let log =
                            fs::read_to_string(state.paths.log_dir().join(commands::log_name(&site, commands::SHARE)))
                                .unwrap_or_default();
                        let tail: Vec<&str> = log.lines().rev().take(3).collect();
                        let tail: Vec<&str> = tail.into_iter().rev().collect();
                        return Err(format!("cloudflared stopped: {}", tail.join(" | ")));
                    }
                    _ if Instant::now() > deadline => {
                        return Ok(json!({"ok": true, "site": site, "url": null, "log": share["log"]}));
                    }
                    _ => std::thread::sleep(std::time::Duration::from_millis(250)),
                }
            }
        }
        Request::RememberRunning => {
            let running: Vec<Value> = state
                .commands
                .running()
                .into_iter()
                .filter(|(_, n)| n != commands::SHARE)
                .map(|(site, name)| json!({"site": site, "name": name}))
                .collect();
            let note = json!({"at": now_secs(), "commands": running});
            let path = resume_file(&state.paths);
            let tmp = path.with_extension("json.tmp");
            fs::write(&tmp, note.to_string()).and_then(|_| fs::rename(&tmp, &path)).map_err(|e| e.to_string())?;
            Ok(json!({"ok": true, "remembered": running}))
        }
        Request::ShareStop { site, cwd } => {
            let site = match site_commands(state, site.clone(), &cwd) {
                Ok((site, ..)) => site,
                Err(e) => match site {
                    Some(s) if state.commands.has_runs(&s) => s,
                    _ => return Err(e),
                },
            };
            let stopped = !state.commands.stop(&site, Some(commands::SHARE)).is_empty();
            Ok(json!({"ok": true, "site": site, "stopped": stopped}))
        }
    }
}

/// Where `RememberRunning` notes the commands to start again (D-040).
fn resume_file(paths: &Paths) -> PathBuf {
    paths.run_dir().join("resume.json")
}

/// A note older than this is not followed: it was not this restart's.
const RESUME_WITHIN_SECS: u64 = 600;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// D-040: start again the commands `RememberRunning` noted just before this daemon's start (an
/// update or `devid setup`). The note is used once; an old one, a removed command or a site that
/// is gone is skipped. Never at login or after a crash: nothing writes the note then.
fn resume_commands(state: &State) {
    let path = resume_file(&state.paths);
    let Ok(text) = fs::read_to_string(&path) else { return };
    let note: Value = serde_json::from_str(&text).unwrap_or_default();
    if now_secs().saturating_sub(note["at"].as_u64().unwrap_or(0)) <= RESUME_WITHIN_SECS {
        start_noted(state, &note);
    }
    // Removed last: once it is gone, this restart's resume is done.
    let _ = fs::remove_file(&path);
}

fn start_noted(state: &State, note: &Value) {
    let config = state.config.lock().unwrap().clone();
    let sites = sites::resolve(&config);
    for c in note["commands"].as_array().into_iter().flatten() {
        let (site, name) = (c["site"].as_str().unwrap_or_default(), c["name"].as_str().unwrap_or_default());
        let saved = config.apps.get(site).and_then(|a| a.commands.iter().find(|x| x.name == name));
        if let (Some(saved), Some(s)) = (saved, sites.iter().find(|s| s.name == site)) {
            match state.commands.start(
                site,
                name,
                &saved.run,
                &s.path,
                s.dev_port(),
                Some("started again after Devid restarted for an update"),
            ) {
                Ok(_) => eprintln!("devid: started {site} · {name} again after the restart"),
                Err(e) => eprintln!("devid: could not start {site} · {name} again: {e}"),
            }
        }
    }
}

/// A site's Share (D-037): null, or its state ("starting" until cloudflared prints the URL,
/// "running", "exited"), URL and log name.
fn share_state(state: &State, site: &str) -> Value {
    let run = state.commands.state(site, commands::SHARE);
    let log = commands::log_name(site, commands::SHARE);
    match run["state"].as_str() {
        Some("running") => {
            let url = fs::read_to_string(state.paths.log_dir().join(&log)).ok().and_then(|t| commands::share_url(&t));
            json!({"state": if url.is_some() { "running" } else { "starting" }, "url": url, "log": log})
        }
        Some("exited") => json!({"state": "exited", "url": null, "log": log}),
        _ => Value::Null,
    }
}

/// The port cloudflared reaches a shared site on: the site's own local port when Devid serves it
/// there (D-037), else 80 when pf forwards it to the web server, else the web server's port. On the
/// owner's Mac (2026-11-09) new direct connections to pf's redirect targets (8088, 8443) hung in
/// the kernel while :80 and a local port answered, so the redirect target is the last choice.
fn share_origin(local_port: Option<u16>, standard_ports: bool, http_port: u16) -> u16 {
    local_port.unwrap_or(if standard_ports { 80 } else { http_port })
}

/// An existing cloudflared (D-037: Devid does not download it). `DEVID_CLOUDFLARED` overrides it
/// for tests.
fn cloudflared() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DEVID_CLOUDFLARED").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    ["/usr/local/bin", "/opt/homebrew/bin"]
        .into_iter()
        .map(PathBuf::from)
        .chain(std::env::split_paths(&path))
        .map(|d| d.join("cloudflared"))
        .find(|p| p.is_file())
}

/// A proxy site's dev server port held by a program Devid did not start (none of the site's
/// commands runs): a dev server on that port would only fail with EADDRINUSE in its log, so the
/// start reply says so first. Owner's case, 2026-11-09: an `npm start` left from a closed Terminal.
fn port_warning(state: &State, site: &str) -> Option<String> {
    let config = state.config.lock().unwrap().clone();
    let s = sites::resolve(&config).into_iter().find(|s| s.name == site)?;
    let ours = state.commands.running().iter().any(|(x, n)| x == site && n != commands::SHARE);
    (s.mode == Mode::Proxy && !ours && ports::in_use(s.upstream)).then(|| {
        let who = ports::holder(s.upstream).unwrap_or_else(|| "another program".into());
        format!(
            "port {} is already in use by {who}, which Devid did not start; a dev server on that port cannot start until it stops",
            s.upstream
        )
    })
}

/// The site `site` (or the one containing `cwd`), its folder and its saved commands.
fn site_commands(state: &State, site: Option<String>, cwd: &str) -> Result<(String, PathBuf, Vec<AppCommand>), String> {
    let config = state.config.lock().unwrap();
    let sites = sites::resolve(&config);
    let name = pick_site(&sites, site, cwd)?;
    let dir = sites.iter().find(|s| s.name == name).map(|s| s.path.clone()).unwrap_or_default();
    Ok((name.clone(), dir, config.apps.get(&name).map(|a| a.commands.clone()).unwrap_or_default()))
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
/// `on_downloaded` runs once the release's archive is checked and unpacked.
fn install_php(
    state: &State,
    version: &str,
    from: Option<String>,
    manifest: Option<&Value>,
    on_downloaded: &dyn Fn(),
) -> Result<String, String> {
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
            let dir = releases::download(&state.paths, &artifact).map_err(|e| e.to_string())?;
            on_downloaded();
            dir
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

/// Install catalog extensions one after another, each verified and preflighted on its own, then
/// restart php-fpm once. One failing does not undo the others; the reply has a result per name.
fn install_extensions(state: &State, version: &str, names: &[String]) -> Result<Value, String> {
    if names.is_empty() {
        return Err("name at least one extension".into());
    }
    let manifest = releases::fetch_manifest(&state.paths).map_err(|e| e.to_string())?;
    let catalog = phpext::catalog(&manifest, version);
    let entry = |n: &str| catalog.iter().find(|e| e.name == n);
    {
        let mut job = state.ext_job.lock().unwrap();
        if job.is_some() {
            return Err("another extension install is running; wait for it to finish".into());
        }
        *job = Some(ExtJob {
            version: version.to_string(),
            total: names.len(),
            index: 0,
            name: String::new(),
            phase: "starting",
            done_bytes: 0,
            total_bytes: names.iter().filter_map(|n| entry(n)).map(|e| e.size).sum(),
            current: None,
            finished: 0,
        });
    }
    // Clears the job however this returns, so a failure never blocks the next install.
    struct Clear<'a>(&'a Mutex<Option<ExtJob>>);
    impl Drop for Clear<'_> {
        fn drop(&mut self) {
            *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }
    let _clear = Clear(&state.ext_job);
    let update = |f: &dyn Fn(&mut ExtJob)| {
        if let Some(job) = state.ext_job.lock().unwrap().as_mut() {
            f(job);
        }
    };
    let mut results = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let current = entry(name).map(|e| (releases::archive_path(&state.paths, &e.artifact), e.size));
        update(&|j| {
            j.index = i + 1;
            j.name = name.clone();
            j.phase = "downloading";
            j.current = current.clone();
        });
        let result = phpext::install(&state.paths, version, name, &manifest, &|| update(&|j| j.phase = "checking"));
        update(&|j| {
            j.done_bytes += j.current.take().map_or(0, |(_, size)| size);
            j.finished += 1;
        });
        results.push(match result {
            Ok(v) => json!({"name": name, "ok": true, "installed": v}),
            Err(e) => json!({"name": name, "ok": false, "error": e.to_string()}),
        });
    }
    let restarted = results.iter().any(|r| r["ok"] == true);
    if restarted {
        update(&|j| j.phase = "restarting");
        state.supervisor.restart(&php_service(version));
    }
    Ok(json!({"ok": true, "version": version, "results": results, "restarted": restarted}))
}

/// Install a web server from `from` or else from the release manifest (Apache: the system httpd).
/// `on_downloaded` runs once the release's archive is checked and unpacked.
fn install_server(
    state: &State,
    server: &str,
    from: Option<String>,
    manifest: Option<&Value>,
    on_downloaded: &dyn Fn(),
) -> Result<(), String> {
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
            let dir = releases::download(&state.paths, &artifact).map_err(|e| e.to_string())?;
            on_downloaded();
            Some(dir.join(server))
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

/// `update`: newer PHP, web servers and catalog extensions in the release; installs them unless
/// `check`. The periodic check (D-028) runs it with `check` on.
fn find_updates(state: &State, check: bool) -> Result<Value, String> {
    let manifest = releases::fetch_manifest(&state.paths).map_err(|e| e.to_string())?;
    let (plan, unknown) = plan_updates(state, &manifest);
    let changes: Vec<Value> = plan.iter().map(|p| p.change.clone()).collect();
    if !check && !plan.is_empty() {
        apply_updates(state, &manifest, plan)?;
    }
    Ok(json!({"ok": true, "changes": changes, "unknown": unknown, "applied": !check, "release": manifest["tag"]}))
}

/// One change `update` makes: its line for the reply (with the download `size`), the archive it
/// downloads (path and size, for progress) and the install.
struct Planned {
    change: Value,
    download: Option<(std::path::PathBuf, u64)>,
    install: Install,
}

enum Install {
    Php(String),
    /// A catalog extension of a PHP version, reinstalled from the release.
    Ext(String, String),
    /// A catalog extension the release no longer has, after its PHP was updated.
    RemoveExt(String, String),
    Server(&'static str),
}

/// What is newer in the release, in the order `update` installs it: each PHP version, then its
/// catalog extensions whose archive changed (a new version, or a rebuild for a new PHP or OpenSSL);
/// one the release no longer offers stays, unless its PHP was updated: it was built for the old
/// PHP, so it is removed. Then nginx and Caddy. `unknown`: installed, version unreadable (e.g. the
/// probe timed out), never treated as older, which would offer and run a needless reinstall.
fn plan_updates(state: &State, manifest: &Value) -> (Vec<Planned>, Vec<String>) {
    let (mut plan, mut unknown) = (Vec::new(), Vec::new());
    let archive = |a: &releases::Artifact| Some((releases::archive_path(&state.paths, a), a.size));
    for version in php::installed(&state.paths) {
        let Some(a) = releases::artifact(manifest, "php", Some(&version)) else { continue };
        let Some(current) = php::full_version(&state.paths, &version) else {
            unknown.push(format!("php {version}"));
            continue;
        };
        let php_updated = releases::newer(&a.version, &current);
        if php_updated {
            plan.push(Planned {
                change: json!({"what": format!("php {version}"), "from": current, "to": a.version, "size": a.size}),
                download: archive(&a),
                install: Install::Php(version.clone()),
            });
        }
        let catalog = phpext::catalog(manifest, &version);
        for ext in phpext::installed(&state.paths, &version) {
            let what = format!("{} for php {version}", ext.name);
            match catalog.iter().find(|e| e.name == ext.name) {
                Some(e) if e.artifact.sha256 != ext.sha256 => {
                    let to =
                        if e.version == ext.version { format!("{} (new build)", e.version) } else { e.version.clone() };
                    plan.push(Planned {
                        change: json!({"what": what, "from": ext.version, "to": to, "size": e.size}),
                        download: Some((releases::archive_path(&state.paths, &e.artifact), e.size)),
                        install: Install::Ext(version.clone(), ext.name),
                    });
                }
                None if php_updated => plan.push(Planned {
                    change: json!({"what": what, "from": ext.version, "to": "removed (not in this release)", "size": 0}),
                    download: None,
                    install: Install::RemoveExt(version.clone(), ext.name),
                }),
                _ => {}
            }
        }
    }
    for server in ["nginx", "caddy"] {
        if !webserver::installed(&state.paths, server) {
            continue;
        }
        let Some(a) = releases::artifact(manifest, server, None) else { continue };
        let Some(current) = webserver::installed_version(&state.paths, server) else {
            unknown.push(server.to_string());
            continue;
        };
        if releases::newer(&a.version, &current) {
            plan.push(Planned {
                change: json!({"what": server, "from": current, "to": a.version, "size": a.size}),
                download: archive(&a),
                install: Install::Server(server),
            });
        }
    }
    (plan, unknown)
}

/// Installs `plan` in order, one at a time, with its progress in `update_job`. php-fpm restarts
/// after a version's extensions change. The first failure stops it; what was installed stays.
fn apply_updates(state: &State, manifest: &Value, plan: Vec<Planned>) -> Result<(), String> {
    *state.update_job.lock().unwrap() = Some(UpdateJob {
        items: plan.iter().map(|p| p.change.clone()).collect(),
        index: 0,
        phase: "starting",
        current: None,
        done_bytes: 0,
        total_bytes: plan.iter().filter_map(|p| p.download.as_ref()).map(|d| d.1).sum(),
        finished: 0,
        state: "running",
        error: None,
    });
    let update = |f: &dyn Fn(&mut UpdateJob)| {
        if let Some(job) = state.update_job.lock().unwrap().as_mut() {
            f(job);
        }
    };
    let installing = || update(&|j| j.phase = "installing");
    let ext_version = |p: Option<&Planned>| match p.map(|p| &p.install) {
        Some(Install::Ext(v, _) | Install::RemoveExt(v, _)) => Some(v.clone()),
        _ => None,
    };
    let mut result = Ok(());
    for (i, p) in plan.iter().enumerate() {
        if let Some((path, _)) = &p.download {
            // A leftover archive from an earlier try would count as already downloaded.
            let _ = std::fs::remove_file(path);
        }
        update(&|j| {
            j.index = i;
            j.phase = if p.download.is_some() { "downloading" } else { "installing" };
            j.current = p.download.clone();
        });
        result = match &p.install {
            Install::Php(version) => install_php(state, version, None, Some(manifest), &installing).map(|_| ()),
            Install::Ext(version, name) => phpext::install(&state.paths, version, name, manifest, &installing)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Install::RemoveExt(version, name) => phpext::remove(&state.paths, version, name).map_err(|e| e.to_string()),
            Install::Server(server) => install_server(state, server, None, Some(manifest), &installing),
        };
        if result.is_ok()
            && let Some(version) = ext_version(Some(p)).filter(|v| ext_version(plan.get(i + 1)).as_ref() != Some(v))
        {
            update(&|j| j.phase = "restarting");
            state.supervisor.restart(&php_service(&version));
        }
        if result.is_err() {
            break;
        }
        update(&|j| {
            j.done_bytes += j.current.take().map_or(0, |(_, size)| size);
            j.finished += 1;
        });
    }
    let error = result.as_ref().err().cloned();
    update(&|j| {
        j.state = if error.is_some() { "failed" } else { "done" };
        j.error = error.clone();
    });
    result
}

/// Keeps what an update check found, for `status` (D-028). After an update the changes are gone;
/// `devid` is the newer devid release, kept from the last check when this one did not look.
fn remember_updates(state: &State, reply: &Value, devid: Option<Option<String>>) {
    let mut cache = state.updates.lock().unwrap();
    let devid = devid.unwrap_or_else(|| cache["devid"].as_str().map(String::from));
    let changes = if reply["applied"] == true { json!([]) } else { reply["changes"].clone() };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    *cache = json!({"checked_at": now, "devid": devid, "changes": changes, "release": reply["release"]});
}

/// D-028: check for updates a few minutes after start, then every `update_check_minutes`.
fn spawn_update_checks(state: Arc<State>) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(180));
        loop {
            let minutes = state.config.lock().unwrap().update_check_minutes;
            if minutes == 0 {
                std::thread::sleep(std::time::Duration::from_secs(300)); // look at the setting again later
                continue;
            }
            check_updates(&state);
            std::thread::sleep(std::time::Duration::from_secs(u64::from(minutes) * 60));
        }
    });
}

fn check_updates(state: &State) {
    let _one = state.update_lock.lock().unwrap();
    let devid = releases::latest_devid(&state.paths).ok().filter(|l| releases::newer(l, crate::VERSION));
    match find_updates(state, true) {
        Ok(reply) => remember_updates(state, &reply, Some(devid)),
        Err(e) => eprintln!("devid: update check failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn share_avoids_pfs_redirect_target() {
        assert_eq!(super::share_origin(Some(8000), true, 8088), 8000, "the site's own local port first");
        assert_eq!(super::share_origin(None, true, 8088), 80, "then :80 when pf forwards it");
        assert_eq!(super::share_origin(None, false, 8088), 8088, "the web server's port without pf");
    }

    #[test]
    fn update_progress_shows_the_check_and_ends_at_100() {
        let dir = std::env::temp_dir().join(format!("devid-updatejob-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("php.tar.gz");
        std::fs::write(&file, vec![0u8; 50]).unwrap();
        let mut job = super::UpdateJob {
            items: vec![serde_json::json!({"what": "php 8.4"}), serde_json::json!({"what": "nginx"})],
            index: 0,
            phase: "downloading",
            current: Some((file.clone(), 100)),
            done_bytes: 0,
            total_bytes: 200,
            finished: 0,
            state: "running",
            error: None,
        };
        assert_eq!(
            (job.json()["phase"].clone(), job.json()["got"].clone(), job.json()["percent"].clone()),
            ("downloading".into(), 50.into(), 22.into())
        );
        std::fs::write(&file, vec![0u8; 100]).unwrap();
        assert_eq!(job.json()["phase"], "checking", "a complete archive is being checked");
        job.state = "done";
        assert_eq!(job.json()["percent"], 100);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn extension_progress_counts_bytes_then_steps() {
        let dir = std::env::temp_dir().join(format!("devid-extjob-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ext-b.tar.gz");
        std::fs::write(&file, vec![0u8; 300]).unwrap();
        // Two extensions of 100 and 900 bytes: the first is done, 300 bytes of the second arrived.
        let mut job = super::ExtJob {
            version: "8.4".into(),
            total: 2,
            index: 2,
            name: "b".into(),
            phase: "downloading",
            done_bytes: 100,
            total_bytes: 1000,
            current: Some((file.clone(), 900)),
            finished: 1,
        };
        assert_eq!(job.percent(), 36 + 3); // 90 % of 400/1000, plus 10 % of 1 in 3 steps
        job.phase = "checking";
        assert_eq!(job.percent(), 90 + 3);
        job.current = None;
        job.done_bytes = 1000;
        job.finished = 2;
        job.phase = "restarting";
        assert_eq!(job.percent(), 96);
        assert_eq!(job.json()["name"], "b");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
