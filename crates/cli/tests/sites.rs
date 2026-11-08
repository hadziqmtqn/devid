//! Sites end to end: park, link, isolate, nginx config + reload, and rollback on a rejected config.

mod common;

use common::*;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn site<'a>(sites: &'a Value, name: &str) -> &'a Value {
    sites.as_array().unwrap().iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("site {name} missing"))
}

fn site_missing(sites: &Value, name: &str) -> bool {
    !sites.as_array().unwrap().iter().any(|s| s["name"] == name)
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
    // A wrong binary (nginx passed as Caddy ignores `version` and keeps running) is rejected
    // after the probe timeout instead of hanging the daemon.
    let out = devid(&home, &["server", "install", "caddy", "--from", &s(&nginx)]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("did not finish in 5 s"), "{}", stderr(&out));
    assert!(devid(&home, &["server", "install", "nginx", "--from", &s(&nginx)]).status.success());
    wait_for("nginx running", || json(&home, &["status", "--json"])["server_status"] == "running");

    // Park + link.
    assert!(devid(&home, &["park", &s(&projects)]).status.success());
    assert!(devid(&home, &["link", "--path", &s(&root.join("elsewhere/shop"))]).status.success());
    let sites = json(&home, &["sites", "--json"]);
    assert_eq!(sites.as_array().unwrap().len(), 3);
    let full = request_json(&home, r#"{"cmd":"sites"}"#);
    assert_eq!(full["parked"], serde_json::json!([s(&projects)]), "sites reply lists parked folders (TUI unpark)");
    assert_eq!(site(&sites, "blog")["url"], format!("https://blog.test:{TEST_HTTPS_PORT}"));
    assert_eq!(site(&sites, "blog")["http_url"], format!("http://blog.test:{TEST_HTTP_PORT}"));
    // Every site has a certificate from the Devid CA, and PHP trusts that CA.
    for host in ["blog.test", "api.test", "shop.test"] {
        assert!(home.join(format!("certs/{host}.crt")).exists(), "{host} certificate");
    }
    let ca = fs::read_to_string(home.join("ca/devid-ca.crt")).unwrap();
    assert!(fs::read_to_string(home.join("ca/cacert.pem")).unwrap().ends_with(&ca));
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

    // Ignore a parked site (D-029): not served, listed as ignored, the folder untouched; unignore brings it back.
    let out = devid(&home, &["ignore", "api"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(&format!("ignored api; the folder is still in {}", s(&projects)))
    );
    assert!(site_missing(&json(&home, &["sites", "--json"]), "api"));
    assert!(projects.join("api").is_dir(), "the folder stays");
    let full = request_json(&home, r#"{"cmd":"sites"}"#);
    assert_eq!(full["ignored"], serde_json::json!([{"name": "api", "path": s(&projects.join("api"))}]));
    assert!(!fs::read_to_string(home.join("nginx/conf/nginx.conf")).unwrap().contains("server_name api.test;"));
    assert!(stderr(&devid(&home, &["ignore", "shop"])).contains("shop is linked; use `devid unlink shop`"));
    assert!(devid(&home, &["unignore", "api"]).status.success());
    assert_eq!(site(&json(&home, &["sites", "--json"]), "api")["linked"], false);
    assert!(stderr(&devid(&home, &["unignore", "api"])).contains("api is not an ignored site"));

    // Unlink + unpark empty the site list.
    assert!(devid(&home, &["unlink", "shop"]).status.success());
    assert!(devid(&home, &["unpark", &s(&projects)]).status.success());
    assert!(json(&home, &["sites", "--json"]).as_array().unwrap().is_empty());

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}

/// App types (D-035, Phase 8 A2): detection, `devid app set/reset/show`, and php-fpm only for PHP sites.
#[test]
fn app_types_and_overrides() {
    let root = temp_root("devid-apps-it");
    let home = root.join("home");
    let projects = root.join("projects");
    let write = |rel: &str, text: &str| {
        let p = projects.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    write("lara/artisan", "");
    write("lara/composer.json", r#"{"require": {"laravel/framework": "^12"}}"#);
    write("lara/public/index.php", "");
    write("web/package.json", r#"{"devDependencies": {"vite": "^6"}}"#);
    write("lp/index.html", "");
    let (b84, b85, nginx) = (fake_build(&root, "8.4"), fake_build(&root, "8.5"), fake_nginx(&root, &home));
    let s = |p: &Path| p.to_str().unwrap().to_string();

    let daemon = start_daemon(&home);
    for (v, b) in [("8.4", &b84), ("8.5", &b85)] {
        assert!(devid(&home, &["php", "install", v, "--from", &s(b)]).status.success());
    }
    assert!(devid(&home, &["server", "install", "nginx", "--from", &s(&nginx)]).status.success());
    assert!(devid(&home, &["park", &s(&projects)]).status.success());

    let sites = json(&home, &["sites", "--json"]);
    let kind = |sites: &Value, name: &str| {
        let s = site(sites, name);
        (s["preset"].as_str().unwrap().to_string(), s["mode"].as_str().unwrap().to_string())
    };
    assert_eq!(kind(&sites, "lara"), ("laravel".into(), "php".into()));
    assert_eq!(kind(&sites, "web"), ("vite".into(), "proxy".into()));
    assert_eq!(site(&sites, "web")["upstream"], 5173);
    assert_eq!(kind(&sites, "lp"), ("static".into(), "static".into()));
    let conf = fs::read_to_string(home.join("nginx/conf/nginx.conf")).unwrap();
    assert!(conf.contains("proxy_pass http://localhost:5173;"));
    assert!(conf.contains(&format!("root \"{}\";", s(&projects.join("lp")))));

    // A static site's PHP version starts no php-fpm; switching it to php does.
    assert!(devid(&home, &["isolate", "8.4", "--site", "web"]).status.success());
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.4").0, "stopped", "web is a proxy site");
    let out = devid(&home, &["app", "set", "web", "--mode", "static"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("serve as: static dist/ (SPA)"));
    assert_eq!(site(&json(&home, &["sites", "--json"]), "web")["docroot"], s(&projects.join("web/dist")));
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.4").0, "stopped", "static sites need no php-fpm");
    assert!(devid(&home, &["app", "set", "web", "--mode", "php"]).status.success());
    wait_for("8.4 running for a php site", || proc_of(&json(&home, &["status", "--json"]), "8.4").0 == "running");

    // A preset brings its own defaults; reset goes back to detection; the config keeps only overrides.
    assert!(devid(&home, &["app", "set", "web", "--preset", "node", "--upstream", "3010"]).status.success());
    let web = site(&json(&home, &["sites", "--json"]), "web").clone();
    assert_eq!(
        (web["preset"].as_str(), web["mode"].as_str(), web["upstream"].as_u64()),
        (Some("node"), Some("proxy"), Some(3010))
    );
    assert!(fs::read_to_string(home.join("config.toml")).unwrap().contains("[apps.web]"));
    assert!(devid(&home, &["app", "reset", "web"]).status.success());
    assert!(!fs::read_to_string(home.join("config.toml")).unwrap().contains("[apps.web]"));
    wait_for("8.4 stopped again", || proc_of(&json(&home, &["status", "--json"]), "8.4").0 == "stopped");
    assert!(devid(&home, &["app", "set", "lara", "--preset", "laravel"]).status.success());
    assert!(
        !fs::read_to_string(home.join("config.toml")).unwrap().contains("[apps"),
        "choosing the detected preset stores nothing"
    );

    // `devid app show` inside a project picks that site.
    let out = Command::new(BIN)
        .args(["app", "show"])
        .current_dir(projects.join("web"))
        .env("DEVID_HOME", &home)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("type:     Vite (detected)") && text.contains("serve as: proxy to localhost:5173"), "{text}");

    // Validation.
    let port = TEST_HTTP_PORT.to_string();
    assert!(
        stderr(&devid(&home, &["app", "set", "web", "--upstream", &port])).contains("cannot be a dev server's port")
    );
    assert!(
        stderr(&devid(&home, &["app", "set", "web", "--root", "../x"])).contains("must be a folder inside the project")
    );
    assert!(!devid(&home, &["app", "set", "web", "--preset", "rails"]).status.success());
    assert!(stderr(&devid(&home, &["app", "set", "nope", "--mode", "php"])).contains("no site named nope"));

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
