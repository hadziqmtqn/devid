//! Keeps child services (php-fpm masters, the web server, Mailpit, databases) alive, restarting them
//! if they die.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs, io,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Give up after this many unexpected exits within `CRASH_WINDOW`.
const MAX_CRASHES: usize = 5;
const CRASH_WINDOW: Duration = Duration::from_secs(60);

/// How long php-fpm, the web servers and Mailpit get to stop before SIGKILL.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How to run one service in the foreground.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(String, OsString)>,
    /// Written by the service itself, or by the supervisor when `write_pid` is set; used to find an
    /// orphan left by a dead daemon. Only its first line is read (PostgreSQL's postmaster.pid has more).
    pub pid_file: PathBuf,
    /// The service writes no pid file of its own (Mailpit), so the supervisor writes the child's pid.
    pub write_pid: bool,
    /// Part of the program's path that must match before an orphan is signalled (pids get reused).
    /// macOS's `ps -o comm=` shows the whole path; on Linux the orphan check compares the
    /// executable's path itself, since `comm` there is only the program's name.
    pub comm: &'static str,
    /// Graceful stop: SIGQUIT for php-fpm and nginx, SIGTERM for Caddy, Apache, Mailpit, MySQL and
    /// MongoDB (SIGQUIT makes Caddy quit abruptly and is not handled by Apache), SIGINT for
    /// PostgreSQL (its fast shutdown).
    pub stop_signal: libc::c_int,
    /// How long a stop may take before SIGKILL: `STOP_TIMEOUT`, or longer for a database to flush
    /// (D-061).
    pub stop_timeout: Duration,
    /// A file for what the service prints; None leaves it in the daemon's log (the services before
    /// the databases write logs of their own).
    pub log: Option<PathBuf>,
    /// The service counts as starting until this port on 127.0.0.1 answers (D-061: a database).
    pub ready_port: Option<u16>,
    /// A file the service creates once it takes connections (a database's socket), so another
    /// program on `ready_port` never counts as this service. Removed before each start, so a file
    /// left by a crash does not count either.
    pub ready_file: Option<PathBuf>,
}

struct Slot {
    spec: Spec,
    stop: AtomicBool,
    pid: AtomicU32,
    ready: AtomicBool,
    failed: AtomicBool,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcState {
    Running(u32),
    Starting,
    Failed,
}

#[derive(Default)]
pub struct Supervisor {
    slots: Mutex<BTreeMap<String, Arc<Slot>>>,
    /// One reconcile or restart at a time. Services stop outside `slots`, so `state` and `signal`
    /// never wait for a slow stop (D-061: a database may take seconds to flush).
    changes: Mutex<()>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Make the running set match `wanted`: start new services, stop unwanted ones, and restart
    /// ones whose spec changed or that had failed (e.g. after a reinstall).
    pub fn reconcile(&self, wanted: &BTreeMap<String, Spec>) {
        let _changing = self.changes.lock().unwrap();
        let outdated: Vec<Arc<Slot>> = {
            let mut slots = self.slots.lock().unwrap();
            let names: Vec<String> = slots
                .iter()
                .filter(|(name, slot)| {
                    wanted.get(*name).is_none_or(|spec| *spec != slot.spec) || slot.failed.load(Ordering::SeqCst)
                })
                .map(|(name, _)| name.clone())
                .collect();
            names.iter().filter_map(|name| slots.remove(name)).collect()
        };
        stop_slots(outdated);
        let mut slots = self.slots.lock().unwrap();
        for (name, spec) in wanted {
            if !slots.contains_key(name) {
                slots.insert(name.clone(), spawn(name.clone(), spec.clone()));
            }
        }
    }

    /// Restart one service on its current spec (e.g. new binaries after a reinstall).
    pub fn restart(&self, name: &str) {
        let _changing = self.changes.lock().unwrap();
        let Some(slot) = self.slots.lock().unwrap().remove(name) else { return };
        let spec = slot.spec.clone();
        stop_slot(&slot);
        self.slots.lock().unwrap().insert(name.to_string(), spawn(name.to_string(), spec));
    }

    /// Send a signal to a running service, e.g. SIGHUP for an nginx config reload.
    /// Returns false if it is not running.
    pub fn signal(&self, name: &str, sig: libc::c_int) -> bool {
        let slots = self.slots.lock().unwrap();
        match slots.get(name).map(|s| s.pid.load(Ordering::SeqCst)) {
            Some(pid) if pid != 0 => {
                signal(pid as i32, sig);
                true
            }
            _ => false,
        }
    }

    pub fn state(&self) -> BTreeMap<String, ProcState> {
        let slots = self.slots.lock().unwrap();
        slots
            .iter()
            .map(|(name, s)| {
                let waits = s.spec.ready_port.is_some() || s.spec.ready_file.is_some();
                let ready = !waits || s.ready.load(Ordering::SeqCst);
                let state = if s.failed.load(Ordering::SeqCst) {
                    ProcState::Failed
                } else {
                    match s.pid.load(Ordering::SeqCst) {
                        0 => ProcState::Starting,
                        _ if !ready => ProcState::Starting,
                        pid => ProcState::Running(pid),
                    }
                };
                (name.clone(), state)
            })
            .collect()
    }

    pub fn stop_all(&self) {
        self.reconcile(&BTreeMap::new());
    }
}

fn spawn(name: String, spec: Spec) -> Arc<Slot> {
    let slot = Arc::new(Slot {
        spec,
        stop: AtomicBool::new(false),
        pid: AtomicU32::new(0),
        ready: AtomicBool::new(false),
        failed: AtomicBool::new(false),
        handle: Mutex::new(None),
    });
    let s = Arc::clone(&slot);
    let handle = thread::spawn(move || watch(&name, &s));
    *slot.handle.lock().unwrap() = Some(handle);
    slot
}

fn watch(name: &str, slot: &Slot) {
    let mut crashes: Vec<Instant> = Vec::new();
    while !slot.stop.load(Ordering::SeqCst) {
        kill_orphan(&slot.spec);
        if let Some(file) = &slot.spec.ready_file {
            let _ = fs::remove_file(file);
        }
        let mut child = match start(&slot.spec) {
            Ok(child) => child,
            Err(e) => {
                eprintln!("devid: {name} failed to start: {e}");
                slot.failed.store(true, Ordering::SeqCst);
                return;
            }
        };
        slot.pid.store(child.id(), Ordering::SeqCst);
        if slot.spec.write_pid {
            let _ = fs::write(&slot.spec.pid_file, child.id().to_string());
        }
        let status = match slot.spec.ready_port.is_some() || slot.spec.ready_file.is_some() {
            true => wait_ready(slot, &mut child),
            false => child.wait(),
        };
        slot.pid.store(0, Ordering::SeqCst);
        slot.ready.store(false, Ordering::SeqCst);
        if slot.stop.load(Ordering::SeqCst) {
            return;
        }

        eprintln!("devid: {name} exited unexpectedly ({status:?}); restarting");
        let now = Instant::now();
        crashes.retain(|t| now.duration_since(*t) < CRASH_WINDOW);
        crashes.push(now);
        if crashes.len() >= MAX_CRASHES {
            eprintln!("devid: {name} crashed {MAX_CRASHES} times in {CRASH_WINDOW:?}; giving up");
            slot.failed.store(true, Ordering::SeqCst);
            return;
        }
        thread::sleep(Duration::from_millis(500 * crashes.len() as u64));
    }
}

/// Mark the service ready once its port answers and its ready file exists, unless it exits or is
/// stopped first; then wait for it to exit.
fn wait_ready(slot: &Slot, child: &mut Child) -> io::Result<ExitStatus> {
    let spec = &slot.spec;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if slot.stop.load(Ordering::SeqCst) {
            break;
        }
        if spec.ready_file.as_ref().is_none_or(|f| f.exists()) && spec.ready_port.is_none_or(crate::ports::in_use) {
            slot.ready.store(true, Ordering::SeqCst);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    child.wait()
}

fn start(spec: &Spec) -> io::Result<Child> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args).stdin(Stdio::null());
    for (key, value) in &spec.env {
        cmd.env(key, value);
    }
    if let Some(log) = &spec.log {
        if let Some(dir) = log.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = fs::OpenOptions::new().create(true).append(true).open(log)?;
        cmd.stdout(file.try_clone()?).stderr(file);
    }
    // The daemon blocks its stop signals (server.rs) and children inherit the mask; without this
    // nginx would never see SIGHUP (reload) and php-fpm/nginx would ignore launchd's SIGTERM.
    // SAFETY: runs between fork and exec; sigemptyset/pthread_sigmask are async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            let mut empty: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut empty);
            libc::pthread_sigmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
            Ok(())
        });
    }
    keep_no_fds(&mut cmd);
    cmd.spawn()
}

/// Makes a long-lived child drop all of the daemon's file descriptors at exec. On macOS, std
/// marks a new pipe or socket close-on-exec only after creating it, so a child forked by another
/// thread in between inherits it. If that is another spawn's exec-status pipe, that spawn() blocks
/// for as long as the child lives: php-fpm 8.4 ran for days while its slot said "starting".
pub(crate) fn keep_no_fds(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    // Read before fork. launchd gives the daemon 256; a shell may allow 60,000, and sweeping
    // that many would cost about 45 ms per child.
    // shortcut: fds above 4096 stay inheritable; fine while the daemon holds a few dozen.
    // SAFETY: getdtablesize has no preconditions.
    let limit = unsafe { libc::getdtablesize() }.min(4096);
    // SAFETY: runs between fork and exec, where fcntl is async-signal-safe; it only sets a flag
    // on fds this process holds.
    unsafe {
        cmd.pre_exec(move || {
            for fd in 3..limit {
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
            Ok(())
        });
    }
}

/// The pid on a pid file's first line.
fn first_pid(text: &str) -> Option<i32> {
    text.lines().next()?.trim().parse().ok()
}

/// Whether `pid` runs `spec`'s program. On Linux the executable's path says so (an updated binary
/// shows " (deleted)" after it); `ps -o comm=` there is only the name, which DBngin's or the
/// system's server would share. On macOS `comm` is the whole path.
fn runs_program(pid: i32, spec: &Spec) -> bool {
    #[cfg(target_os = "linux")]
    if let Ok(exe) = fs::read_link(format!("/proc/{pid}/exe")) {
        let exe = exe.to_string_lossy();
        let exe = std::path::Path::new(exe.strip_suffix(" (deleted)").unwrap_or(&exe));
        return exe == spec.program || fs::canonicalize(&spec.program).is_ok_and(|p| p == exe);
    }
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(spec.comm))
}

/// A service left over from a daemon that died would hold its socket, port or data folder; stop it
/// first, by SIGKILL if it outlasts its stop timeout.
fn kill_orphan(spec: &Spec) {
    let Some(pid) = fs::read_to_string(&spec.pid_file).ok().and_then(|s| first_pid(&s)) else {
        return;
    };
    if pid > 0 && alive(pid) && runs_program(pid, spec) {
        signal(pid, spec.stop_signal);
        let deadline = Instant::now() + spec.stop_timeout;
        while alive(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        if alive(pid) {
            eprintln!("devid: an earlier {} (pid {pid}) did not stop; killing it", spec.program.display());
            signal(pid, libc::SIGKILL);
            let deadline = Instant::now() + Duration::from_secs(2);
            while alive(pid) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    let _ = fs::remove_file(&spec.pid_file);
}

/// Stop several services at once (on daemon exit every service stops together, and a database's
/// flush does not hold up php-fpm).
fn stop_slots(slots: Vec<Arc<Slot>>) {
    let threads: Vec<_> = slots.into_iter().map(|slot| thread::spawn(move || stop_slot(&slot))).collect();
    for t in threads {
        let _ = t.join();
    }
}

/// Stop a service without ever hanging the daemon. Its stop signal lets it finish in-flight
/// requests, but php-fpm drops signals that arrive while it is still starting up, so repeat it
/// every second and fall back to SIGKILL after the spec's stop timeout.
fn stop_slot(slot: &Slot) {
    let timeout = slot.spec.stop_timeout;
    slot.stop.store(true, Ordering::SeqCst);
    let Some(handle) = slot.handle.lock().unwrap().take() else { return };
    let start = Instant::now();
    let mut last_quit: Option<Instant> = None;
    while !handle.is_finished() {
        let pid = slot.pid.load(Ordering::SeqCst) as i32;
        if pid != 0 {
            if start.elapsed() >= timeout {
                eprintln!("devid: pid {pid} ignored its stop signal for {timeout:?}; killing it");
                signal(pid, libc::SIGKILL);
            } else if last_quit.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
                signal(pid, slot.spec.stop_signal);
                last_quit = Some(Instant::now());
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = handle.join();
}

fn signal(pid: i32, sig: libc::c_int) {
    // SAFETY: kill(2) has no memory-safety preconditions.
    unsafe {
        libc::kill(pid, sig);
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener, os::fd::FromRawFd, sync::mpsc};

    fn sh(script: &str) -> Spec {
        Spec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            env: Vec::new(),
            pid_file: PathBuf::new(),
            write_pid: false,
            comm: "sh",
            stop_signal: libc::SIGTERM,
            stop_timeout: STOP_TIMEOUT,
            log: None,
            ready_port: None,
            ready_file: None,
        }
    }

    fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cond() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// A pipe that another thread has not yet marked close-on-exec must not stay open in a
    /// service. When it did, that thread's spawn() never returned, and php-fpm 8.4 ran for days
    /// while its slot still said "starting".
    #[test]
    fn a_service_keeps_none_of_the_daemons_fds() {
        let mut fds = [0; 2];
        // SAFETY: pipe(2) fills the array with two new fds, owned below.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: fds[0] is new and owned by nothing else.
        let mut read_end = unsafe { fs::File::from_raw_fd(fds[0]) };
        let mut spec = sh("sleep 30");
        spec.program = "/bin/sleep".into();
        spec.args = vec!["30".into()];
        let mut child = start(&spec).unwrap();
        // SAFETY: fds[1] is ours and is not used again.
        unsafe { libc::close(fds[1]) };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || tx.send(read_end.read_to_end(&mut Vec::new()).map_err(|e| e.to_string())));
        let eof = rx.recv_timeout(Duration::from_secs(5));
        let _ = child.kill();
        let _ = child.wait();
        assert!(matches!(eof, Ok(Ok(0))), "the service kept the daemon's pipe open: {eof:?}");
    }

    /// D-061: a database counts as starting until its port answers, and what it prints goes to its
    /// own log.
    #[test]
    fn ready_once_the_port_answers_and_output_goes_to_its_log() {
        let dir = std::env::temp_dir().join(format!("devid-supervisor-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut spec = sh("echo started; exec sleep 30");
        spec.ready_port = Some(port);
        spec.ready_file = Some(dir.join("db.sock"));
        spec.log = Some(dir.join("log/db.log"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("db.sock"), "left by a crash").unwrap();
        let sup = Supervisor::new();
        sup.reconcile(&BTreeMap::from([("db".to_string(), spec)]));
        wait_until("the log", || fs::read_to_string(dir.join("log/db.log")).is_ok_and(|t| t.contains("started")));
        assert_eq!(sup.state()["db"], ProcState::Starting, "no answer on the port yet");
        // The port answers (here the test listens, as the database would), but its socket is not
        // there yet: another program on the port does not count.
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(sup.state()["db"], ProcState::Starting, "the port alone is not enough");
        fs::write(dir.join("db.sock"), "").unwrap();
        wait_until("running", || matches!(sup.state()["db"], ProcState::Running(_)));
        sup.stop_all();
        drop(listener);
        assert!(sup.state().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A service that ignores its stop signal is killed after its own stop timeout, and `state`
    /// answers while it stops.
    #[test]
    fn a_stop_ends_in_sigkill_after_the_specs_timeout_without_blocking_state() {
        let mut spec = sh("trap '' TERM; while :; do sleep 0.1; done");
        spec.stop_timeout = Duration::from_millis(1500);
        let sup = Arc::new(Supervisor::new());
        sup.reconcile(&BTreeMap::from([("stubborn".to_string(), spec)]));
        wait_until("running", || matches!(sup.state().get("stubborn"), Some(ProcState::Running(_))));
        let stopping = Arc::clone(&sup);
        let started = Instant::now();
        let stopper = thread::spawn(move || stopping.stop_all());
        thread::sleep(Duration::from_millis(300));
        let asked = Instant::now();
        let _ = sup.state();
        assert!(asked.elapsed() < Duration::from_millis(200), "state waited for the stop");
        stopper.join().unwrap();
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(1400) && took < Duration::from_secs(5), "{took:?}");
    }

    #[test]
    fn the_orphan_check_reads_the_first_line_and_the_program() {
        assert_eq!(first_pid("4242\n/data\n1700000000\n"), Some(4242), "PostgreSQL's postmaster.pid");
        assert_eq!(first_pid(" 17 \n"), Some(17));
        assert_eq!(first_pid("not a pid"), None);
        let mut child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        let mut spec = sh("");
        spec.program = "/bin/sleep".into();
        spec.comm = "sleep";
        assert!(runs_program(child.id() as i32, &spec), "our own sleep");
        spec.program = "/bin/cat".into();
        spec.comm = "/bin/cat";
        assert!(!runs_program(child.id() as i32, &spec), "another program's path");
        let _ = child.kill();
        let _ = child.wait();
    }
}
