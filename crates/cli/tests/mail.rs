//! Phase 9 M3 (D-049): Mail runs Mailpit, downloaded from the release the first time. A fake
//! Mailpit (a shell script) stands in for the real one; the real SMTP and API calls are covered by
//! the daemon's unit tests and by scripts/smoke.sh.

mod common;

use common::*;
use serde_json::{Value, json};
use std::{fs, net::TcpListener, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum").args(["-a", "256"]).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap().to_string()
}

/// A release archive with a fake `mailpit` that reports `version`, writes its arguments to
/// `<DEVID_HOME>/mailpit-args` and runs until SIGTERM; returns its manifest entry.
fn mailpit_asset(root: &Path, rel: &Path, version: &str) -> Value {
    let stage = root.join(format!("stage-mailpit-{version}"));
    fs::create_dir_all(&stage).unwrap();
    let script = format!(
        "#!/bin/sh\n\
         if [ \"$1\" = version ]; then echo \"$0 v{version} compiled with go1.27.3 on darwin/amd64\"; exit 0; fi\n\
         echo \"$@\" > \"$DEVID_HOME/mailpit-args\"\n\
         trap 'exit 0' TERM\n\
         while :; do sleep 0.1; done\n"
    );
    fs::write(stage.join("mailpit"), script).unwrap();
    fs::set_permissions(stage.join("mailpit"), fs::Permissions::from_mode(0o755)).unwrap();
    let name = format!("mailpit-{version}-darwin-x86_64.tar.gz");
    let tar = Command::new("tar").arg("-czf").arg(rel.join(&name)).arg("-C").arg(&stage).arg(".").status().unwrap();
    assert!(tar.success());
    let e = json!({"url": format!("file://{}/{name}", rel.display()), "sha256": sha256(&rel.join(&name)), "size": 1});
    json!({"version": version, "arch": {"x86_64": e.clone(), "arm64": e.clone(), "linux-x86_64": e}})
}

fn publish(rel: &Path, tag: &str, mailpit: Value) {
    let manifest = json!({"schema": 1, "tag": tag, "repo": "local/test", "php": {}, "mailpit": mailpit});
    fs::write(rel.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
}

/// An SMTP port whose next port is free too, so the test never meets a Mail running on this Mac.
fn free_pair() -> u16 {
    loop {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        if port < u16::MAX && TcpListener::bind(("127.0.0.1", port + 1)).is_ok() {
            return port;
        }
    }
}

fn mail(home: &Path) -> Value {
    json(home, &["status", "--json"])["mail"].clone()
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn mail_on_off_settings_and_update() {
    let root = temp_root("devid-mail-it");
    let home = root.join("home");
    let rel = root.join("release");
    fs::create_dir_all(&rel).unwrap();
    fs::create_dir_all(&home).unwrap();
    let port = free_pair();
    fs::write(
        home.join("config.toml"),
        format!(
            "http_port = {TEST_HTTP_PORT}\nhttps_port = {TEST_HTTPS_PORT}\ndns_port = {}\nupdate_check_minutes = 0\n\
             [mail]\nport = {port}\n",
            free_udp_port()
        ),
    )
    .unwrap();
    publish(&rel, "r1", mailpit_asset(&root, &rel, "1.33.1"));
    let url = format!("file://{}/manifest.json", rel.display());
    let daemon = start_daemon_env(&home, &[("DEVID_MANIFEST_URL", &url)]);
    let php = fake_build(&root, "8.5");
    assert!(devid(&home, &["php", "install", "8.5", "--from", php.to_str().unwrap()]).status.success());
    let ini = home.join("php/8.5/etc/conf.d/devid-mail.ini");

    // Off by default: nothing downloaded, nothing running, PHP untouched.
    assert_eq!(mail(&home)["state"], "off");
    assert!(
        out(&devid(&home, &["mail"])).contains("`devid mail on` turns it on (the first time, it downloads Mailpit")
    );
    assert!(!home.join("mailpit").exists() && !ini.exists());

    // On: downloaded from the release, running on loopback with its Host allowlist.
    let o = devid(&home, &["mail", "on"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(out(&o).contains(&format!("MAIL_PORT={port}")), "{}", out(&o));
    wait_for("mail running", || mail(&home)["state"] == "running");
    let m = mail(&home);
    assert_eq!((m["version"].as_str(), m["port"].as_u64()), (Some("1.33.1"), Some(port as u64)));
    assert_eq!(m["inbox_url"], format!("http://127.0.0.1:{}/", port + 1));
    // The fake writes its arguments in one echo; under load the file can exist before they are in it.
    let complete = |a: &String| a.contains("--smtp-auth-allow-insecure");
    wait_for("mailpit args", || fs::read_to_string(home.join("mailpit-args")).is_ok_and(|a| complete(&a)));
    let args = fs::read_to_string(home.join("mailpit-args")).unwrap();
    for want in [
        format!("--smtp 127.0.0.1:{port}"),
        format!("--listen 127.0.0.1:{}", port + 1),
        format!("--allowed-hosts 127.0.0.1:{0},localhost:{0}", port + 1),
        "--max 500".to_string(),
        "--disable-version-check".to_string(),
    ] {
        assert!(args.contains(&want), "{want} missing from: {args}");
    }
    let body = fs::read_to_string(&ini).unwrap();
    assert!(body.contains(&format!("sendmail -S 127.0.0.1:{port}")), "PHP's mail() goes to Mail: {body}");
    wait_for("the pid file the supervisor writes for Mailpit", || home.join("run/mailpit.pid").exists());
    assert!(out(&devid(&home, &["status"])).contains(&format!("mail:        running on 127.0.0.1:{port}")));

    // It is a service like the others: stop, start, stop all.
    assert!(devid(&home, &["stop", "mail"]).status.success());
    wait_for("mail paused", || mail(&home)["state"] == "paused");
    assert!(devid(&home, &["start", "mail"]).status.success());
    wait_for("mail running again", || mail(&home)["state"] == "running");

    // Ports it may not take, and settings it refuses.
    let o = devid(&home, &["mail", "port", &(TEST_HTTP_PORT - 1).to_string()]);
    assert!(stderr(&o).contains(&format!("port {TEST_HTTP_PORT} is Devid's HTTP port")), "{}", stderr(&o));
    let busy = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = busy.local_addr().unwrap().port();
    let o = devid(&home, &["mail", "port", &taken.to_string()]);
    assert!(stderr(&o).contains(&format!("port {taken} is in use by")), "{}", stderr(&o));
    drop(busy);
    assert!(stderr(&devid(&home, &["mail", "keep", "0"])).contains("use 1 to 100000"));
    assert_eq!(mail(&home)["port"], port, "a refused change keeps the config");

    // Settings that restart Mailpit or change PHP.
    assert!(devid(&home, &["mail", "keep", "100"]).status.success());
    wait_for("mailpit restarted with --max 100", || {
        fs::read_to_string(home.join("mailpit-args")).is_ok_and(|a| complete(&a) && a.contains("--max 100"))
    });
    assert!(devid(&home, &["mail", "php-mail", "off"]).status.success());
    assert!(!ini.exists(), "PHP's mail() is left alone");
    assert!(devid(&home, &["mail", "notify", "on"]).status.success());
    assert_eq!((mail(&home)["notify"].as_bool(), mail(&home)["php_mail"].as_bool()), (Some(true), Some(false)));

    // A newer Mailpit in the release is an update like nginx's.
    publish(&rel, "r2", mailpit_asset(&root, &rel, "1.33.2"));
    let check = out(&devid(&home, &["update", "--check"]));
    assert!(check.contains("mailpit") && check.contains("1.33.1 -> 1.33.2"), "{check}");
    let o = devid(&home, &["update"]);
    assert!(o.status.success(), "{}", stderr(&o));
    wait_for("mailpit 1.33.2", || mail(&home)["version"] == "1.33.2");

    // Off: stopped, PHP untouched, the binary and the mail kept for next time.
    assert!(devid(&home, &["mail", "php-mail", "on"]).status.success());
    assert!(ini.exists());
    assert!(devid(&home, &["mail", "off"]).status.success());
    assert_eq!(mail(&home)["state"], "off");
    assert!(!ini.exists(), "no Mail, no sendmail_path");
    assert!(stderr(&devid(&home, &["mail", "test"])).contains("Mail is off"));
    assert!(home.join("mailpit/bin/mailpit").exists() && home.join("mail").is_dir());
    assert!(out(&devid(&home, &["mail"])).contains("`devid mail on` turns it on\n"), "no download the second time");

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
