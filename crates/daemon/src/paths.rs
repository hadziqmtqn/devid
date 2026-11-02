//! Locations under the Devid home (`~/.devid`, overridable with `DEVID_HOME` for tests).

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let home = match std::env::var_os("DEVID_HOME") {
            Some(h) if !h.is_empty() => PathBuf::from(h),
            _ => PathBuf::from(std::env::var_os("HOME").expect("HOME is not set")).join(".devid"),
        };
        Self { home }
    }

    pub fn run_dir(&self) -> PathBuf {
        self.home.join("run")
    }

    pub fn socket(&self) -> PathBuf {
        self.run_dir().join("devid.sock")
    }

    pub fn config(&self) -> PathBuf {
        self.home.join("config.toml")
    }
}
