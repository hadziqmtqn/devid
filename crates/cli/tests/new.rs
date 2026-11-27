//! `devid new` end to end (D-063), with a fake PHP that plays Composer and artisan: the checks made
//! before anything is written, each PHP template's settings files, the site it ends as (parked or
//! linked, isolated), the progress, and what a failure leaves.

mod common;

use common::*;
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

/// PHP that acts as composer.phar (`create-project` makes a folder that looks like the package's)
/// and as artisan, and records what it was asked in `<DEVID_HOME>/calls`.
const FAKE_PHP: &str = r#"#!/bin/sh
case "$1" in
  *composer.phar)
    shift
    echo "composer $*" >> "$DEVID_HOME/calls"
    if [ "$1" = create-project ]; then
      pkg="$2"; dir="$3"
      mkdir -p "$dir/public" "$dir/database"
      echo "$pkg" > "$dir/.package"
      case "$dir" in *broken*) echo "fake composer: the network is down" >&2; exit 3 ;; esac
      case "$pkg" in
        laravel/*)
          printf 'APP_NAME=Laravel\nAPP_URL=http://localhost:8000\n\nDB_CONNECTION=sqlite\n# DB_HOST=127.0.0.1\n# DB_PORT=3306\n# DB_DATABASE=laravel\n\nMAIL_MAILER=log\n' > "$dir/.env"
          echo '{"require": {"laravel/framework": "^13"}}' > "$dir/composer.json"
          touch "$dir/artisan" "$dir/database/database.sqlite" ;;
        codeigniter4/*)
          printf '# ENVIRONMENT\n\n# CI_ENVIRONMENT = production\n\n# app.baseURL = %s\n' "''" > "$dir/env" ;;
        symfony/*)
          printf 'DATABASE_URL="postgresql://app:!ChangeMe!@127.0.0.1:5432/app"\n' > "$dir/.env" ;;
      esac
    fi
    exit 0 ;;
  artisan)
    echo "artisan $*" >> "$DEVID_HOME/calls"; exit 0 ;;
esac
echo 8.5
"#;

fn ok(home: &Path, args: &[&str]) -> String {
    let out = devid(home, args);
    assert!(out.status.success(), "devid {args:?}: {}", stderr(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn fails(home: &Path, args: &[&str]) -> String {
    let out = devid(home, args);
    assert!(!out.status.success(), "devid {args:?} should fail");
    stderr(&out)
}

fn site<'a>(sites: &'a Value, name: &str) -> &'a Value {
    sites.as_array().unwrap().iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("site {name} missing"))
}

#[test]
fn new_projects_from_the_php_templates() {
    let root = temp_root("devid-new-it");
    let home = root.join("home");
    let sites = root.join("sites");
    let elsewhere = root.join("elsewhere");
    fs::create_dir_all(&sites).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    let build = fake_build(&root, "8.5");
    fs::write(build.join("php"), FAKE_PHP).unwrap();
    fs::set_permissions(build.join("php"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(home.join("composer")).unwrap();
    fs::write(home.join("composer/composer.phar"), "").unwrap();
    let s = |p: &Path| p.to_str().unwrap().to_string();

    let daemon = start_daemon_env(&home, &[("SHELL", "/bin/sh")]);
    ok(&home, &["php", "install", "8.5", "--from", &s(&build)]);

    // The list, and the checks made before anything is written.
    let list = ok(&home, &["new", "--list"]);
    for t in [
        "laravel",
        "wordpress",
        "symfony",
        "codeigniter",
        "static",
        "--kit react, vue, svelte, livewire",
        "next",
        "express",
    ] {
        assert!(list.contains(t), "{t} in {list}");
    }
    assert!(fails(&home, &["new", "php", "blog"]).contains("no parked folder"));
    ok(&home, &["park", &s(&sites)]);
    assert!(fails(&home, &["new", "php", "1blog"]).contains("not a project name"));
    assert!(fails(&home, &["new", "rails", "blog"]).contains("no template"));
    assert!(fails(&home, &["new", "next", "blog", "--php", "8.5"]).contains("uses no PHP"));
    assert!(fails(&home, &["new", "laravel", "blog", "--db", "mongodb"]).contains("sqlite, mysql, pgsql"));
    assert!(fails(&home, &["new", "symfony", "blog", "--kit", "skeleton", "--db", "mysql"]).contains("skeleton"));
    assert!(fails(&home, &["new", "laravel", "blog", "--php", "8.4"]).contains("PHP 8.4 is not installed"));
    fs::create_dir_all(sites.join("full")).unwrap();
    fs::write(sites.join("full/index.php"), "").unwrap();
    assert!(fails(&home, &["new", "php", "full"]).contains("a site named full exists"));
    fs::create_dir_all(elsewhere.join("taken")).unwrap();
    fs::write(elsewhere.join("taken/x"), "").unwrap();
    assert!(fails(&home, &["new", "php", "taken", "--path", &s(&elsewhere.join("taken"))]).contains("not empty"));
    assert!(!home.join("calls").exists(), "nothing ran before the checks passed");

    // Laravel in the parked folder, on SQLite: Laravel's scripts did the database; Devid the URL.
    let out = ok(&home, &["new", "laravel", "shop"]);
    assert!(out.contains("shop is ready: https://shop.test:"), "{out}");
    let shop = sites.join("shop");
    assert_eq!(fs::read_to_string(shop.join(".package")).unwrap().trim(), "laravel/laravel");
    let env = fs::read_to_string(shop.join(".env")).unwrap();
    assert!(env.contains(&format!("APP_URL=https://shop.test:{TEST_HTTPS_PORT}\n")), "{env}");
    assert!(env.contains("DB_CONNECTION=sqlite\n") && env.contains("MAIL_MAILER=log\n"), "Mail is off: {env}");
    let calls = fs::read_to_string(home.join("calls")).unwrap();
    assert!(calls.contains(&format!("composer create-project laravel/laravel {} --prefer-dist", s(&shop))), "{calls}");
    assert!(!calls.contains("artisan migrate"), "SQLite is migrated by Laravel's own scripts: {calls}");
    let progress = request_json(&home, r#"{"cmd":"new_project_progress"}"#)["progress"].clone();
    assert_eq!(progress["done"], true);
    assert!(progress["steps"].as_array().unwrap().iter().all(|s| s["state"] == "done"), "{progress}");
    assert!(home.join("log/new-shop.log").exists());
    let listed = json(&home, &["sites", "--json"]);
    assert_eq!(site(&listed, "shop")["preset"], "laravel");

    // A starter kit saves Vite as the site's command (Node is checked first; skip without one).
    let kit = devid(&home, &["new", "laravel", "kit", "--kit", "react"]);
    if kit.status.success() {
        let calls = fs::read_to_string(home.join("calls")).unwrap();
        assert!(calls.contains("laravel/react-starter-kit") && calls.contains("--stability=dev"), "{calls}");
        let cmds = request_json(&home, r#"{"cmd":"sites"}"#);
        let kit_site = site(&cmds["sites"], "kit");
        assert!(kit_site["commands"].to_string().contains("npm run dev"), "{kit_site}");
    } else {
        assert!(stderr(&kit).contains("Node.js"), "{}", stderr(&kit));
    }

    // Symfony's web app: no Docker files, and SQLite in .env.local.
    ok(&home, &["new", "symfony", "api"]);
    let calls = fs::read_to_string(home.join("calls")).unwrap();
    assert!(calls.contains("composer config --json extra.symfony.docker false"), "{calls}");
    assert!(calls.contains("composer require webapp"), "{calls}");
    let local = fs::read_to_string(sites.join("api/.env.local")).unwrap();
    assert!(local.contains("DATABASE_URL=\"sqlite:///%kernel.project_dir%/var/data_%kernel.environment%.db\""));

    // CodeIgniter's env becomes .env.
    ok(&home, &["new", "codeigniter", "ci"]);
    let env = fs::read_to_string(sites.join("ci/.env")).unwrap();
    assert!(env.contains("CI_ENVIRONMENT = development\n"), "{env}");
    assert!(env.contains(&format!("app.baseURL = 'https://ci.test:{TEST_HTTPS_PORT}/'\n")), "{env}");

    // Static and blank PHP elsewhere are linked; --php pins the site.
    ok(&home, &["new", "static", "page", "--path", &s(&elsewhere.join("page"))]);
    assert!(fs::read_to_string(elsewhere.join("page/index.html")).unwrap().contains("<title>page</title>"));
    ok(&home, &["new", "php", "blog", "--php", "8.5", "--path", &s(&elsewhere.join("blog"))]);
    let listed = json(&home, &["sites", "--json"]);
    assert_eq!(site(&listed, "page")["linked"], true);
    assert_eq!(site(&listed, "blog")["isolated"], true);
    assert_eq!(site(&listed, "shop")["linked"], false);

    // A failed tool stops the job, keeps the folder and says so.
    let err = fails(&home, &["new", "codeigniter", "broken"]);
    assert!(err.contains("composer failed") && err.contains("the network is down"), "{err}");
    assert!(err.contains("What was made stays: the folder") && sites.join("broken").is_dir(), "{err}");
    let progress = request_json(&home, r#"{"cmd":"new_project_progress"}"#)["progress"].clone();
    assert!(progress["error"].is_string());
    assert!(progress["steps"].as_array().unwrap().iter().any(|s| s["state"] == "failed"), "{progress}");

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}

/// Node, npm and npx in `<DEVID_HOME>/bin`, which comes first on the PATH Devid gives tools: the
/// create tools make a package.json like the framework's, `npm install` a node_modules folder,
/// and `npm view` answers with Vite's `engines.node`. `<DEVID_HOME>/node-version` sets the Node.
const FAKE_NODE: &str = "#!/bin/sh\ncat \"$DEVID_HOME/node-version\" 2>/dev/null || echo v24.22.3\n";
const FAKE_NPM: &str = r#"#!/bin/sh
echo "$(basename "$0") $*" >> "$DEVID_HOME/calls"
case "$1" in
  view) echo '^20.19.0 || >=22.12.0' ;;
  install) mkdir -p node_modules ;;
  create) mkdir -p "$3"; echo '{"scripts": {"dev": "vite"}, "devDependencies": {"vite": "^8"}}' > "$3/package.json" ;;
  --yes) mkdir -p "$3"; echo '{"scripts": {"dev": "next dev"}, "dependencies": {"next": "16"}}' > "$3/package.json" ;;
esac
exit 0
"#;

#[test]
fn new_node_projects_are_proxied_and_start_on_demand() {
    let root = temp_root("devid-new-node-it");
    let home = root.join("home");
    let sites = root.join("sites");
    fs::create_dir_all(home.join("bin")).unwrap();
    fs::create_dir_all(&sites).unwrap();
    for (name, body) in [("node", FAKE_NODE), ("npm", FAKE_NPM), ("npx", FAKE_NPM)] {
        fs::write(home.join("bin").join(name), body).unwrap();
        fs::set_permissions(home.join("bin").join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let s = |p: &Path| p.to_str().unwrap().to_string();
    let daemon = start_daemon_env(&home, &[("SHELL", "/bin/sh")]);
    ok(&home, &["park", &s(&sites)]);
    // The app's sheet shows which Node it will use (N4).
    assert_eq!(request_json(&home, r#"{"cmd":"new_templates"}"#)["node"], "24.22.3");

    // Vite's React: created without installing or starting, then npm install; a proxy site whose
    // first visit starts `dev` on its own port. It is the first site on demand, so the daemon
    // starts listening for visits then (N5's weekly check found it did not).
    let wake = home.join("run/wake.sock");
    assert!(!wake.exists(), "no site is on demand yet");
    let out = ok(&home, &["new", "react", "web"]);
    assert!(wake.exists(), "the first visit could not wake the dev server");
    assert!(out.contains("web is ready") && out.contains("dev      port"), "{out}");
    let calls = fs::read_to_string(home.join("calls")).unwrap();
    assert!(calls.contains("npm view vite engines.node"), "{calls}");
    assert!(
        calls.contains("npm create vite@latest web -- --template react-ts --no-interactive --no-immediate"),
        "{calls}"
    );
    assert!(calls.contains("npm install"), "{calls}");
    assert!(sites.join("web/node_modules").is_dir());
    let full = request_json(&home, r#"{"cmd":"sites"}"#);
    let web = site(&full["sites"], "web");
    assert_eq!(web["preset"], "vite");
    assert_eq!(web["mode"], "proxy");
    assert!(web["upstream"].as_u64().unwrap() >= 5173, "{web}");
    assert!(web["commands"].to_string().contains("npm run dev -- --port $PORT --strictPort"), "{web}");
    assert_eq!(web["app"]["on_demand"], "dev", "{web}");

    // Next.js gets its options and another port than any site's.
    ok(&home, &["new", "next", "shop"]);
    let calls = fs::read_to_string(home.join("calls")).unwrap();
    assert!(calls.contains("npx --yes create-next-app@latest shop --yes"), "{calls}");
    assert!(calls.contains("--skip-install --disable-git"), "{calls}");
    let full = request_json(&home, r#"{"cmd":"sites"}"#);
    let (shop, web) = (site(&full["sites"], "shop"), site(&full["sites"], "web"));
    assert_eq!(shop["preset"], "next");
    assert_ne!(shop["upstream"], web["upstream"]);

    // Express: Devid's own server.js and package.json, and `npm install express`.
    ok(&home, &["new", "express", "api"]);
    assert!(fs::read_to_string(sites.join("api/server.js")).unwrap().contains("process.env.PORT"));
    assert!(fs::read_to_string(sites.join("api/package.json")).unwrap().contains("node --watch server.js"));
    assert!(fs::read_to_string(home.join("calls")).unwrap().contains("npm install express"));

    // A Node.js older than the framework's engines stops the job before anything is created.
    fs::write(home.join("node-version"), "v20.0.0\n").unwrap();
    let err = fails(&home, &["new", "vue", "old"]);
    assert!(err.contains("vite needs Node.js ^20.19.0 || >=22.12.0; this is 20.0.0"), "{err}");
    assert!(!sites.join("old").exists());

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
