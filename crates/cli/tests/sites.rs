//! Sites end to end: park, link, isolate, nginx config + reload, and rollback on a rejected config.

mod common;

use common::*;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn site<'a>(sites: &'a Value, name: &str) -> &'a Value {
    sites.as_array().unwrap().iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("site {name} missing"))
}

fn reloads(home: &Path) -> usize {
    fs::read_to_string(home.join("reloads")).map(|s| s.lines().count()).unwrap_or(0)
}

#[test]
fn park_link_isolate_and_nginx() {
    let root = temp_root("devid-sites-it");
    let home = root.join("home");
    let projects = root.join("projects");
    for dir in ["projects/blog/public", "projects/api", "elsewhere/shop", "elsewhere/reject"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    let (b84, b85, nginx) = (fake_build(&root, "8.4"), fake_build(&root, "8.5"), fake_nginx(&root, &home));
    let s = |p: &Path| p.to_str().unwrap().to_string();

    let daemon = start_daemon(&home);
    assert_eq!(json(&home, &["status", "--json"])["server_status"], "not installed");
    for (v, b) in [("8.4", &b84), ("8.5", &b85)] {
        assert!(devid(&home, &["php", "install", v, "--from", &s(b)]).status.success());
    }
    assert!(devid(&home, &["server", "install", "caddy", "--from", &s(&nginx)]).status.code() == Some(1));
    assert!(devid(&home, &["server", "install", "nginx", "--from", &s(&nginx)]).status.success());
    wait_for("nginx running", || json(&home, &["status", "--json"])["server_status"] == "running");

    // Park + link.
    assert!(devid(&home, &["park", &s(&projects)]).status.success());
    assert!(devid(&home, &["link", "--path", &s(&root.join("elsewhere/shop"))]).status.success());
    let sites = json(&home, &["sites", "--json"]);
    assert_eq!(sites.as_array().unwrap().len(), 3);
    assert_eq!(site(&sites, "blog")["url"], "http://blog.test:8088");
    assert_eq!(site(&sites, "blog")["docroot"], s(&projects.join("blog/public")));
    assert_eq!(site(&sites, "shop")["linked"], true);
    let conf = fs::read_to_string(home.join("nginx/conf/nginx.conf")).unwrap();
    for host in ["blog.test", "api.test", "shop.test"] {
        assert!(conf.contains(&format!("server_name {host};")), "{host} in nginx.conf");
    }
    wait_for("reload after site changes", || reloads(&home) >= 1);

    // Isolate blog to 8.4: its master starts next to the default 8.5, and nginx points blog at it.
    assert!(devid(&home, &["isolate", "8.4", "--site", "blog"]).status.success());
    wait_for("8.4 running", || proc_of(&json(&home, &["status", "--json"]), "8.4").0 == "running");
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.5").0, "running");
    let conf = fs::read_to_string(home.join("nginx/conf/nginx.conf")).unwrap();
    let blog = &conf[conf.find("server_name blog.test;").unwrap()..];
    let blog = &blog[..blog.find("\n    }").unwrap()];
    assert!(blog.contains("php84.sock"), "blog uses the 8.4 socket:\n{blog}");

    // Isolate from inside a project folder (no --site).
    let out = Command::new(BIN)
        .args(["isolate", "8.4"])
        .current_dir(projects.join("api"))
        .env("DEVID_HOME", &home)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(site(&json(&home, &["sites", "--json"]), "api")["php"], "8.4");

    // Unisolate both: nobody uses 8.4 any more, so its master stops.
    assert!(devid(&home, &["unisolate", "--site", "blog"]).status.success());
    assert!(devid(&home, &["unisolate", "--site", "api"]).status.success());
    wait_for("8.4 stopped", || proc_of(&json(&home, &["status", "--json"]), "8.4").0 == "stopped");

    // A config nginx rejects is rolled back: error, no new link, previous nginx.conf kept.
    let before = fs::read_to_string(home.join("nginx/conf/nginx.conf")).unwrap();
    let out = devid(&home, &["link", "--path", &s(&root.join("elsewhere/reject"))]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("fake rejection"), "{}", stderr(&out));
    assert!(!fs::read_to_string(home.join("config.toml")).unwrap().contains("reject"));
    assert_eq!(fs::read_to_string(home.join("nginx/conf/nginx.conf")).unwrap(), before);

    // Validation.
    assert!(stderr(&devid(&home, &["link", "Bad Name", "--path", &s(&projects)])).contains("not a valid site name"));
    assert!(stderr(&devid(&home, &["isolate", "8.3", "--site", "blog"])).contains("not installed"));
    assert!(stderr(&devid(&home, &["isolate", "8.4", "--site", "nope"])).contains("no site named nope"));

    // Unlink + unpark empty the site list.
    assert!(devid(&home, &["unlink", "shop"]).status.success());
    assert!(devid(&home, &["unpark", &s(&projects)]).status.success());
    assert!(json(&home, &["sites", "--json"]).as_array().unwrap().is_empty());

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
