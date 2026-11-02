//! Keeps one php-fpm master per wanted PHP version alive, restarting it if it dies.

use crate::{paths::Paths, php};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
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

struct Slot {
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

pub struct Supervisor {
    paths: Paths,
    slots: Mutex<BTreeMap<String, Arc<Slot>>>,
}

impl Supervisor {
    pub fn new(paths: Paths) -> Self {
        Self { paths, slots: Mutex::new(BTreeMap::new()) }
    }

    /// Start versions in `wanted` that are not running, stop the ones that are no longer wanted.
    pub fn reconcile(&self, wanted: &BTreeSet<String>) {
        let mut slots = self.slots.lock().unwrap();
        let stale: Vec<String> = slots.keys().filter(|v| !wanted.contains(*v)).cloned().collect();
        for version in stale {
            if let Some(slot) = slots.remove(&version) {
                stop_slot(&slot);
            }
        }
        for version in wanted {
            let restart = slots.get(version).is_some_and(|s| s.failed.load(Ordering::SeqCst));
            if restart {
                // A failed version gets a fresh start when it is wanted again (e.g. after reinstall).
                stop_slot(&slots.remove(version).unwrap());
            }
            if !slots.contains_key(version) {
                slots.insert(version.clone(), self.spawn(version.clone()));
            }
        }
    }

    pub fn state(&self) -> BTreeMap<String, ProcState> {
        let slots = self.slots.lock().unwrap();
        slots
            .iter()
            .map(|(v, s)| {
                let state = if s.failed.load(Ordering::SeqCst) {
                    ProcState::Failed
                } else {
                    match s.pid.load(Ordering::SeqCst) {
                        0 => ProcState::Starting,
                        pid => ProcState::Running(pid),
                    }
                };
                (v.clone(), state)
            })
            .collect()
    }

    pub fn stop_all(&self) {
        self.reconcile(&BTreeSet::new());
    }

    fn spawn(&self, version: String) -> Arc<Slot> {
        let slot = Arc::new(Slot {
            stop: AtomicBool::new(false),
            pid: AtomicU32::new(0),
            failed: AtomicBool::new(false),
            handle: Mutex::new(None),
        });
        let (paths, s) = (self.paths.clone(), Arc::clone(&slot));
        let handle = thread::spawn(move || watch(&paths, &version, &s));
        *slot.handle.lock().unwrap() = Some(handle);
        slot
    }
}

fn watch(paths: &Paths, version: &str, slot: &Slot) {
    let mut crashes: Vec<Instant> = Vec::new();
    while !slot.stop.load(Ordering::SeqCst) {
        kill_orphan(paths, version);
        let mut child = match start(paths, version) {
            Ok(child) => child,
            Err(e) => {
                eprintln!("devid: php-fpm {version} failed to start: {e}");
                slot.failed.store(true, Ordering::SeqCst);
                return;
            }
        };
        slot.pid.store(child.id(), Ordering::SeqCst);
        let status = child.wait();
        slot.pid.store(0, Ordering::SeqCst);
        if slot.stop.load(Ordering::SeqCst) {
            return;
        }

        eprintln!("devid: php-fpm {version} exited unexpectedly ({status:?}); restarting");
        let now = Instant::now();
        crashes.retain(|t| now.duration_since(*t) < CRASH_WINDOW);
        crashes.push(now);
        if crashes.len() >= MAX_CRASHES {
            eprintln!("devid: php-fpm {version} crashed {MAX_CRASHES} times in {CRASH_WINDOW:?}; giving up");
            slot.failed.store(true, Ordering::SeqCst);
            return;
        }
        thread::sleep(Duration::from_millis(500 * crashes.len() as u64));
    }
}

fn start(paths: &Paths, version: &str) -> std::io::Result<Child> {
    let etc = php::etc(paths, version);
    Command::new(php::fpm_bin(paths, version))
        .arg("--nodaemonize")
        .arg("--fpm-config")
        .arg(etc.join("php-fpm.conf"))
        .arg("-c")
        .arg(etc.join("php.ini"))
        // D-008: never fall back to compiled-in ini paths.
        .env("PHPRC", &etc)
        .env("PHP_INI_SCAN_DIR", etc.join("conf.d"))
        .stdin(Stdio::null())
        .spawn()
}

/// A master left over from a daemon that died would hold the socket; stop it first.
fn kill_orphan(paths: &Paths, version: &str) {
    let pid_file = php::pid_file(paths, version);
    let Some(pid) = fs::read_to_string(&pid_file).ok().and_then(|s| s.trim().parse::<i32>().ok()) else {
        return;
    };
    // Only signal it if that pid is still a php-fpm process; pids get reused.
    let comm = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output();
    if comm.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("php-fpm")) {
        signal(pid, libc::SIGQUIT);
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = fs::remove_file(pid_file);
}

/// Stop a version without ever hanging the daemon. SIGQUIT lets php-fpm finish in-flight requests,
/// but php-fpm drops signals that arrive while it is still starting up, so repeat it every second
/// and fall back to SIGKILL after `STOP_TIMEOUT`.
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
                eprintln!("devid: php-fpm pid {pid} ignored SIGQUIT for {STOP_TIMEOUT:?}; killing it");
                signal(pid, libc::SIGKILL);
            } else if last_quit.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
                signal(pid, libc::SIGQUIT);
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
