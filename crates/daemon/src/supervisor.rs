//! Keeps child services (php-fpm masters, the web server) alive, restarting them if they die.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
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

/// How to run one service in the foreground.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(String, OsString)>,
    /// Written by the service itself, or by the supervisor when `write_pid` is set; used to find an
    /// orphan left by a dead daemon.
    pub pid_file: PathBuf,
    /// The service writes no pid file of its own (Mailpit), so the supervisor writes the child's pid.
    pub write_pid: bool,
    /// Process name that must match before an orphan is signalled (pids get reused).
    pub comm: &'static str,
    /// Graceful stop: SIGQUIT for php-fpm and nginx, SIGTERM for Caddy and Apache
    /// (SIGQUIT makes Caddy quit abruptly and is not handled by Apache).
    pub stop_signal: libc::c_int,
}

struct Slot {
    spec: Spec,
    stop: AtomicBool,
    pid: AtomicU32,
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
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Make the running set match `wanted`: start new services, stop unwanted ones, and restart
    /// ones whose spec changed or that had failed (e.g. after a reinstall).
    pub fn reconcile(&self, wanted: &BTreeMap<String, Spec>) {
        let mut slots = self.slots.lock().unwrap();
        let outdated: Vec<String> = slots
            .iter()
            .filter(|(name, slot)| {
                wanted.get(*name).is_none_or(|spec| *spec != slot.spec) || slot.failed.load(Ordering::SeqCst)
            })
            .map(|(name, _)| name.clone())
            .collect();
        for name in outdated {
            stop_slot(&slots.remove(&name).unwrap());
        }
        for (name, spec) in wanted {
            if !slots.contains_key(name) {
                slots.insert(name.clone(), spawn(name.clone(), spec.clone()));
            }
        }
    }

    /// Restart one service on its current spec (e.g. new binaries after a reinstall).
    pub fn restart(&self, name: &str) {
        let mut slots = self.slots.lock().unwrap();
        if let Some(slot) = slots.remove(name) {
            let spec = slot.spec.clone();
            stop_slot(&slot);
            slots.insert(name.to_string(), spawn(name.to_string(), spec));
        }
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
                let state = if s.failed.load(Ordering::SeqCst) {
                    ProcState::Failed
                } else {
                    match s.pid.load(Ordering::SeqCst) {
                        0 => ProcState::Starting,
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
        let status = child.wait();
        slot.pid.store(0, Ordering::SeqCst);
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

fn start(spec: &Spec) -> std::io::Result<Child> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args).stdin(Stdio::null());
    for (key, value) in &spec.env {
        cmd.env(key, value);
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

/// A service left over from a daemon that died would hold its socket or port; stop it first.
fn kill_orphan(spec: &Spec) {
    let Some(pid) = fs::read_to_string(&spec.pid_file).ok().and_then(|s| s.trim().parse::<i32>().ok()) else {
        return;
    };
    // Only signal it if that pid still runs the expected program; pids get reused.
    let comm = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output();
    if comm.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(spec.comm)) {
        signal(pid, spec.stop_signal);
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = fs::remove_file(&spec.pid_file);
}

/// Stop a service without ever hanging the daemon. Its stop signal lets it finish in-flight
/// requests, but php-fpm drops signals that arrive while it is still starting up, so repeat it
/// every second and fall back to SIGKILL after `STOP_TIMEOUT`.
fn stop_slot(slot: &Slot) {
    const STOP_TIMEOUT: Duration = Duration::from_secs(10);
    slot.stop.store(true, Ordering::SeqCst);
    let Some(handle) = slot.handle.lock().unwrap().take() else { return };
    let start = Instant::now();
    let mut last_quit: Option<Instant> = None;
    while !handle.is_finished() {
        let pid = slot.pid.load(Ordering::SeqCst) as i32;
        if pid != 0 {
            if start.elapsed() >= STOP_TIMEOUT {
                eprintln!("devid: pid {pid} ignored its stop signal for {STOP_TIMEOUT:?}; killing it");
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
    use std::{io::Read, os::fd::FromRawFd, sync::mpsc};

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
        let spec = Spec {
            program: "/bin/sleep".into(),
            args: vec!["30".into()],
            env: Vec::new(),
            pid_file: PathBuf::new(),
            write_pid: false,
            comm: "sleep",
            stop_signal: libc::SIGTERM,
        };
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
}
