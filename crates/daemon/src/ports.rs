//! Local ports for sites (D-037, Phase 8 A3): whether a port is taken, by whom, and a free one.

use std::{
    net::{SocketAddr, TcpStream},
    process::Command,
    time::Duration,
};

/// Whether something accepts connections on `port` over IPv4 or IPv6 loopback. A bind test is not
/// enough: on macOS, binding 127.0.0.1:<port> succeeds next to a listener on *:<port> (checked in
/// A3), and the web server would then take that program's traffic, a tunnel's included.
pub fn in_use(port: u16) -> bool {
    ["127.0.0.1", "[::1]"].iter().any(|ip| {
        let addr: SocketAddr = format!("{ip}:{port}").parse().expect("a valid loopback address");
        TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
    })
}

/// "php (pid 38571)" for the program listening on `port`, from lsof; None if lsof cannot tell.
pub fn holder(port: u16) -> Option<String> {
    let out = Command::new("lsof").args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fpc"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let pid = text.lines().find_map(|l| l.strip_prefix('p'))?;
    let command = text.lines().find_map(|l| l.strip_prefix('c')).unwrap_or("?");
    Some(format!("{command} (pid {pid})"))
}

/// The first port from 8000 to 8999 that `taken` allows and nothing listens on.
pub fn first_free(taken: impl Fn(u16) -> bool) -> Option<u16> {
    (8000..9000).find(|p| !taken(*p) && !in_use(*p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn a_wildcard_listener_counts_as_in_use() {
        let listener = TcpListener::bind("0.0.0.0:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(in_use(port), "a listener on * is found through loopback");
        if let Some(who) = holder(port) {
            assert!(who.contains(&format!("pid {}", std::process::id())), "{who}");
        }
        drop(listener);
        assert!(!in_use(port));
        assert_eq!(first_free(|p| p < 8005), (8005..9000).find(|p| !in_use(*p)));
    }
}
