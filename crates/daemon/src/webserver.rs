//! The active web server (D-005: one at a time; D-015: nginx by default), behind one interface.

use crate::{apache, caddy, config::Config, nginx, paths::Paths, sites::Site, supervisor::Spec};
use std::{io, path::Path};

pub const SERVERS: [&str; 3] = ["nginx", "caddy", "apache"];

/// How a running server picks up a new config.
pub enum Reload {
    Signal(libc::c_int),
    /// Caddy has no signal reload while its admin API is off.
    Restart,
}

pub fn check(name: &str) -> Result<(), String> {
    if SERVERS.contains(&name) {
        Ok(())
    } else {
        Err(format!("unknown web server {name:?}; choose one of {}", SERVERS.join(", ")))
    }
}

pub fn installed(paths: &Paths, name: &str) -> bool {
    match name {
        "nginx" => nginx::installed(paths),
        "caddy" => caddy::installed(paths),
        "apache" => apache::installed(paths),
        _ => false,
    }
}

/// `from` is required except for Apache, which defaults to the macOS system httpd.
pub fn install(paths: &Paths, name: &str, from: Option<&Path>) -> io::Result<()> {
    let need = |from: Option<&Path>| {
        from.map(Path::to_path_buf).ok_or_else(|| io::Error::other(format!("{name} needs --from <binary>")))
    };
    match name {
        "nginx" => nginx::install(paths, &need(from)?),
        "caddy" => caddy::install(paths, &need(from)?),
        "apache" => apache::install(paths, from),
        _ => Err(io::Error::other(format!("unknown web server {name:?}"))),
    }
}

/// Render and validate the config for `name`; the old config stays if validation fails.
/// Returns whether it changed.
pub fn write_config(paths: &Paths, name: &str, config: &Config, sites: &[Site]) -> io::Result<bool> {
    match name {
        "nginx" => nginx::write_checked(paths, &nginx::render(paths, config, sites)),
        "caddy" => caddy::write_checked(paths, &caddy::render(paths, config, sites)),
        "apache" => apache::write_checked(paths, &apache::render(paths, config, sites)),
        _ => Err(io::Error::other(format!("unknown web server {name:?}"))),
    }
}

pub fn spec(paths: &Paths, name: &str) -> Spec {
    match name {
        "caddy" => caddy::spec(paths),
        "apache" => apache::spec(paths),
        _ => nginx::spec(paths),
    }
}

pub fn reload(name: &str) -> Reload {
    match name {
        "caddy" => Reload::Restart,
        // Graceful restart: finish in-flight requests, then load the new config.
        "apache" => Reload::Signal(libc::SIGUSR1),
        _ => Reload::Signal(libc::SIGHUP),
    }
}

/// Run `program args` for a version probe, killing it after 5 s: a wrong binary passed to
/// `server install` (e.g. nginx as Caddy, which ignores `version` and starts serving) must not
/// block the daemon forever.
pub(crate) fn probe(program: &Path, args: &[&str]) -> io::Result<std::process::Output> {
    use std::{
        process::Stdio,
        thread::sleep,
        time::{Duration, Instant},
    };
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| io::Error::other(format!("{}: {e}", program.display())))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait()?.is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(format!("{} {} did not finish in 5 s", program.display(), args.join(" "))));
        }
        sleep(Duration::from_millis(50));
    }
    child.wait_with_output()
}
