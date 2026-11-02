//! PHP manager end to end, with fake `php`/`php-fpm` scripts so it runs without real PHP builds.

mod common;

use common::*;
use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn php_install_use_supervise() {
    let root = temp_root("devid-php-it");
    let home = root.join("home");
    let (b84, b85) = (fake_build(&root, "8.4"), fake_build(&root, "8.5"));

    let mut daemon = start_daemon(&home);
    assert!(home.join("ca/cacert.pem").exists(), "cacert.pem is created at start");

    // Validation.
    assert!(stderr(&devid(&home, &["php", "use", "8.4"])).contains("not installed"));
    assert!(
        stderr(&devid(&home, &["php", "install", "8.1", "--from", b84.to_str().unwrap()])).contains("not supported")
    );
    assert!(stderr(&devid(&home, &["php", "install", "8.4", "--from", b85.to_str().unwrap()])).contains("not 8.4"));

    // Install both; the default (8.5) starts, 8.4 stays stopped.
    for (v, b) in [("8.4", &b84), ("8.5", &b85)] {
        assert!(devid(&home, &["php", "install", v, "--from", b.to_str().unwrap()]).status.success());
    }
    let ini = fs::read_to_string(home.join("php/8.5/etc/php.ini")).unwrap();
    assert!(ini.contains(&format!("curl.cainfo = {}", home.join("ca/cacert.pem").display())));
    assert!(fs::read_to_string(home.join("php/8.5/etc/php-fpm.conf")).unwrap().contains("pm = ondemand"));
    assert!(home.join("php/8.5/modules/xdebug.so").exists());

    wait_for("8.5 running", || proc_of(&json(&home, &["status", "--json"]), "8.5").0 == "running");
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.4").0, "stopped");

    // Switch the default: 8.4 starts, 8.5 stops, and the choice is saved.
    let (_, old85) = proc_of(&json(&home, &["status", "--json"]), "8.5");
    assert!(devid(&home, &["php", "use", "8.4"]).status.success());
    wait_for("8.4 running", || proc_of(&json(&home, &["status", "--json"]), "8.4").0 == "running");
    assert_eq!(proc_of(&json(&home, &["status", "--json"]), "8.5").0, "stopped");
    assert!(!alive(old85.unwrap()), "old 8.5 master was stopped");
    assert!(fs::read_to_string(home.join("config.toml")).unwrap().contains("default_php = \"8.4\""));

    // A crashed master is restarted with a new pid.
    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.4");
    Command::new("kill").args(["-9", &pid.unwrap().to_string()]).status().unwrap();
    wait_for("8.4 restarted", || {
        let (s, p) = proc_of(&json(&home, &["status", "--json"]), "8.4");
        s == "running" && p != pid
    });

    // Stopping the daemon stops PHP too.
    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.4");
    assert!(devid(&home, &["daemon", "stop"]).status.success());
    wait_for("daemon exit", || daemon.exited());
    wait_for("php-fpm exit", || !alive(pid.unwrap()));

    fs::remove_dir_all(&root).unwrap();
}

/// Regression: stopping right after php-fpm starts must not hang the daemon (signals lost at startup).
#[test]
fn stop_during_php_startup_does_not_hang() {
    let root = temp_root("devid-php-stop-it");
    let home = root.join("home");
    let build = fake_build_with(&root, "8.3", SLOW_START_FPM);

    let mut daemon = start_daemon(&home);
    assert!(devid(&home, &["php", "install", "8.3", "--from", build.to_str().unwrap()]).status.success());
    assert!(devid(&home, &["php", "use", "8.3"]).status.success());
    wait_for("8.3 running", || proc_of(&json(&home, &["status", "--json"]), "8.3").0 == "running");
    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.3");

    let stop = Instant::now();
    assert!(devid(&home, &["daemon", "stop"]).status.success());
    wait_for("daemon exit", || daemon.exited());
    assert!(stop.elapsed() < Duration::from_secs(5), "stop took {:?}", stop.elapsed());
    assert!(!alive(pid.unwrap()));

    fs::remove_dir_all(&root).unwrap();
}

/// launchd stops the LaunchAgent with SIGTERM: the daemon must stop PHP and clean up like `stop`.
#[test]
fn sigterm_stops_daemon_and_php() {
    let root = temp_root("devid-sigterm-it");
    let home = root.join("home");
    let build = fake_build(&root, "8.5");
    let mut daemon = start_daemon(&home);
    assert!(devid(&home, &["php", "install", "8.5", "--from", build.to_str().unwrap()]).status.success());
    wait_for("8.5 running", || proc_of(&json(&home, &["status", "--json"]), "8.5").0 == "running");
    let status = json(&home, &["status", "--json"]);
    let (daemon_pid, php_pid) = (status["pid"].as_u64().unwrap(), proc_of(&status, "8.5").1.unwrap());

    Command::new("kill").args(["-TERM", &daemon_pid.to_string()]).status().unwrap();
    wait_for("daemon exit", || daemon.exited());
    assert!(!alive(php_pid), "php-fpm stopped with the daemon");
    assert!(!home.join("run/devid.sock").exists(), "socket removed");
    fs::remove_dir_all(&root).unwrap();
}

/// T3: xdebug/pcov toggles write or remove conf.d/devid-<ext>.ini and restart php-fpm; `restart`.
#[test]
fn extensions_and_restart() {
    let root = temp_root("devid-php-ext-it");
    let home = root.join("home");
    let build = fake_build(&root, "8.5");
    fs::write(build.join("modules/pcov.so"), b"").unwrap();
    let daemon = start_daemon(&home);
    assert!(devid(&home, &["php", "install", "8.5", "--from", build.to_str().unwrap()]).status.success());
    wait_for("8.5 running", || proc_of(&json(&home, &["status", "--json"]), "8.5").0 == "running");
    let ext = |s: &serde_json::Value, e: &str| s["php"][0]["extensions"][e].as_bool().unwrap();
    assert!(!ext(&json(&home, &["status", "--json"]), "xdebug"), "off by default (D-008)");

    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.5");
    let o = devid(&home, &["php", "ext", "8.5", "xdebug", "on"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let ini = fs::read_to_string(home.join("php/8.5/etc/conf.d/devid-xdebug.ini")).unwrap();
    assert!(ini.contains(&format!("zend_extension={}", home.join("php/8.5/modules/xdebug.so").display())));
    assert!(ini.contains("xdebug.start_with_request=trigger"));
    wait_for("8.5 restarted", || {
        let (s, p) = proc_of(&json(&home, &["status", "--json"]), "8.5");
        s == "running" && p != pid
    });
    assert!(ext(&json(&home, &["status", "--json"]), "xdebug"));
    assert!(devid(&home, &["php", "ext", "8.5", "pcov", "on"]).status.success());
    assert!(fs::read_to_string(home.join("php/8.5/etc/conf.d/devid-pcov.ini")).unwrap().contains("pcov.so"));

    assert!(devid(&home, &["php", "ext", "8.5", "xdebug", "off"]).status.success());
    assert!(!home.join("php/8.5/etc/conf.d/devid-xdebug.ini").exists());
    assert!(devid(&home, &["php", "ext", "8.5", "xdebug", "off"]).status.success(), "off twice is fine");
    assert!(stderr(&devid(&home, &["php", "ext", "8.5", "opcache", "on"])).contains("unknown extension"));
    assert!(stderr(&devid(&home, &["php", "ext", "8.4", "xdebug", "on"])).contains("not installed"));

    let (_, pid) = proc_of(&json(&home, &["status", "--json"]), "8.5");
    assert!(devid(&home, &["restart", "php-8.5"]).status.success());
    wait_for("restarted", || {
        let (s, p) = proc_of(&json(&home, &["status", "--json"]), "8.5");
        s == "running" && p != pid
    });
    assert!(stderr(&devid(&home, &["restart", "php-7.4"])).contains("is not running"));

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
