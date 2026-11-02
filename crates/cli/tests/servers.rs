//! M7: switching the active web server between nginx, Caddy and Apache (fakes, so CI can run it).

mod common;

use common::*;
use serde_json::Value;
use std::{fs, path::Path};

fn server(home: &Path) -> (String, String) {
    let s = json(home, &["status", "--json"]);
    (s["server"].as_str().unwrap().to_string(), s["server_status"].as_str().unwrap().to_string())
}

fn pid_of(home: &Path, needle: &str) -> Option<u32> {
    let out = std::process::Command::new("pgrep").args(["-f", needle]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).lines().next().and_then(|l| l.trim().parse().ok()).filter(|_| home.exists())
}

#[test]
fn switch_between_nginx_caddy_and_apache() {
    let root = temp_root("devid-servers-it");
    let home = root.join("home");
    fs::create_dir_all(root.join("projects/blog/public")).unwrap();
    fs::create_dir_all(root.join("elsewhere/shop")).unwrap();
    let s = |p: &Path| p.to_str().unwrap().to_string();
    let (nginx, caddy, httpd) = (fake_nginx(&root, &home), fake_caddy(&root), fake_httpd(&root, &home));
    let php = fake_build(&root, "8.5");

    // No network in tests: point the manifest at a local file that does not exist.
    let manifest = format!("file://{}/no-release/manifest.json", root.display());
    let daemon = start_daemon_env(&home, &[("DEVID_MANIFEST_URL", &manifest)]);
    assert!(devid(&home, &["php", "install", "8.5", "--from", &s(&php)]).status.success());
    assert!(devid(&home, &["park", &s(&root.join("projects"))]).status.success());

    // Validation before anything is installed.
    assert!(stderr(&devid(&home, &["server", "use", "caddy"])).contains("not installed"));
    assert!(stderr(&devid(&home, &["server", "use", "iis"])).contains("unknown web server"));
    // Without --from, install downloads from the release manifest (unreachable here).
    assert!(stderr(&devid(&home, &["server", "install", "caddy"])).contains("download failed"));

    for (name, bin) in [("nginx", &nginx), ("caddy", &caddy), ("apache", &httpd)] {
        let out = devid(&home, &["server", "install", name, "--from", &s(bin)]);
        assert!(out.status.success(), "{name}: {}", stderr(&out));
    }
    wait_for("nginx running", || server(&home) == ("nginx".into(), "running".into()));
    let list: Value = json(&home, &["status", "--json"]);
    assert_eq!(list["server"], "nginx", "nginx stays the default (D-015)");

    // nginx -> caddy: Caddy starts with a Caddyfile for every site, nginx stops (one at a time).
    let nginx_pid = pid_of(&home, &s(&home.join("nginx/bin/nginx"))).expect("nginx runs");
    assert!(devid(&home, &["server", "use", "caddy"]).status.success());
    wait_for("caddy running", || server(&home) == ("caddy".into(), "running".into()));
    wait_for("nginx stopped", || pid_of(&home, &s(&home.join("nginx/bin/nginx"))) != Some(nginx_pid));
    let caddyfile = fs::read_to_string(home.join("caddy/Caddyfile")).unwrap();
    assert!(caddyfile.contains("https://blog.test:"), "{caddyfile}");
    assert!(fs::read_to_string(home.join("config.toml")).unwrap().contains("server = \"caddy\""));

    // A site change restarts Caddy (no signal reload) with the new config.
    let caddy_pid = pid_of(&home, &s(&home.join("caddy/bin/caddy"))).expect("caddy runs");
    assert!(devid(&home, &["link", "--path", &s(&root.join("elsewhere/shop"))]).status.success());
    wait_for("caddy restarted", || {
        pid_of(&home, &s(&home.join("caddy/bin/caddy"))).is_some_and(|p| p != caddy_pid) && server(&home).1 == "running"
    });
    assert!(fs::read_to_string(home.join("caddy/Caddyfile")).unwrap().contains("shop.test"));

    // caddy -> apache: httpd.conf rendered; a site change is a graceful SIGUSR1 reload.
    assert!(devid(&home, &["server", "use", "apache"]).status.success());
    wait_for("apache running", || server(&home) == ("apache".into(), "running".into()));
    assert!(fs::read_to_string(home.join("apache/httpd.conf")).unwrap().contains("ServerName shop.test"));
    assert!(devid(&home, &["unlink", "shop"]).status.success());
    wait_for("apache graceful reload", || home.join("apache-reloads").exists());
    assert!(!fs::read_to_string(home.join("apache/httpd.conf")).unwrap().contains("shop.test"));

    // Back to nginx.
    assert!(devid(&home, &["server", "use", "nginx"]).status.success());
    wait_for("nginx running again", || server(&home) == ("nginx".into(), "running".into()));
    let listing = String::from_utf8_lossy(&devid(&home, &["server", "list"]).stdout).into_owned();
    assert!(listing.contains("* nginx") && listing.contains("caddy") && listing.contains("apache"), "{listing}");

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
