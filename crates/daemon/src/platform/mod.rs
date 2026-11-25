//! Everything that differs between macOS and Linux (D-013, D-057), behind one set of functions:
//! the root step's actions and status, the login service, the shell startup files, the arch key
//! of the runtime manifest, on-demand's open-connection check, the browser trust step that runs as
//! the user, and the programs that open a URL or copy text. Nothing else branches on the OS.
//!
//! Both implementations compile on both systems, so each one's generated files and action lists
//! are tested on the owner's Mac and on CI alike; only the calls into the OS are behind `cfg`.

pub mod linux;
pub mod macos;

#[cfg(not(target_os = "macos"))]
use linux as os;
#[cfg(target_os = "macos")]
use macos as os;

pub use os::{
    COPY_COMMANDS, DESKTOP_APP, DEVID_ASSET, NAME, OPEN_COMMAND, REDIRECT_FIX, SERVICE, arch_key, browser_trust,
    browser_trust_needed, browser_untrust, established, install_actions, load_service, os_checks, prerequisites,
    rc_files_for, redirect_installed, remove_service, service_loaded, service_path, stop_service, system_status,
    terminal_command, unavailable_server, uninstall_actions,
};

/// Seconds the login service gives the daemon to stop: a database may take a minute to stop
/// cleanly (`db::STOP_TIMEOUT`, D-061), and the rest stops alongside it.
pub const EXIT_TIMEOUT: u64 = crate::db::STOP_TIMEOUT.as_secs() + 15;

/// A health check for `devid doctor`: whether it holds, what it checks, and the fix.
pub type Check = (bool, String, String);

#[cfg(test)]
pub(crate) mod tests {
    use crate::helper::Action;
    use std::path::PathBuf;

    /// Uninstall removes every file install writes or copies, and puts back every file it links.
    pub fn uninstall_removes_every_file(install: &[Action], uninstall: &[Action]) {
        let written = install.iter().filter_map(|a| match a {
            Action::Write { path, .. } | Action::Link { path, .. } => Some(path),
            Action::Copy { to, .. } => Some(to),
            _ => None,
        });
        let removed: Vec<&PathBuf> = uninstall
            .iter()
            .filter_map(|a| match a {
                Action::Remove { path, .. } | Action::Restore { path } => Some(path),
                _ => None,
            })
            .collect();
        for path in written {
            assert!(removed.contains(&path), "{} is cleaned up by uninstall", path.display());
        }
    }

    /// `established` sees a connection this test process holds, and only while it is open.
    pub fn sees_an_open_connection(established: fn(i32, u16) -> bool) {
        use std::net::{TcpListener, TcpStream};
        // SAFETY: getpgrp(2) has no preconditions.
        let group = unsafe { libc::getpgrp() };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!established(group, port), "listening only");
        let client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let (server, _) = listener.accept().unwrap();
        assert!(established(group, port), "the accepted side has local port {port}");
        let other = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(!established(group, other));
        drop((client, server));
        assert!(!established(group, port), "closed");
        assert!(!established(i32::MAX, port), "no such group");
    }
}
