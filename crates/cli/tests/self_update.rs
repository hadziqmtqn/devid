//! Phase 5 R4: `devid self-update` and install.sh against a local `file://` release (fake devid
//! binary, real checksums): self-update reads GitHub's Latest (`latest/`, D-038), install.sh the
//! rolling `devid-latest/`.

mod common;

use common::*;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

const ASSET: &str = if cfg!(target_arch = "aarch64") { "devid-darwin-arm64" } else { "devid-darwin-x86_64" };

/// Publish a fake devid `version` as `<base>/latest` and `<base>/devid-latest`; its `setup`
/// writes a marker file.
fn publish(base: &Path, version: &str, tamper: bool) {
    for release in ["latest", "devid-latest"] {
        publish_to(&base.join(release), version, tamper);
    }
}

fn publish_to(rel: &Path, version: &str, tamper: bool) {
    fs::create_dir_all(rel).unwrap();
    let bin = rel.join(ASSET);
    fs::write(
        &bin,
        format!(
            "#!/bin/sh\ncase \"$1\" in\n  --version) echo \"devid {version}\" ;;\n  setup) [ \"$2\" = --no-sudo ] && [ -n \"$FAKE_NEEDS_SUDO\" ] && exit 3\n    echo \"setup by {version} $2\" > \"$DEVID_HOME/setup-marker\" ;;\nesac\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(rel.join("devid-version.txt"), format!("{version}\n")).unwrap();
    let sum = String::from_utf8(Command::new("shasum").args(["-a", "256"]).arg(&bin).output().unwrap().stdout).unwrap();
    let sum = sum.split_whitespace().next().unwrap();
    let sum = if tamper { "0".repeat(64) } else { sum.to_string() };
    fs::write(rel.join("checksums.txt"), format!("{sum}  {ASSET}\n")).unwrap();
}

fn run(home: &Path, base: &Path, args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env("DEVID_HOME", home)
        .env("DEVID_RELEASE_BASE", format!("file://{}", base.display()))
        .output()
        .unwrap()
}

#[test]
fn self_update_checks_verifies_and_hands_over() {
    let root = temp_root("devid-self-update-it");
    let (home, base) = (root.join("home"), root.join("release"));
    fs::create_dir_all(&home).unwrap();
    let stdout = |o: &std::process::Output| String::from_utf8_lossy(&o.stdout).into_owned();

    publish(&base, devid_daemon::VERSION, false);
    assert!(stdout(&run(&home, &base, &["self-update", "--check"])).contains("is the newest version"));

    publish(&base, "9.9.9", false);
    let o = run(&home, &base, &["self-update", "--check"]);
    assert!(stdout(&o).contains("devid 9.9.9 is available"), "{}", stdout(&o));
    assert!(!home.join("setup-marker").exists(), "--check installs nothing");

    let o = run(&home, &base, &["self-update"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(
        fs::read_to_string(home.join("setup-marker")).unwrap().trim(),
        "setup by 9.9.9",
        "the new binary ran setup"
    );

    // From the desktop app: no password prompt. The new setup gets --no-sudo, and a sudo step it
    // had to skip (exit 3) is reported as such.
    let o = run(&home, &base, &["self-update", "--no-sudo"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(fs::read_to_string(home.join("setup-marker")).unwrap().trim(), "setup by 9.9.9 --no-sudo");
    let o = Command::new(BIN)
        .args(["self-update", "--no-sudo"])
        .env("DEVID_HOME", &home)
        .env("DEVID_RELEASE_BASE", format!("file://{}", base.display()))
        .env("FAKE_NEEDS_SUDO", "1")
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(
        stderr(&o).contains("devid 9.9.9 is installed, but its system setup")
            && stderr(&o).contains("needs your password"),
        "{}",
        stderr(&o)
    );
    assert!(!home.join(format!("downloads/self-update-{ASSET}")).exists(), "download cleaned up");

    fs::remove_file(home.join("setup-marker")).unwrap();
    publish(&base, "9.9.10", true);
    let o = run(&home, &base, &["self-update"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("checksum mismatch"), "{}", stderr(&o));
    assert!(!home.join("setup-marker").exists(), "a tampered binary never runs");

    fs::remove_dir_all(&root).unwrap();
}

/// install.sh detects macOS via uname, so it only runs there.
#[cfg(target_os = "macos")]
#[test]
fn install_sh_downloads_verifies_and_installs() {
    let root = temp_root("devid-install-sh-it");
    let (home, base) = (root.join("home"), root.join("release"));
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh");
    let sh = |extra: &[(&str, &str)]| {
        Command::new("sh")
            .arg(&script)
            .env("DEVID_HOME", &home)
            .env("DEVID_RELEASE_BASE", format!("file://{}", base.display()))
            .env("DEVID_NO_SETUP", "1")
            .envs(extra.iter().copied())
            .output()
            .unwrap()
    };

    publish(&base, "9.9.9", false);
    let o = sh(&[]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("(devid 9.9.9)"));
    assert!(home.join("bin/devid").is_file());

    publish(&base, "9.9.10", true);
    let o = sh(&[]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("checksum mismatch"));
    let o = sh(&[("DEVID_VERSION", "1.2.3")]);
    assert!(String::from_utf8_lossy(&o.stderr).contains("no devid-darwin-"), "missing version tag is reported");

    fs::remove_dir_all(&root).unwrap();
}
