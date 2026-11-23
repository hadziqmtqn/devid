//! The one-time root setup (D-002, D-013). Run through `sudo devid helper apply …`; every other
//! part of Devid runs as the owner. This file holds what both systems share: the plan, its
//! validation, and `apply`. Which files and commands each system needs is in platform/: on macOS
//! /etc/resolver, a pf anchor and the System keychain; on Linux systemd-resolved, an nftables table
//! and the system CA store (D-057). `uninstall` reverses each. The CA is name-constrained to
//! `.<tld>` (see ca.rs), so trusting it cannot vouch for any real domain.

use crate::sites;
use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

/// Everything the helper needs, passed explicitly on the sudo command line and validated as root.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub tld: String,
    pub dns_port: u16,
    pub http_port: u16,
    pub https_port: u16,
    pub ca_cert: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Write a file; if `backup` and another file is already there, keep it as `<path>.devid-backup`.
    Write { path: PathBuf, content: String, backup: bool },
    /// Copy a file the owner has (the CA certificate), readable by everyone.
    Copy { from: PathBuf, to: PathBuf },
    /// Remove a file, restoring `<path>.devid-backup` if present.
    Remove { path: PathBuf, restore_backup: bool },
    /// Run a command; `ok_to_fail` for best-effort cleanup steps.
    Run { program: String, args: Vec<String>, ok_to_fail: bool },
}

impl Action {
    pub fn describe(&self) -> String {
        match self {
            Action::Write { path, backup, .. } => {
                format!("write {}{}", path.display(), if *backup { " (backing up an existing file)" } else { "" })
            }
            Action::Copy { from, to } => format!("copy {} to {}", from.display(), to.display()),
            Action::Remove { path, restore_backup } => {
                format!(
                    "remove {}{}",
                    path.display(),
                    if *restore_backup { " (restoring a backup if any)" } else { "" }
                )
            }
            Action::Run { program, args, .. } => {
                format!("run {program}{}", args.iter().map(|a| format!(" {a}")).collect::<String>())
            }
        }
    }
}

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

/// Reject anything odd before root acts on it.
pub fn validate(plan: &Plan) -> io::Result<()> {
    if !sites::valid_name(&plan.tld) {
        return Err(err(format!("invalid TLD {:?}", plan.tld)));
    }
    for (name, port) in [("dns", plan.dns_port), ("http", plan.http_port), ("https", plan.https_port)] {
        // Targets must be unprivileged: the point is that nginx and the daemon never need root.
        if port < 1024 {
            return Err(err(format!("{name} port {port} must be 1024 or higher")));
        }
    }
    let ca = &plan.ca_cert;
    if !ca.is_absolute() || !ca.ends_with("ca/devid-ca.crt") {
        return Err(err(format!("{} is not a Devid CA certificate path", ca.display())));
    }
    let meta = fs::symlink_metadata(ca).map_err(|e| err(format!("{}: {e}", ca.display())))?;
    if !meta.is_file() {
        return Err(err(format!("{} must be a regular file, not a symlink", ca.display())));
    }
    Ok(())
}

/// The root step's actions for this OS (platform/).
pub fn install_actions(plan: &Plan) -> Vec<Action> {
    crate::platform::install_actions(plan)
}

/// What `install_actions` undoes, in reverse.
pub fn uninstall_actions(plan: &Plan) -> Vec<Action> {
    crate::platform::uninstall_actions(plan)
}

fn backup_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.devid-backup", path.display()))
}

/// Execute actions in order; stops at the first failure that is not `ok_to_fail`.
pub fn apply(actions: &[Action], mut log: impl FnMut(&str)) -> io::Result<()> {
    for action in actions {
        log(&action.describe());
        match action {
            Action::Write { path, content, backup } => {
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                let existing = fs::read_to_string(path).ok();
                let backup_file = backup_path(path);
                // Back up a foreign file once; never overwrite an earlier backup with Devid's own file.
                if *backup && existing.as_ref().is_some_and(|e| e != content) && !backup_file.exists() {
                    fs::copy(path, &backup_file)?;
                }
                fs::write(path, content)?;
            }
            Action::Copy { from, to } => {
                if let Some(dir) = to.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::copy(from, to)?;
                fs::set_permissions(to, fs::Permissions::from_mode(0o644))?;
            }
            Action::Remove { path, restore_backup } => {
                let backup_file = backup_path(path);
                if *restore_backup && backup_file.exists() {
                    fs::rename(&backup_file, path)?;
                } else if path.exists() {
                    fs::remove_file(path)?;
                }
            }
            Action::Run { program, args, ok_to_fail } => {
                let status = Command::new(program).args(args).status()?;
                if !status.success() && !ok_to_fail {
                    return Err(err(format!("`{}` failed ({status})", action.describe())));
                }
            }
        }
    }
    Ok(())
}

/// Read-only checks of the root step, usable without root.
pub fn status(plan: &Plan) -> Vec<(&'static str, bool)> {
    crate::platform::system_status(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(dir: &Path) -> Plan {
        Plan {
            tld: "test".into(),
            dns_port: 53535,
            http_port: 8088,
            https_port: 8443,
            ca_cert: dir.join("ca/devid-ca.crt"),
        }
    }

    #[test]
    fn validation() {
        let dir = std::env::temp_dir().join(format!("devid-helper-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("ca")).unwrap();
        fs::write(dir.join("ca/devid-ca.crt"), "x").unwrap();
        let ok = plan(&dir);
        assert!(validate(&ok).is_ok());
        assert!(validate(&Plan { tld: "te st".into(), ..ok.clone() }).is_err());
        assert!(validate(&Plan { http_port: 80, ..ok.clone() }).is_err(), "targets must be unprivileged");
        assert!(validate(&Plan { ca_cert: "/etc/passwd".into(), ..ok.clone() }).is_err());
        std::os::unix::fs::symlink("/etc/hosts", dir.join("ca/link.crt")).unwrap();
        fs::create_dir_all(dir.join("x/ca")).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", dir.join("x/ca/devid-ca.crt")).unwrap();
        assert!(validate(&Plan { ca_cert: dir.join("x/ca/devid-ca.crt"), ..ok }).is_err(), "no symlinks");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_backs_up_and_restores_foreign_files() {
        let dir = std::env::temp_dir().join(format!("devid-helper-apply-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("resolver-test");
        fs::write(&target, "nameserver 127.0.0.1\n").unwrap(); // e.g. Valet's

        let write = Action::Write { path: target.clone(), content: "devid\n".into(), backup: true };
        apply(std::slice::from_ref(&write), |_| {}).unwrap();
        apply(&[write], |_| {}).unwrap(); // idempotent: the second run must not back up Devid's own file
        assert_eq!(fs::read_to_string(&target).unwrap(), "devid\n");
        assert_eq!(fs::read_to_string(backup_path(&target)).unwrap(), "nameserver 127.0.0.1\n");

        apply(&[Action::Remove { path: target.clone(), restore_backup: true }], |_| {}).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "nameserver 127.0.0.1\n", "original restored");
        assert!(!backup_path(&target).exists());

        let copy = dir.join("ca/devid-ca.crt");
        apply(&[Action::Copy { from: target.clone(), to: copy.clone() }], |_| {}).unwrap();
        assert_eq!(fs::read_to_string(&copy).unwrap(), "nameserver 127.0.0.1\n");
        assert_eq!(fs::metadata(&copy).unwrap().permissions().mode() & 0o777, 0o644, "readable by all");

        let failing = Action::Run { program: "/usr/bin/false".into(), args: vec![], ok_to_fail: false };
        assert!(apply(&[failing], |_| {}).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
