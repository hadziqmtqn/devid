//! User commands (D-036, Phase 8 A4): shell lines the owner saves for a site and starts or stops
//! from Devid. Each runs in the site's folder in its own session, with the login shell's
//! environment, nvm's Node and Devid's `php` first on PATH, and its output goes straight to its
//! log file. Nothing starts at login and nothing restarts by itself.

use crate::{install, paths::Paths};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        ffi::OsStringExt,
        process::{CommandExt, ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// A log past this size is cut to its last `LOG_KEEP` bytes.
const LOG_MAX: u64 = 10 * 1024 * 1024;
const LOG_KEEP: u64 = 1024 * 1024;

type Env = Vec<(OsString, OsString)>;

/// One start of a command. Its pid is also its process group (`setsid`).
struct Run {
    pgid: i32,
    since: u64,
    /// Set when the command ended by itself: {"code", "signal", "at"}.
    exit: Mutex<Option<Value>>,
}

pub struct Commands {
    paths: Paths,
    /// The login shell's environment, read at the first start; an error keeps the daemon's own.
    env: Mutex<Option<Result<Env, String>>>,
    /// (site, command name) -> its current or last run. Stopping removes the entry.
    runs: Mutex<BTreeMap<(String, String), Arc<Run>>>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Command names show in the UI and in log and pid file names: `dev`, `dev:pg:local`, `queue`.
pub fn valid_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

pub fn log_name(site: &str, name: &str) -> String {
    format!("app-{site}-{name}.log")
}

impl Commands {
    pub fn new(paths: Paths) -> Self {
        Self { paths, env: Mutex::new(None), runs: Mutex::new(BTreeMap::new()) }
    }

    fn pid_file(&self, site: &str, name: &str) -> PathBuf {
        self.paths.run_dir().join(format!("app-{site}-{name}.pid"))
    }

    /// Start `run` in `dir` unless it is running already; returns its pid.
    pub fn start(&self, site: &str, name: &str, run: &str, dir: &Path) -> Result<u32, String> {
        let key = (site.to_string(), name.to_string());
        let old = {
            let mut runs = self.runs.lock().unwrap();
            match runs.get(&key) {
                Some(r) if r.exit.lock().unwrap().is_none() => return Ok(r.pgid as u32),
                _ => runs.remove(&key),
            }
        };
        // What an exited command left in its group would still hold its ports.
        if let Some(old) = old {
            stop_groups(&[old.pgid]);
        }
        let (env, warning) = self.env_for(dir);
        let log_path = self.paths.log_dir().join(log_name(site, name));
        let open_log = || -> io::Result<File> {
            fs::create_dir_all(self.paths.log_dir())?;
            File::create(&log_path)?;
            // O_APPEND: when the log is cut (cap_log), the command carries on at the new end.
            let mut log = OpenOptions::new().append(true).open(&log_path)?;
            if let Some(w) = &warning {
                writeln!(log, "devid: {w}")?;
            }
            writeln!(log, "$ {run}")?;
            Ok(log)
        };
        let log = open_log().map_err(|e| format!("{}: {e}", log_path.display()))?;

        let mut runs = self.runs.lock().unwrap();
        if let Some(r) = runs.get(&key).filter(|r| r.exit.lock().unwrap().is_none()) {
            return Ok(r.pgid as u32);
        }
        let shell = env.iter().find(|(k, _)| k == "SHELL").map_or_else(|| "/bin/zsh".into(), |(_, v)| v.clone());
        let mut cmd = Command::new(shell);
        cmd.arg("-c").arg(run).current_dir(dir).env_clear().envs(env).stdin(Stdio::null());
        cmd.stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log);
        // SAFETY: runs between fork and exec; setsid, sigemptyset and pthread_sigmask are
        // async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                // Its own session and process group, so stop reaches npm, node, cargo and the app.
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                // The daemon blocks its stop signals (server.rs); a command must get Ctrl+C's SIGINT.
                let mut empty: libc::sigset_t = std::mem::zeroed();
                libc::sigemptyset(&mut empty);
                libc::pthread_sigmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
                Ok(())
            });
        }
        let child = cmd.spawn().map_err(|e| format!("could not start {run:?}: {e}"))?;
        let pgid = child.id() as i32;
        let pid_file = self.pid_file(site, name);
        let _ = fs::write(&pid_file, format!("{pgid} {}\n", start_time(pgid).unwrap_or_default()));
        let current = Arc::new(Run { pgid, since: now(), exit: Mutex::new(None) });
        runs.insert(key, Arc::clone(&current));
        thread::spawn(move || watch(child, &current, &log_path, &pid_file));
        Ok(pgid as u32)
    }

    /// Stop a site's command, or all of them when `name` is None, and wait until nothing of their
    /// process groups is left. Returns the names that were running.
    pub fn stop(&self, site: &str, name: Option<&str>) -> Vec<String> {
        let taken: Vec<((String, String), Arc<Run>)> = {
            let mut runs = self.runs.lock().unwrap();
            let keys: Vec<_> = runs.keys().filter(|(s, n)| s == site && name.is_none_or(|x| x == n)).cloned().collect();
            keys.into_iter().filter_map(|k| runs.remove(&k).map(|r| (k, r))).collect()
        };
        self.stop_runs(taken)
    }

    /// Every command, as the daemon stops.
    pub fn stop_all(&self) {
        let taken: Vec<_> = std::mem::take(&mut *self.runs.lock().unwrap()).into_iter().collect();
        self.stop_runs(taken);
    }

    fn stop_runs(&self, taken: Vec<((String, String), Arc<Run>)>) -> Vec<String> {
        // Before stopping: once stopped, their watchers record them as exited.
        let running =
            taken.iter().filter(|(_, r)| r.exit.lock().unwrap().is_none()).map(|((_, n), _)| n.clone()).collect();
        stop_groups(&taken.iter().map(|(_, r)| r.pgid).collect::<Vec<_>>());
        for ((site, name), run) in &taken {
            remove_pid_file(&self.pid_file(site, name), run.pgid);
        }
        running
    }

    /// Whether any command of `site` was started (for stopping one whose site is gone).
    pub fn has_runs(&self, site: &str) -> bool {
        self.runs.lock().unwrap().keys().any(|(s, _)| s == site)
    }

    /// (site, command name) of every running command, sorted.
    pub fn running(&self) -> Vec<(String, String)> {
        let runs = self.runs.lock().unwrap();
        runs.iter().filter(|(_, r)| r.exit.lock().unwrap().is_none()).map(|(k, _)| k.clone()).collect()
    }

    /// {"state": "running", "pid", "since"}, {"state": "exited", "code", "signal", "at"} or
    /// {"state": "stopped"}.
    pub fn state(&self, site: &str, name: &str) -> Value {
        let runs = self.runs.lock().unwrap();
        let Some(run) = runs.get(&(site.to_string(), name.to_string())) else { return json!({"state": "stopped"}) };
        match run.exit.lock().unwrap().clone() {
            None => json!({"state": "running", "pid": run.pgid, "since": run.since}),
            Some(mut exit) => {
                exit["state"] = "exited".into();
                exit
            }
        }
    }

    /// Process groups a crashed daemon left running: each `app-*.pid` whose process still has the
    /// start time saved with it (pids get reused). The files are removed; the caller stops the
    /// groups. Called once, before the daemon takes requests.
    pub fn leftovers(&self) -> Vec<i32> {
        let Ok(entries) = fs::read_dir(self.paths.run_dir()) else { return Vec::new() };
        let mut groups = Vec::new();
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if !(file.starts_with("app-") && file.ends_with(".pid")) {
                continue;
            }
            let text = fs::read_to_string(entry.path()).unwrap_or_default();
            if let Some((pid, started)) = text.trim().split_once(' ')
                && let Ok(pid) = pid.parse::<i32>()
                && start_time(pid).as_deref() == Some(started)
            {
                groups.push(pid);
            }
            let _ = fs::remove_file(entry.path());
        }
        groups
    }

    /// The environment for a command in `dir`, and a note for its log when the login shell's
    /// could not be read.
    fn env_for(&self, dir: &Path) -> (Env, Option<String>) {
        let mut cached = self.env.lock().unwrap();
        let login = cached.get_or_insert_with(|| {
            let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
            login_env(Path::new(&shell))
        });
        let (base, warning) = match login {
            Ok(env) => (env.clone(), None),
            Err(e) => (
                std::env::vars_os().collect(),
                Some(format!("could not read your login shell's environment ({e}); using the daemon's")),
            ),
        };
        drop(cached);
        let mut vars: BTreeMap<OsString, OsString> = base.into_iter().collect();
        let home = vars.get(&OsString::from("HOME")).map_or_else(crate::paths::user_home, PathBuf::from);
        let nvm = vars.get(&OsString::from("NVM_DIR")).map_or_else(|| home.join(".nvm"), PathBuf::from);
        // Devid's php and composer shims, then the folder's Node, then the login shell's PATH.
        let mut path = vec![install::bin_dir(&self.paths)];
        path.extend(nvm_bin(&nvm, dir));
        path.extend(
            vars.get(&OsString::from("PATH")).map(|p| std::env::split_paths(p).collect::<Vec<_>>()).unwrap_or_default(),
        );
        if let Ok(joined) = std::env::join_paths(path) {
            vars.insert("PATH".into(), joined);
        }
        (vars.into_iter().collect(), warning)
    }
}

/// Wait for the command to end, cutting its log when it grows past LOG_MAX.
fn watch(mut child: Child, run: &Run, log: &Path, pid_file: &Path) {
    let mut ticks = 0u32;
    let status = loop {
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => break Some(status),
            Err(_) => break None,
        }
        ticks += 1;
        if ticks.is_multiple_of(30) {
            let _ = cap_log(log);
        }
        thread::sleep(Duration::from_secs(1));
    };
    let (code, signal) = (status.and_then(|s| s.code()), status.and_then(|s| s.signal()));
    *run.exit.lock().unwrap() = Some(json!({"code": code, "signal": signal, "at": now()}));
    if !group_alive(run.pgid) {
        remove_pid_file(pid_file, run.pgid);
    }
}

/// Keep the last LOG_KEEP bytes (from a line start) of a log past LOG_MAX. Lines the command
/// writes between the read and the cut are lost.
fn cap_log(path: &Path) -> io::Result<()> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len <= LOG_MAX {
        return Ok(());
    }
    file.seek(SeekFrom::Start(len - LOG_KEEP))?;
    let mut tail = Vec::new();
    file.take(LOG_KEEP).read_to_end(&mut tail)?;
    let start = tail.iter().position(|b| *b == b'\n').map_or(0, |i| i + 1);
    OpenOptions::new().write(true).open(path)?.set_len(0)?;
    OpenOptions::new().append(true).open(path)?.write_all(&tail[start..])
}

fn remove_pid_file(path: &Path, pgid: i32) {
    // A newer start of the same command may have written it already.
    if fs::read_to_string(path).is_ok_and(|t| t.starts_with(&format!("{pgid} "))) {
        let _ = fs::remove_file(path);
    }
}

fn group_alive(pgid: i32) -> bool {
    // SAFETY: kill(2) with signal 0 only checks that the group has a process we may signal.
    unsafe { libc::kill(-pgid, 0) == 0 }
}

/// Stop process groups the way Ctrl+C does, then harder: SIGINT, SIGTERM after 5 s, SIGKILL after
/// 10 s. Returns when no process of any group is left (or 2 s after SIGKILL).
pub fn stop_groups(groups: &[i32]) {
    let send = |sig| {
        for &g in groups {
            // SAFETY: killpg(2) has no memory-safety preconditions.
            unsafe { libc::killpg(g, sig) };
        }
    };
    send(libc::SIGINT);
    let start = Instant::now();
    let (mut term, mut kill) = (false, false);
    while groups.iter().any(|&g| group_alive(g)) {
        let waited = start.elapsed();
        if !term && waited >= Duration::from_secs(5) {
            send(libc::SIGTERM);
            term = true;
        }
        if !kill && waited >= Duration::from_secs(10) {
            send(libc::SIGKILL);
            kill = true;
        }
        if waited >= Duration::from_secs(12) {
            eprintln!("devid: process groups {groups:?} survived SIGKILL");
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// `ps`'s start time of a process, to tell it from a later one with the same pid.
fn start_time(pid: i32) -> Option<String> {
    let out = Command::new("ps").args(["-o", "lstart=", "-p", &pid.to_string()]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

/// `$SHELL -l -c 'env -0'`: what a new Terminal tab gets from the login files, without the slow
/// interactive part (A1: 5.9 s with the owner's .zshrc). Ten seconds at most.
fn login_env(shell: &Path) -> Result<Env, String> {
    let child = Command::new(shell)
        .args(["-l", "-c", "env -0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", shell.display()))?;
    let pid = child.id() as i32;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let out = match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(out) => out.map_err(|e| e.to_string())?,
        Err(_) => {
            // SAFETY: kill(2) has no memory-safety preconditions.
            unsafe { libc::kill(pid, libc::SIGKILL) };
            return Err(format!("{} -l took longer than 10 s", shell.display()));
        }
    };
    if !out.status.success() {
        return Err(format!("{} -l failed ({})", shell.display(), out.status));
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter_map(|kv| {
            let eq = kv.iter().position(|b| *b == b'=')?;
            Some((OsString::from_vec(kv[..eq].to_vec()), OsString::from_vec(kv[eq + 1..].to_vec())))
        })
        .collect())
}

/// nvm's Node `bin` for a folder (D-036): the version in its `.nvmrc`, else nvm's default alias,
/// as the highest installed `versions/node/v<…>` that matches. None without nvm or a match.
pub fn nvm_bin(nvm_dir: &Path, folder: &Path) -> Option<PathBuf> {
    let read = |p: PathBuf| fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut want = read(folder.join(".nvmrc")).or_else(|| read(nvm_dir.join("alias/default")))?;
    // Aliases are files under alias/: default -> lts/* -> lts/jod -> v22.12.0.
    for _ in 0..5 {
        let safe = !want.contains("..") && want.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/*".contains(&b));
        match read(nvm_dir.join("alias").join(&want)).filter(|_| safe) {
            Some(next) => want = next,
            None => break,
        }
    }
    let parse =
        |v: &str| v.trim_start_matches('v').split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>();
    let wanted = match want.as_str() {
        "node" | "stable" => Vec::new(),
        w => parse(w)?,
    };
    let versions = nvm_dir.join("versions/node");
    fs::read_dir(&versions)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            parse(&name).filter(|v| v.starts_with(&wanted)).map(|v| (v, name))
        })
        .max()
        .map(|(_, name)| versions.join(name).join("bin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvm_picks_the_highest_matching_installed_node() {
        let root = std::env::temp_dir().join(format!("devid-nvm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (nvm, app) = (root.join("nvm"), root.join("app"));
        for v in ["v20.11.1", "v22.12.0", "v22.9.0", "v24.20.0"] {
            fs::create_dir_all(nvm.join("versions/node").join(v).join("bin")).unwrap();
        }
        fs::create_dir_all(nvm.join("alias/lts")).unwrap();
        fs::create_dir_all(&app).unwrap();
        let bin = |v: &str| Some(nvm.join("versions/node").join(v).join("bin"));

        assert_eq!(nvm_bin(&nvm, &app), None, "no .nvmrc and no default alias");
        fs::write(nvm.join("alias/default"), "24\n").unwrap();
        assert_eq!(nvm_bin(&nvm, &app), bin("v24.20.0"), "default alias");
        fs::write(app.join(".nvmrc"), "v22").unwrap();
        assert_eq!(nvm_bin(&nvm, &app), bin("v22.12.0"), ".nvmrc wins; 22.12 > 22.9, numerically");
        fs::write(app.join(".nvmrc"), "lts/*").unwrap();
        fs::write(nvm.join("alias/lts/*"), "lts/iron").unwrap();
        fs::write(nvm.join("alias/lts/iron"), "v20.11.1").unwrap();
        assert_eq!(nvm_bin(&nvm, &app), bin("v20.11.1"), "aliases are followed");
        fs::write(app.join(".nvmrc"), "node").unwrap();
        assert_eq!(nvm_bin(&nvm, &app), bin("v24.20.0"), "node: the newest");
        fs::write(app.join(".nvmrc"), "18").unwrap();
        assert_eq!(nvm_bin(&nvm, &app), None, "not installed: PATH stays as it is");
        fs::write(app.join(".nvmrc"), "../../etc/x").unwrap();
        assert_eq!(nvm_bin(&nvm, &app), None);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn names_and_log_cutting() {
        for ok in ["dev", "dev:pg:local", "queue.work", "tauri_2"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", "-x", ".x", "a b", "a/b", "../x", &"x".repeat(41)] {
            assert!(!valid_name(bad), "{bad}");
        }

        let log = std::env::temp_dir().join(format!("devid-caplog-test-{}.log", std::process::id()));
        let line = "x".repeat(99) + "\n";
        fs::write(&log, line.repeat((LOG_MAX / 100 + 1) as usize)).unwrap();
        cap_log(&log).unwrap();
        let kept = fs::read_to_string(&log).unwrap();
        assert!(kept.len() as u64 <= LOG_KEEP && kept.len() as u64 > LOG_KEEP - 100);
        assert!(kept.starts_with('x') && kept.ends_with('\n'), "cut at a line start");
        cap_log(&log).unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap(), kept, "a small log stays");
        fs::remove_file(&log).unwrap();
    }
}
