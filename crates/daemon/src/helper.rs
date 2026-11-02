//! The one-time root setup (D-002, D-013: macOS-only code lives here). Run through
//! `sudo devid helper apply …`; every other part of Devid runs as the owner.
//!
//! It touches only these, and `uninstall` reverses each:
//! - `/etc/resolver/<tld>`: send `.<tld>` lookups to the daemon's DNS port (an existing file,
//!   e.g. Valet's, is backed up first and restored on uninstall);
//! - a pf anchor in `com.apple/devid` (no edit to /etc/pf.conf) redirecting 127.0.0.1:80/443 to
//!   the high ports nginx listens on, plus a LaunchDaemon that reloads it at boot;
//! - trust for the Devid CA (SSL only) in the System keychain. The CA is name-constrained to
//!   `.<tld>` (see ca.rs), so trusting it cannot vouch for any real domain.

use crate::sites;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

pub const PF_ANCHOR: &str = "com.apple/devid";
pub const PF_FILE: &str = "/etc/pf.anchors/dev.devid";
pub const PF_PLIST: &str = "/Library/LaunchDaemons/dev.devid.pf.plist";
pub const PF_LABEL: &str = "dev.devid.pf";
const SYSTEM_KEYCHAIN: &str = "/Library/Keychains/System.keychain";
const CA_NAME: &str = "Devid Local CA";

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
            Action::Remove { path, restore_backup } => {
                format!(
                    "remove {}{}",
                    path.display(),
                    if *restore_backup { " (restoring a backup if any)" } else { "" }
                )
            }
            Action::Run { program, args, .. } => format!("run {program} {}", args.join(" ")),
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

pub fn resolver_path(tld: &str) -> PathBuf {
    PathBuf::from(format!("/etc/resolver/{tld}"))
}

pub fn resolver_content(plan: &Plan) -> String {
    format!("# Devid: send .{} lookups to the Devid daemon\nnameserver 127.0.0.1\nport {}\n", plan.tld, plan.dns_port)
}

pub fn pf_rules(plan: &Plan) -> String {
    format!(
        "# Devid: local-only redirect of the standard web ports to nginx's high ports\n\
         rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 80 -> 127.0.0.1 port {}\n\
         rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 443 -> 127.0.0.1 port {}\n",
        plan.http_port, plan.https_port
    )
}

fn pf_load_command() -> String {
    // -E enables pf (it is off by default on macOS) and takes a reference so others' -X cannot disable it.
    format!("/sbin/pfctl -E >/dev/null 2>&1; /sbin/pfctl -a {PF_ANCHOR} -f {PF_FILE}")
}

fn pf_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PF_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/sh</string>
        <string>-c</string>
        <string>{cmd}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        cmd = pf_load_command()
    )
}

fn run(program: &str, args: &[&str], ok_to_fail: bool) -> Action {
    Action::Run { program: program.into(), args: args.iter().map(|s| s.to_string()).collect(), ok_to_fail }
}

pub fn install_actions(plan: &Plan) -> Vec<Action> {
    let ca = plan.ca_cert.display().to_string();
    vec![
        Action::Write { path: resolver_path(&plan.tld), content: resolver_content(plan), backup: true },
        Action::Write { path: PF_FILE.into(), content: pf_rules(plan), backup: false },
        Action::Write { path: PF_PLIST.into(), content: pf_plist(), backup: false },
        run("/bin/sh", &["-c", &pf_load_command()], false),
        // Replace any earlier Devid CA in the System keychain, then trust the current one for SSL.
        run("/usr/bin/security", &["delete-certificate", "-c", CA_NAME, SYSTEM_KEYCHAIN], true),
        run(
            "/usr/bin/security",
            &["add-trusted-cert", "-d", "-r", "trustRoot", "-p", "ssl", "-k", SYSTEM_KEYCHAIN, &ca],
            false,
        ),
        run("/usr/bin/dscacheutil", &["-flushcache"], true),
        run("/usr/bin/killall", &["-HUP", "mDNSResponder"], true),
    ]
}

pub fn uninstall_actions(plan: &Plan) -> Vec<Action> {
    let ca = plan.ca_cert.display().to_string();
    vec![
        run("/sbin/pfctl", &["-a", PF_ANCHOR, "-F", "all"], true),
        Action::Remove { path: PF_PLIST.into(), restore_backup: false },
        Action::Remove { path: PF_FILE.into(), restore_backup: false },
        Action::Remove { path: resolver_path(&plan.tld), restore_backup: true },
        run("/usr/bin/security", &["remove-trusted-cert", "-d", &ca], true),
        run("/usr/bin/security", &["delete-certificate", "-c", CA_NAME, SYSTEM_KEYCHAIN], true),
        run("/usr/bin/dscacheutil", &["-flushcache"], true),
        run("/usr/bin/killall", &["-HUP", "mDNSResponder"], true),
    ]
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

/// Read-only checks, usable without root.
pub fn status(plan: &Plan) -> Vec<(&'static str, bool)> {
    let resolver = fs::read_to_string(resolver_path(&plan.tld)).is_ok_and(|c| c == resolver_content(plan));
    let pf = fs::read_to_string(PF_FILE).is_ok_and(|c| c == pf_rules(plan)) && Path::new(PF_PLIST).exists();
    let trusted = Command::new("/usr/bin/security")
        .args(["verify-cert", "-q", "-L", "-c"])
        .arg(&plan.ca_cert)
        .args(["-l", "-p", "ssl"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    vec![("resolver", resolver), ("ports 80/443", pf), ("CA trusted", trusted)]
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
    fn generated_files() {
        let p = plan(Path::new("/Users/me/.devid"));
        assert_eq!(
            resolver_content(&p),
            "# Devid: send .test lookups to the Devid daemon\nnameserver 127.0.0.1\nport 53535\n"
        );
        let rules = pf_rules(&p);
        assert!(rules.contains("to 127.0.0.1 port 80 -> 127.0.0.1 port 8088"));
        assert!(rules.contains("to 127.0.0.1 port 443 -> 127.0.0.1 port 8443"));
        assert!(rules.contains("on lo0"), "local traffic only");
        assert!(pf_plist().contains("<string>dev.devid.pf</string>"));
        // Uninstall reverses every file install writes.
        let written: Vec<PathBuf> = install_actions(&p)
            .into_iter()
            .filter_map(|a| if let Action::Write { path, .. } = a { Some(path) } else { None })
            .collect();
        let removed: Vec<PathBuf> = uninstall_actions(&p)
            .into_iter()
            .filter_map(|a| if let Action::Remove { path, .. } = a { Some(path) } else { None })
            .collect();
        for path in written {
            assert!(removed.contains(&path), "{} is cleaned up by uninstall", path.display());
        }
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

        let failing = Action::Run { program: "/usr/bin/false".into(), args: vec![], ok_to_fail: false };
        assert!(apply(&[failing], |_| {}).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
