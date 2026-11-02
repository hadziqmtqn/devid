//! Phase 5 R3: install and update from a release manifest, using a local `file://` release with
//! fake binaries (same layout and manifest schema as devid-binaries).

mod common;

use common::*;
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum").args(["-a", "256"]).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap().to_string()
}

fn script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Package a fake PHP `full` (e.g. 8.5.12) or nginx release asset into `rel`; returns (file name, sha256).
fn asset(root: &Path, rel: &Path, tool: &str, full: &str) -> (String, String) {
    let stage = root.join(format!("stage-{tool}-{full}"));
    fs::create_dir_all(stage.join("modules")).unwrap();
    if tool == "php" {
        let minor: String = full.splitn(3, '.').take(2).collect::<Vec<_>>().join(".");
        script(
            &stage.join("php"),
            &format!("#!/bin/sh\ncase \"$*\" in *'echo PHP_VERSION;'*) echo {full} ;; *) echo {minor} ;; esac\n"),
        );
        script(&stage.join("php-fpm"), IDLE_FPM);
        fs::write(stage.join("modules/xdebug.so"), b"").unwrap();
    } else {
        let fake = fake_nginx(&stage, &root.join("home"));
        let body = fs::read_to_string(&fake).unwrap().replace("nginx/fake", &format!("nginx/{full}"));
        script(&stage.join("nginx"), &body);
        fs::remove_file(fake).unwrap();
    }
    let name = format!("{tool}-{full}-darwin-x86_64.tar.gz");
    assert!(
        Command::new("tar").arg("-czf").arg(rel.join(&name)).arg("-C").arg(&stage).arg(".").status().unwrap().success()
    );
    let sum = sha256(&rel.join(&name));
    (name, sum)
}

fn entry(rel: &Path, (name, sum): &(String, String)) -> Value {
    let e = json!({"url": format!("file://{}/{name}", rel.display()), "sha256": sum, "size": 1});
    json!({"x86_64": e.clone(), "arm64": e})
}

fn publish(rel: &Path, tag: &str, php: &(String, String), php_full: &str, nginx: &(String, String), nginx_ver: &str) {
    let manifest = json!({
        "schema": 1, "tag": tag, "repo": "local/test",
        "php": {"8.5": {"version": php_full, "arch": entry(rel, php)}},
        "nginx": {"version": nginx_ver, "arch": entry(rel, nginx)}
    });
    fs::write(rel.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn install_and_update_from_a_release() {
    let root = temp_root("devid-releases-it");
    let home = root.join("home");
    let rel = root.join("release");
    fs::create_dir_all(&rel).unwrap();
    let (php1, nginx1) = (asset(&root, &rel, "php", "8.5.12"), asset(&root, &rel, "nginx", "1.30.5"));
    publish(&rel, "r1", &php1, "8.5.12", &nginx1, "1.30.5");
    let url = format!("file://{}/manifest.json", rel.display());
    let daemon = start_daemon_env(&home, &[("DEVID_MANIFEST_URL", &url)]);

    // Install without --from: downloaded from the manifest, verified, running.
    let o = devid(&home, &["php", "install", "8.5"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(out(&o).contains("PHP 8.5.12 installed"), "{}", out(&o));
    let o = devid(&home, &["server", "install", "nginx"]);
    assert!(o.status.success(), "{}", stderr(&o));
    wait_for("php + nginx running", || {
        let s = json(&home, &["status", "--json"]);
        s["server_status"] == "running" && proc_of(&s, "8.5").0 == "running"
    });
    assert!(out(&devid(&home, &["update", "--check"])).contains("everything is up to date (release r1)"));
    let leftovers: Vec<_> = fs::read_dir(home.join("downloads")).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(leftovers, ["manifest.json"], "archives and unpacked folders are cleaned up");

    // A new release: --check lists it without installing; update installs it.
    let (php2, nginx2) = (asset(&root, &rel, "php", "8.5.13"), asset(&root, &rel, "nginx", "1.30.6"));
    publish(&rel, "r2", &php2, "8.5.13", &nginx2, "1.30.6");
    let check = out(&devid(&home, &["update", "--check"]));
    assert!(check.contains("available: php 8.5 8.5.12 -> 8.5.13"), "{check}");
    assert!(check.contains("available: nginx 1.30.5 -> 1.30.6"), "{check}");
    assert!(out(&devid(&home, &["update", "--check"])).contains("8.5.12 -> 8.5.13"), "--check changed nothing");
    let o = devid(&home, &["update"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(out(&o).contains("updated: php 8.5 8.5.12 -> 8.5.13"), "{}", out(&o));
    assert!(out(&devid(&home, &["update", "--check"])).contains("everything is up to date (release r2)"));
    wait_for("8.5 running after update", || proc_of(&json(&home, &["status", "--json"]), "8.5").0 == "running");

    // A tampered asset is refused and the installed version stays.
    let mut bad = php2.clone();
    bad.1 = "0".repeat(64);
    publish(&rel, "r3", &bad, "8.5.14", &nginx2, "1.30.6");
    let o = devid(&home, &["php", "install", "8.5"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("checksum mismatch"), "{}", stderr(&o));
    assert!(out(&devid(&home, &["update", "--check"])).contains("8.5.13 -> 8.5.14"), "8.5.13 still installed");

    // An asset outside the manifest's own folder is refused before any download.
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(rel.join("manifest.json")).unwrap()).unwrap();
    manifest["php"]["8.5"]["arch"]["x86_64"]["url"] = json!("file:///etc/hosts");
    manifest["php"]["8.5"]["arch"]["arm64"]["url"] = json!("file:///etc/hosts");
    fs::write(rel.join("manifest.json"), manifest.to_string()).unwrap();
    let o = devid(&home, &["php", "install", "8.5"]);
    assert!(stderr(&o).contains("refusing file:///etc/hosts"), "{}", stderr(&o));

    // --from still works and needs no manifest.
    let local = fake_build(&root, "8.5");
    assert!(devid(&home, &["php", "install", "8.5", "--from", local.to_str().unwrap()]).status.success());

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
