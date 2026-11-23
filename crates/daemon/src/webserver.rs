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
    if !SERVERS.contains(&name) {
        return Err(format!("unknown web server {name:?}; choose one of {}", SERVERS.join(", ")));
    }
    crate::platform::unavailable_server(name).map_or(Ok(()), Err)
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
    probe_env(program, args, &[])
}

/// `probe` with extra environment variables. Output stays small (it is read after exit).
pub(crate) fn probe_env(
    program: &Path,
    args: &[&str],
    env: &[(String, std::path::PathBuf)],
) -> io::Result<std::process::Output> {
    use std::{
        process::Stdio,
        thread::sleep,
        time::{Duration, Instant},
    };
    let mut child = std::process::Command::new(program)
        .args(args)
        .envs(env.iter().map(|(k, v)| (k, v)))
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

/// Version of an installed nginx ("1.30.6") or Caddy ("2.11.7"); None for Apache (the OS owns it).
pub fn installed_version(paths: &Paths, name: &str) -> Option<String> {
    let (bin, args): (std::path::PathBuf, &[&str]) = match name {
        "nginx" => (nginx::bin(paths), &["-v"]),
        "caddy" => (caddy::bin(paths), &["version"]),
        _ => return None,
    };
    let out = probe(&bin, args).ok()?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let raw = match name {
        "nginx" => text.split("nginx/").nth(1)?,
        _ => text.trim_start().strip_prefix('v')?,
    };
    Some(raw.split_whitespace().next()?.to_string())
}

/// The catch-all reply of every Devid web server to a host that is not one of its sites.
pub const NO_SITE: &str = "No Devid site for this host.";

/// Whether Devid's web server answers on 127.0.0.1:`port`: its catch-all reply (NO_SITE) to an
/// unknown host, so another server on that port does not count. Port 80 reaches the web server only
/// through pf's redirect, which another app can switch off (D-047).
pub fn answers_on(port: u16) -> bool {
    use std::io::{Read, Write};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500)) else {
        return false;
    };
    let timeout = Some(std::time::Duration::from_secs(1));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    if stream.write_all(b"GET / HTTP/1.0\r\nHost: devid-probe.invalid\r\nConnection: close\r\n\r\n").is_err() {
        return false;
    }
    let mut reply = Vec::new();
    let _ = stream.take(4096).read_to_end(&mut reply);
    String::from_utf8_lossy(&reply).contains(NO_SITE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    /// A one-request HTTP server on a free port that replies 404 with `body`.
    fn serve(body: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = s.read(&mut [0; 1024]);
                let _ = write!(s, "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            }
        });
        port
    }

    #[test]
    fn answers_on_needs_devids_own_reply() {
        assert!(answers_on(serve("No Devid site for this host.\n")));
        assert!(!answers_on(serve("404 page not found\n")), "another server on the port does not count");
        let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(!answers_on(closed), "nothing listening");
    }
}
