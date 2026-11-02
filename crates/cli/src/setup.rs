//! `devid setup`, `doctor` and `uninstall` (D-014). User-level steps run directly; the root steps
//! go through `devid helper` (sudo, one password prompt).

use crate::{HelperAction, helper, helper_plan, request};
use devid_daemon::{ca, helper as h, install, paths::Paths, protocol::Request};
use std::{
    net::ToSocketAddrs,
    path::Path,
    thread::sleep,
    time::{Duration, Instant},
};

fn step(dry_run: bool, what: &str, f: impl FnOnce() -> Result<String, String>) -> Result<(), String> {
    if dry_run {
        println!("  - {what}");
        return Ok(());
    }
    let note = f().map_err(|e| format!("{what}: {e}"))?;
    println!("  ok  {what}{}", if note.is_empty() { String::new() } else { format!(" ({note})") });
    Ok(())
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        sleep(Duration::from_millis(100));
    }
    false
}

pub fn setup(paths: &Paths, dry_run: bool) -> Result<(), String> {
    println!("devid setup{}:", if dry_run { " (dry run, nothing changes)" } else { "" });
    let bin = install::installed_bin(paths);
    step(dry_run, &format!("install {} and php/composer shims", bin.display()), || {
        install::install_binary(paths).map(|_| String::new()).map_err(|e| e.to_string())
    })?;
    let rc = install::rc_file();
    step(dry_run, &format!("put {} first on PATH in {}", install::bin_dir(paths).display(), rc.display()), || {
        install::ensure_rc_line(&rc, &install::rc_line(paths))
            .map(|changed| if changed { "open a new terminal".into() } else { "already there".into() })
            .map_err(|e| e.to_string())
    })?;
    step(dry_run, "run the daemon as a LaunchAgent (now and at every login)", || {
        // A daemon started by hand would hold the socket; hand over to launchd.
        if !install::agent_loaded() && request(paths, &Request::Stop).is_ok() {
            wait_until(Duration::from_secs(15), || !paths.socket().exists());
        }
        install::load_agent(paths).map_err(|e| e.to_string())?;
        if wait_until(Duration::from_secs(15), || request(paths, &Request::Status).is_ok()) {
            Ok(install::agent_plist_path().display().to_string())
        } else {
            Err(format!("daemon did not start; see {}", paths.log_dir().join("daemon.log").display()))
        }
    })?;
    step(dry_run, "download composer.phar (sha256-checked)", || {
        install::ensure_composer(paths)
            .map(|new| if new { "downloaded".into() } else { "already there".into() })
            .map_err(|e| e.to_string())
    })?;

    let plan = helper_plan(paths)?;
    if !dry_run && h::status(&plan).iter().all(|(_, ok)| *ok) {
        println!("  ok  system setup (.test DNS, ports 80/443, CA trust) already in place");
    } else {
        println!("  {} system setup (.test DNS, ports 80/443, CA trust) via sudo:", if dry_run { "-" } else { ".." });
        helper(HelperAction::Install { dry_run }, paths)?;
    }
    if !dry_run {
        println!(
            "\ndone. Next: `devid php install <ver> --from <dir>` if needed, `devid park ~/Projects`, `devid doctor`."
        );
    }
    Ok(())
}

/// Read-only health check with a fix for every problem.
pub fn doctor(paths: &Paths) -> Result<(), String> {
    let mut problems = 0;
    let mut check = |ok: bool, what: &str, fix: &str| {
        if ok {
            println!("ok       {what}");
        } else {
            problems += 1;
            println!("PROBLEM  {what}\n         fix: {fix}");
        }
    };

    let on_path = |name: &str| {
        std::env::var_os("PATH")
            .and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|c| c.is_file()))
            .map(|p| p.display().to_string())
    };
    let ours = |name: &str| on_path(name).is_some_and(|p| Path::new(&p).starts_with(install::bin_dir(paths)));
    // The PATH line may already be in the rc file but not loaded by this (older) terminal.
    let rc = install::rc_file();
    let rc_has_line = std::fs::read_to_string(&rc).is_ok_and(|t| t.lines().any(|l| l == install::rc_line(paths)));
    let path_fix = if rc_has_line {
        format!("{} already has Devid's PATH line; open a new terminal or run `exec zsh`", rc.display())
    } else {
        "run `devid setup`, then open a new terminal".to_string()
    };
    check(ours("devid"), "devid on PATH is ~/.devid/bin/devid", &path_fix);
    check(
        ours("php"),
        &format!("php on PATH is Devid's shim (now: {})", on_path("php").unwrap_or_else(|| "none".into())),
        &format!("{path_fix} (Devid's bin must come before Homebrew)"),
    );
    check(install::agent_loaded(), "LaunchAgent dev.devid.daemon is loaded", "run `devid setup`");

    match request(paths, &Request::Status) {
        Ok(s) => {
            check(true, "daemon answers", "");
            check(
                s["dns"].as_str().is_some_and(|d| d.starts_with("listening")),
                &format!("DNS {}", s["dns"].as_str().unwrap_or("?")),
                "another program holds the DNS port; change dns_port in ~/.devid/config.toml",
            );
            check(
                s["server_status"] == "running",
                &format!("nginx {}", s["server_status"].as_str().unwrap_or("?")),
                "`devid server install nginx --from <file>`; logs in ~/.devid/log/",
            );
            let default = s["default_php"].as_str().unwrap_or("?");
            let running =
                s["php"].as_array().is_some_and(|l| l.iter().any(|p| p["default"] == true && p["status"] == "running"));
            check(
                running,
                &format!("default PHP {default} runs"),
                &format!("`devid php install {default} --from <dir>`"),
            );
        }
        Err(e) => check(false, "daemon answers", &format!("{e}; or `devid setup`")),
    }

    let plan = helper_plan(paths)?;
    for (name, ok) in h::status(&plan) {
        check(ok, &format!("system: {name}"), "run `devid helper install`");
    }
    let probe = format!("devid-doctor.{}:443", plan.tld);
    let resolves = probe.to_socket_addrs().is_ok_and(|mut a| a.any(|a| a.ip().is_loopback()));
    check(resolves, &format!("*.{} resolves to 127.0.0.1 through macOS", plan.tld), "run `devid helper install`");
    check(ca::ca_cert(paths).is_file(), "Devid CA exists", "start the daemon once");

    let valet = std::process::Command::new("pgrep")
        .args(["-f", "/usr/local/opt/dnsmasq"])
        .output()
        .is_ok_and(|o| o.status.success());
    check(!valet, "Valet is stopped", "Valet still runs (its dnsmasq); once Devid works, stop it with `valet stop`");

    if problems == 0 {
        println!("\nall good");
        Ok(())
    } else {
        Err(format!("{problems} problem(s) found"))
    }
}

pub fn uninstall(paths: &Paths, dry_run: bool, purge: bool) -> Result<(), String> {
    println!("devid uninstall{}:", if dry_run { " (dry run, nothing changes)" } else { "" });
    let plan = helper_plan(paths)?;
    if dry_run || h::status(&plan).iter().any(|(_, ok)| *ok) {
        println!("  {} undo system setup via sudo:", if dry_run { "-" } else { ".." });
        helper(HelperAction::Uninstall { dry_run }, paths)?;
    }
    step(dry_run, "stop the daemon and remove the LaunchAgent", || {
        install::unload_agent();
        let _ = std::fs::remove_file(install::agent_plist_path());
        Ok(String::new())
    })?;
    let rc = install::rc_file();
    step(dry_run, &format!("remove Devid's PATH line from {}", rc.display()), || {
        install::remove_rc_line(&rc).map(|_| String::new()).map_err(|e| e.to_string())
    })?;
    let target = if purge { paths.home.clone() } else { install::bin_dir(paths) };
    step(dry_run, &format!("delete {}", target.display()), || {
        let _ = std::fs::remove_dir_all(&target);
        Ok(String::new())
    })?;
    if !purge {
        println!(
            "  kept {} (PHP, config, CA, certificates); `devid uninstall --purge` removes it",
            paths.home.display()
        );
    }
    Ok(())
}

/// `devid self-update`: download the newest devid from the `devid-latest` release, verify it against
/// that release's checksums.txt, then run its `setup`, which installs it and restarts the daemon.
pub fn self_update(paths: &Paths, check: bool) -> Result<(), String> {
    use devid_daemon::releases;
    let base = format!("{}/devid-latest", releases::release_base());
    let dir = paths.home.join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let get = |name: &str| -> Result<std::path::PathBuf, String> {
        let dest = dir.join(format!("self-update-{name}"));
        releases::fetch(&format!("{base}/{name}"), &dest).map_err(|e| e.to_string())?;
        Ok(dest)
    };

    let latest = std::fs::read_to_string(get("devid-version.txt")?).map_err(|e| e.to_string())?.trim().to_string();
    let current = devid_daemon::VERSION;
    if !releases::newer(&latest, current) {
        println!("devid {current} is the newest version");
        return Ok(());
    }
    if check {
        println!("devid {latest} is available (installed: {current}); run `devid self-update`");
        return Ok(());
    }

    let asset = if cfg!(target_arch = "aarch64") { "devid-darwin-arm64" } else { "devid-darwin-x86_64" };
    println!("downloading devid {latest} (sha256-checked)…");
    let sums = std::fs::read_to_string(get("checksums.txt")?).map_err(|e| e.to_string())?;
    let expected = sums
        .lines()
        .find_map(|l| l.split_once("  ").filter(|(_, f)| *f == asset).map(|(h, _)| h.to_string()))
        .ok_or(format!("no checksum for {asset} in the release"))?;
    let bin = get(asset)?;
    let actual = releases::sha256_hex(&bin).map_err(|e| e.to_string())?;
    if actual != expected {
        let _ = std::fs::remove_file(&bin);
        return Err(format!("checksum mismatch for {asset}; not installed"));
    }
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).map_err(|e| e.to_string())?;
    // The new binary installs itself (copy to ~/.devid/bin, LaunchAgent reload); same steps as setup.
    let status = std::process::Command::new(&bin).arg("setup").status().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&bin);
    if status.success() { Ok(()) } else { Err(format!("devid {latest} setup failed ({status})")) }
}
