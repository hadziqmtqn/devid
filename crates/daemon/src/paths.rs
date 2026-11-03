//! Locations under the Devid home (`~/.devid`, overridable with `DEVID_HOME` for tests).

use std::path::PathBuf;

/// The user's home: `$HOME`, or the passwd entry when a parent cleared the environment
/// (`php artisan serve` starts `php -S` without HOME).
pub fn user_home() -> PathBuf {
    if let Some(h) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(h);
    }
    passwd_home()
}

fn passwd_home() -> PathBuf {
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut out: *mut libc::passwd = std::ptr::null_mut();
    let mut buf = vec![0 as libc::c_char; 4096];
    let rc = unsafe { libc::getpwuid_r(libc::getuid(), &mut pw, buf.as_mut_ptr(), buf.len(), &mut out) };
    assert!(rc == 0 && !out.is_null(), "HOME is not set and the user has no passwd entry");
    let dir = unsafe { std::ffi::CStr::from_ptr(pw.pw_dir) };
    PathBuf::from(<std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(dir.to_bytes()))
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let home = match std::env::var_os("DEVID_HOME") {
            Some(h) if !h.is_empty() => PathBuf::from(h),
            _ => user_home().join(".devid"),
        };
        Self { home }
    }

    pub fn run_dir(&self) -> PathBuf {
        self.home.join("run")
    }

    pub fn socket(&self) -> PathBuf {
        self.run_dir().join("devid.sock")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.home.join("log")
    }

    pub fn config(&self) -> PathBuf {
        self.home.join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn passwd_home_matches_home() {
        assert_eq!(super::passwd_home(), std::path::PathBuf::from(std::env::var_os("HOME").unwrap()));
    }
}
