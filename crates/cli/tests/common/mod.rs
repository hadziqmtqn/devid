//! Helpers shared by the integration tests. Fake php/php-fpm/nginx scripts stand in for the
//! real builds, so the suite runs anywhere (including CI) in a temporary DEVID_HOME.
#![allow(dead_code)]

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread::sleep,
    time::{Duration, Instant},
};

pub const BIN: &str = env!("CARGO_BIN_EXE_devid");

pub fn devid(home: &Path, args: &[&str]) -> Output {
    Command::new(BIN).args(args).env("DEVID_HOME", home).output().unwrap()
}

pub fn json(home: &Path, args: &[&str]) -> Value {
    let out = devid(home, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

pub fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        sleep(Duration::from_millis(50));
    }
}

// Like php-fpm, the fakes create the `listen` socket path from their config (arg 3) and remove it on exit.
pub const IDLE_FPM: &str = concat!(
    "#!/bin/sh\n",
    "sock=$(sed -n 's/^listen = //p' \"$3\")\ntouch \"$sock\"\n",
    "trap 'rm -f \"$sock\"; exit 0' QUIT TERM\nwhile :; do sleep 0.1; done\n"
);
/// Like real php-fpm during startup: signals arriving in the first 1.5 s are lost.
pub const SLOW_START_FPM: &str = concat!(
    "#!/bin/sh\n",
    "trap '' QUIT TERM\nsleep 1.5\n",
    "sock=$(sed -n 's/^listen = //p' \"$3\")\ntouch \"$sock\"\n",
    "trap 'rm -f \"$sock\"; exit 0' QUIT TERM\nwhile :; do sleep 0.1; done\n"
);

/// A build folder whose `php` reports `version` and whose `php-fpm` runs `fpm`.
pub fn fake_build(root: &Path, version: &str) -> PathBuf {
    fake_build_with(root, version, IDLE_FPM)
}

pub fn fake_build_with(root: &Path, version: &str, fpm: &str) -> PathBuf {
    let dir = root.join(format!("build-{version}"));
    fs::create_dir_all(dir.join("modules")).unwrap();
    let scripts = [("php", format!("#!/bin/sh\necho {version}\n")), ("php-fpm", fpm.to_string())];
    for (name, body) in scripts {
        fs::write(dir.join(name), body).unwrap();
        fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(dir.join("modules/xdebug.so"), b"").unwrap();
    dir
}

/// (status, pid) of one version in `devid php list --json`-style output.
pub fn proc_of(status: &Value, version: &str) -> (String, Option<u64>) {
    let p = status["php"].as_array().unwrap().iter().find(|p| p["version"] == version).expect("version listed");
    (p["status"].as_str().unwrap().to_string(), p["pid"].as_u64())
}

pub fn alive(pid: u64) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success()
}

/// A fake nginx: `-v` prints a version, `-t` rejects configs that mention `reject.test`,
/// and a running instance appends "reload" to `<home>/reloads` on SIGHUP.
pub fn fake_nginx(root: &Path, home: &Path) -> PathBuf {
    let path = root.join("fake-nginx");
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *-v*) echo 'nginx version: nginx/fake' >&2; exit 0 ;;\n\
           *-t*) conf=$(echo \"$*\" | sed 's/.*-c //; s/ .*//'); \
                 if grep -q reject.test \"$conf\"; then echo 'emerg: fake rejection' >&2; exit 1; fi; exit 0 ;;\n\
         esac\n\
         trap 'echo reload >> {reloads}' HUP\n\
         trap 'exit 0' QUIT TERM\n\
         while :; do sleep 0.1; done\n",
        reloads = home.join("reloads").display()
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// A running daemon that is always stopped when dropped, even if the test panics; otherwise its
/// orphaned children keep cargo's output pipe open and the whole run hangs.
pub struct Daemon {
    home: PathBuf,
    child: std::process::Child,
}

/// Ports for test daemons: never the real ones (8088/8443/53535), so a Devid running on this Mac
/// (or its pf redirect) cannot interfere.
pub const TEST_HTTP_PORT: u16 = 18088;
pub const TEST_HTTPS_PORT: u16 = 18443;

pub fn free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

pub fn start_daemon(home: &Path) -> Daemon {
    start_daemon_env(home, &[])
}

/// Like `start_daemon`, with extra environment for the daemon (e.g. DEVID_MANIFEST_URL).
pub fn start_daemon_env(home: &Path, env: &[(&str, &str)]) -> Daemon {
    let config = home.join("config.toml");
    if !config.exists() {
        fs::create_dir_all(home).unwrap();
        fs::write(
            &config,
            format!("http_port = {TEST_HTTP_PORT}\nhttps_port = {TEST_HTTPS_PORT}\ndns_port = {}\n", free_udp_port()),
        )
        .unwrap();
    }
    let child =
        Command::new(BIN).args(["daemon", "run"]).env("DEVID_HOME", home).envs(env.iter().copied()).spawn().unwrap();
    let daemon = Daemon { home: home.to_path_buf(), child };
    wait_for("daemon", || devid(home, &["status"]).status.success());
    daemon
}

impl Daemon {
    /// Stop through the CLI and assert it worked.
    pub fn stop(mut self) {
        assert!(devid(&self.home, &["daemon", "stop"]).status.success());
        wait_for("daemon exit", || self.child.try_wait().unwrap().is_some());
    }

    pub fn exited(&mut self) -> bool {
        self.child.try_wait().unwrap().is_some()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.exited() {
            return;
        }
        let _ = devid(&self.home, &["daemon", "stop"]);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !self.exited() && Instant::now() < deadline {
            sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
    }
}

/// A fresh temp folder with symlinks resolved (/var -> /private/var on macOS), matching the
/// canonical paths the CLI sends.
pub fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

/// A fake Caddy: `version` says v2, `validate` rejects configs mentioning `reject.test`, and a
/// running instance exits on SIGTERM (Caddy reloads by restart, so no reload signal).
pub fn fake_caddy(root: &Path) -> PathBuf {
    let path = root.join("fake-caddy");
    let script = "#!/bin/sh\n\
         case \"$1\" in\n\
           version) echo 'v2.99.0 fake'; exit 0 ;;\n\
           validate) grep -q reject.test \"$5\" && { echo 'Error: fake rejection' >&2; exit 1; }; exit 0 ;;\n\
         esac\n\
         trap 'exit 0' TERM\n\
         while :; do sleep 0.1; done\n";
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// A fake httpd: `-v` says Apache/2.4, `-t` accepts, a running instance appends "reload" to
/// `<home>/apache-reloads` on SIGUSR1 (graceful restart) and exits on SIGTERM.
pub fn fake_httpd(root: &Path, home: &Path) -> PathBuf {
    let path = root.join("fake-httpd");
    let script = format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
           -v) echo 'Server version: Apache/2.4.99 (Unix)'; exit 0 ;;\n\
           -t) exit 0 ;;\n\
         esac\n\
         trap 'echo reload >> {reloads}' USR1\n\
         trap 'exit 0' TERM\n\
         while :; do sleep 0.1; done\n",
        reloads = home.join("apache-reloads").display()
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}
