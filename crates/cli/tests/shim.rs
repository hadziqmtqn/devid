//! M6: `php`/`composer` shims pick the PHP version per folder; setup/uninstall dry runs change nothing.

mod common;

use common::*;
use std::{fs, os::unix::fs::symlink, path::Path, process::Command};

/// Run the shim `name` (a symlink to devid) in `cwd`.
fn shim(bin_dir: &Path, name: &str, home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin_dir.join(name)).args(args).current_dir(cwd).env("DEVID_HOME", home).output().unwrap()
}

#[test]
fn php_shim_follows_the_site_version() {
    let root = temp_root("devid-shim-it");
    let home = root.join("home");
    let projects = root.join("projects");
    for dir in ["projects/legacy/src", "projects/modern", "outside"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    // Fake php: prints its version and the ini dir it was given, so the test sees which one ran.
    let fake = |v: &str| {
        let dir = fake_build(&root, v);
        fs::write(dir.join("php"), format!("#!/bin/sh\necho \"{v} $PHPRC $*\"\n")).unwrap();
        dir
    };
    let (b84, b85) = (fake("8.4"), fake("8.5"));
    let daemon = start_daemon(&home);
    // `php install` checks the version with `php -r`; the fake prints "<v> ..." so check via install of
    // a plain fake first, then swap in the verbose fake.
    for (v, dir) in [("8.4", &b84), ("8.5", &b85)] {
        let plain = fake_build(&root.join(format!("plain-{v}")), v);
        assert!(devid(&home, &["php", "install", v, "--from", plain.to_str().unwrap()]).status.success());
        fs::copy(dir.join("php"), home.join(format!("php/{v}/bin/php"))).unwrap();
    }
    assert!(devid(&home, &["park", projects.to_str().unwrap()]).status.success());
    assert!(devid(&home, &["isolate", "8.4", "--site", "legacy"]).status.success());
    daemon.stop(); // shims must work without the daemon

    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for name in ["php", "composer"] {
        symlink(BIN, bin.join(name)).unwrap();
    }
    let out = |cwd: &Path, args: &[&str]| {
        let o = shim(&bin, "php", &home, cwd, args);
        assert!(o.status.success(), "{}", stderr(&o));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let etc84 = home.join("php/8.4/etc").display().to_string();
    assert_eq!(
        out(&projects.join("legacy/src"), &["-v"]),
        format!("8.4 {etc84} -v"),
        "isolated site, from a subfolder"
    );
    assert!(out(&projects.join("modern"), &[]).starts_with("8.5 "), "site on the default version");
    assert!(out(&root.join("outside"), &["-r", "echo 1;"]).starts_with("8.5 "), "outside any site: default");

    // composer without composer.phar explains how to get it.
    let o = shim(&bin, "composer", &home, &projects.join("modern"), &["--version"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("devid setup"), "{}", stderr(&o));

    // A version that is not installed gives a clear error instead of running something else.
    fs::write(
        home.join("config.toml"),
        fs::read_to_string(home.join("config.toml")).unwrap().replace("\"8.5\"", "\"8.3\""),
    )
    .unwrap();
    let o = shim(&bin, "php", &home, &root.join("outside"), &[]);
    assert!(stderr(&o).contains("PHP 8.3 is not installed"), "{}", stderr(&o));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn setup_and_uninstall_dry_runs_change_nothing() {
    let root = temp_root("devid-setup-dry-it");
    let home = root.join("home");
    let fake_home = root.join("user");
    fs::create_dir_all(&fake_home).unwrap();
    let daemon = start_daemon(&home); // creates the CA the helper plan points at
    daemon.stop();

    let run = |args: &[&str]| {
        let o = Command::new(BIN).args(args).env("DEVID_HOME", &home).env("HOME", &fake_home).output().unwrap();
        assert!(o.status.success(), "{args:?}: {}", stderr(&o));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let plan = run(&["setup", "--dry-run"]);
    for expected in [
        "shims",
        "PATH",
        "LaunchAgent",
        "composer.phar",
        "/etc/resolver/test",
        "dry run: nothing changed",
        "install PHP 8.5",
        "install nginx",
    ] {
        assert!(plan.contains(expected), "setup dry run mentions {expected}:\n{plan}");
    }
    let plan = run(&["setup", "--dry-run", "--php", "8.3", "--php", "8.4", "--server", "caddy"]);
    for expected in
        ["install PHP 8.3", "install PHP 8.4", "make PHP 8.3 the default", "install caddy", "serve sites with caddy"]
    {
        assert!(plan.contains(expected), "setup dry run with choices mentions {expected}:\n{plan}");
    }
    let plan = run(&["setup", "--dry-run", "--no-php", "--no-server"]);
    assert!(!plan.contains("PHP") && !plan.contains("nginx"), "--no-php --no-server installs neither:\n{plan}");
    let plan = run(&["uninstall", "--dry-run", "--purge"]);
    for expected in ["remove-trusted-cert", "LaunchAgent", "PATH line", "delete"] {
        assert!(plan.contains(expected), "uninstall dry run mentions {expected}:\n{plan}");
    }
    assert!(fs::read_dir(&fake_home).unwrap().next().is_none(), "nothing written to the user's home");
    assert!(!home.join("bin").exists(), "binary not installed");
    fs::remove_dir_all(&root).unwrap();
}
