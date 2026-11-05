//! Phase 6 E3: catalog extensions from a local `file://` release, with fake `php`/`php-fpm` that list
//! the extensions conf.d turns on and refuse a broken `.so`, like real PHP does at startup.

mod common;

use common::*;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
    process::Command,
};

const API: &str = "20250925";

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum").args(["-a", "256"]).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap().to_string()
}

fn script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn tar(stage: &Path, archive: &Path) {
    assert!(Command::new("tar").arg("-czf").arg(archive).arg("-C").arg(stage).arg(".").status().unwrap().success());
}

/// `-m`: Core, then every conf.d `extension=` whose file starts with "ok"; any other file gets PHP's
/// startup warning instead. `-n -m` ignores conf.d.
const MODULES: &str = r#"modules() {
  echo '[PHP Modules]'; echo Core
  [ "$1" = -n ] && return
  for f in "$PHP_INI_SCAN_DIR"/*.ini; do
    so=$(sed -n 's/^extension=//p' "$f" 2>/dev/null)
    [ -n "$so" ] || continue
    if grep -q '^ok' "$so" 2>/dev/null; then basename "$so" .so
    else echo "PHP Warning:  PHP Startup: Unable to load dynamic library '$so'"; fi
  done
}
"#;

/// A fake PHP 8.5.12 release asset; returns its manifest entry.
fn php_asset(root: &Path, rel: &Path, full: &str) -> Value {
    let stage = root.join(format!("stage-php-{full}"));
    fs::create_dir_all(stage.join("modules")).unwrap();
    script(
        &stage.join("php"),
        &format!(
            "#!/bin/sh\n{MODULES}case \" $* \" in\n\
             *'echo PHP_VERSION;'*) echo {full} ;;\n\
             *PHP_EXTENSION_DIR*) echo /fake/lib/php/extensions/no-debug-non-zts-{API} ;;\n\
             *getenv*) echo \"magick=$MAGICK_CONFIGURE_PATH\" ;;\n\
             *' -m '*) modules \"$@\" ;;\n\
             *) echo 8.5 ;;\nesac\n"
        ),
    );
    let idle = IDLE_FPM.trim_start_matches("#!/bin/sh\n");
    script(
        &stage.join("php-fpm"),
        &format!("#!/bin/sh\n{MODULES}case \" $* \" in *' -m '*) modules \"$@\"; exit 0 ;; esac\n{idle}"),
    );
    fs::write(stage.join("modules/xdebug.so"), b"").unwrap();
    let name = format!("php-{full}-darwin-x86_64.tar.gz");
    tar(&stage, &rel.join(&name));
    json!({"version": full, "arch": arch(rel, &name)})
}

/// A fake extension archive; `so` is the .so content ("ok…" loads, anything else is refused).
fn ext_asset(root: &Path, rel: &Path, name: &str, version: &str, so: &str, api: &str, env: Value) -> Value {
    let tag = format!("{name}-{version}-{}", hex(so));
    let stage = root.join(format!("stage-ext-{tag}"));
    fs::create_dir_all(stage.join("etc")).unwrap();
    fs::write(stage.join(format!("{name}.so")), so).unwrap();
    let info = json!({"name": name, "version": version, "php": "8.5", "php_api": api, "type": "extension",
                      "file": format!("{name}.so"), "license": "PHP-3.01", "env": env});
    fs::write(stage.join("ext.json"), info.to_string()).unwrap();
    let file = format!("ext-{tag}-php8.5-darwin-x86_64.tar.gz");
    tar(&stage, &rel.join(&file));
    json!({"version": version, "type": "extension", "file": format!("{name}.so"), "php_api": api,
           "license": "PHP-3.01", "env": env, "arch": arch(rel, &file)})
}

/// Distinct archive names for distinct builds of the same version.
fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn arch(rel: &Path, file: &str) -> Value {
    let e = json!({"url": format!("file://{}/{file}", rel.display()), "sha256": sha256(&rel.join(file)), "size": 1});
    json!({"x86_64": e.clone(), "arm64": e})
}

fn publish(rel: &Path, tag: &str, php: &Value, extensions: Value) {
    let m =
        json!({"schema": 1, "tag": tag, "repo": "local/test", "php": {"8.5": php}, "extensions": {"8.5": extensions}});
    fs::write(rel.join("manifest.json"), m.to_string()).unwrap();
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn fpm_pid(home: &Path) -> Option<u64> {
    proc_of(&json(home, &["status", "--json"]), "8.5").1
}

#[test]
fn catalog_extensions_install_update_remove() {
    let root = temp_root("devid-ext-it");
    let (home, rel) = (root.join("home"), root.join("release"));
    fs::create_dir_all(&rel).unwrap();
    let php = php_asset(&root, &rel, "8.5.12");
    let apcu = ext_asset(&root, &rel, "apcu", "5.1.28", "ok apcu", API, json!({}));
    let imagick =
        ext_asset(&root, &rel, "imagick", "3.8.1", "ok imagick", API, json!({"MAGICK_CONFIGURE_PATH": "etc"}));
    let broken = ext_asset(&root, &rel, "broken", "1.0", "missing symbol", API, json!({}));
    let oldapi = ext_asset(&root, &rel, "oldapi", "1.0", "ok oldapi", "20240924", json!({}));
    publish(&rel, "r1", &php, json!({"apcu": apcu, "imagick": imagick, "broken": broken, "oldapi": oldapi}));
    let url = format!("file://{}/manifest.json", rel.display());
    let _daemon = start_daemon_env(&home, &[("DEVID_MANIFEST_URL", &url)]);
    let ok = |args: &[&str]| {
        let o = devid(&home, args);
        assert!(o.status.success(), "{args:?}: {}", stderr(&o));
        out(&o)
    };
    let fails = |args: &[&str]| {
        let o = devid(&home, args);
        assert!(!o.status.success(), "{args:?} should fail: {}", out(&o));
        stderr(&o)
    };

    ok(&["php", "install", "8.5"]);
    wait_for("8.5 running", || fpm_pid(&home).is_some());
    let available = ok(&["php", "ext", "available", "8.5"]);
    for name in ["apcu", "broken", "imagick", "oldapi"] {
        assert!(available.contains(name), "{available}");
    }
    assert!(!available.contains("installed"), "{available}");

    // Install: files outside the PHP folder, the ini in conf.d, the archive's sha256 recorded.
    let o = ok(&["php", "ext", "install", "8.5", "apcu"]);
    assert!(o.contains("installed apcu 5.1.28 · php-fpm 8.5 restarted"), "{o}");
    let dir = home.join("php-ext/8.5/apcu");
    assert!(dir.join("apcu.so").is_file());
    let ini = fs::read_to_string(home.join("php/8.5/etc/conf.d/devid-apcu.ini")).unwrap();
    assert!(ini.contains(&format!("extension={}", dir.join("apcu.so").display())), "{ini}");
    assert!(ini.contains("\napc.enable_cli=1\n"), "apcu is on in the CLI too: {ini}");
    assert_eq!(fs::read_to_string(dir.join(".sha256")).unwrap(), apcu["arch"]["x86_64"]["sha256"].as_str().unwrap());
    assert!(ok(&["php", "ext", "available", "8.5"]).contains("installed 5.1.28"));
    assert!(fs::read_dir(home.join("downloads")).unwrap().flatten().all(|e| e.file_name() == "manifest.json"));
    wait_for("8.5 running after the install", || fpm_pid(&home).is_some());

    // Several at once: each on its own, one php-fpm restart; a failure does not undo the others.
    let o = devid(&home, &["php", "ext", "install", "8.5", "imagick", "broken"]);
    assert!(!o.status.success(), "a failed extension makes the command fail");
    assert!(out(&o).contains("installed imagick 3.8.1 · php-fpm 8.5 restarted · failed: broken"), "{}", out(&o));
    assert!(stderr(&o).contains("broken: broken does not load in PHP 8.5") && stderr(&o).contains("Unable to load"));
    assert!(home.join("php-ext/8.5/imagick/imagick.so").is_file() && !home.join("php-ext/8.5/broken").exists());

    // imagick's environment reaches the php-fpm pool and the php shim.
    let conf = fs::read_to_string(home.join("php/8.5/etc/php-fpm.conf")).unwrap();
    let magick = home.join("php-ext/8.5/imagick/etc");
    assert!(conf.contains(&format!("env[MAGICK_CONFIGURE_PATH] = \"{}\"", magick.display())), "{conf}");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    symlink(BIN, bin.join("php")).unwrap();
    let shim = Command::new(bin.join("php"))
        .args(["-r", "echo getenv('X');"])
        .current_dir(&root)
        .env("DEVID_HOME", &home)
        .output()
        .unwrap();
    assert_eq!(out(&shim).trim(), format!("magick={}", magick.display()), "{}", stderr(&shim));

    // A .so that does not load is rolled back before php-fpm restarts; so is a wrong API or name.
    wait_for("8.5 running", || fpm_pid(&home).is_some());
    let pid = fpm_pid(&home);
    let e = fails(&["php", "ext", "install", "8.5", "broken"]);
    assert!(e.contains("does not load in PHP 8.5, so nothing changed") && e.contains("Unable to load"), "{e}");
    assert!(!home.join("php-ext/8.5/broken").exists() && !home.join("php/8.5/etc/conf.d/devid-broken.ini").exists());
    assert_eq!(fpm_pid(&home), pid, "php-fpm was not restarted");
    assert!(fails(&["php", "ext", "install", "8.5", "oldapi"]).contains("PHP API 20240924"));
    assert!(fails(&["php", "ext", "install", "8.5", "nope"]).contains("not in the catalog"));
    assert!(fails(&["php", "ext", "install", "8.5", "../x"]).contains("not an extension name"));
    assert!(fails(&["php", "ext", "remove", "8.5", "nope"]).contains("not installed"));
    let list = ok(&["php", "ext", "list", "8.5"]);
    assert!(list.contains("catalog: apcu 5.1.28, imagick 3.8.1") && list.contains("xdebug off"), "{list}");

    // A rebuild of the same version is found by its sha256 and reinstalled by `update`.
    let apcu2 = ext_asset(&root, &rel, "apcu", "5.1.28", "ok apcu rebuilt", API, json!({}));
    publish(&rel, "r2", &php, json!({"apcu": apcu2, "imagick": imagick}));
    let check = ok(&["update", "--check"]);
    assert!(check.contains("available: apcu for php 8.5 5.1.28 -> 5.1.28 (new build)"), "{check}");
    assert!(!check.contains("imagick"), "{check}");
    assert!(ok(&["update"]).contains("updated: apcu for php 8.5"));
    assert_eq!(fs::read_to_string(dir.join(".sha256")).unwrap(), apcu2["arch"]["x86_64"]["sha256"].as_str().unwrap());
    assert!(fs::read_to_string(dir.join("apcu.so")).unwrap().contains("rebuilt"));

    // A PHP update keeps the extensions (outside ~/.devid/php/8.5) and drops one the release lost.
    let php2 = php_asset(&root, &rel, "8.5.13");
    publish(&rel, "r3", &php2, json!({"apcu": apcu2}));
    let updated = ok(&["update"]);
    assert!(updated.contains("updated: php 8.5 8.5.12 -> 8.5.13"), "{updated}");
    assert!(updated.contains("imagick for php 8.5 3.8.1 -> removed"), "{updated}");
    assert!(dir.join("apcu.so").is_file() && !home.join("php-ext/8.5/imagick").exists());
    assert!(!fs::read_to_string(home.join("php/8.5/etc/php-fpm.conf")).unwrap().contains("MAGICK"));
    assert!(ok(&["update", "--check"]).contains("everything is up to date (release r3)"));

    // Remove: ini and files go, php-fpm keeps running.
    assert!(ok(&["php", "ext", "remove", "8.5", "apcu"]).contains("apcu removed from PHP 8.5"));
    assert!(!dir.exists() && !home.join("php/8.5/etc/conf.d/devid-apcu.ini").exists());
    wait_for("8.5 running after remove", || fpm_pid(&home).is_some());
    assert!(ok(&["php", "ext", "list", "8.5"]).contains("catalog: none"));
    // The xdebug syntax still works next to the new words.
    assert!(ok(&["php", "ext", "8.5", "xdebug", "on"]).contains("xdebug on for PHP 8.5"));
}
