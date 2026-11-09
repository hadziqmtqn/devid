//! `devid setup`, `doctor` and `uninstall` (D-014). User-level steps run directly; the root steps
//! go through `devid helper` (sudo, one password prompt).

use crate::{HelperAction, helper, helper_plan, request};
use devid_daemon::{ca, config::Config, helper as h, install, paths::Paths, php, phpext, protocol::Request, webserver};
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

/// What `devid setup` installs besides the system parts (`--php`, `--server`, `--no-php`, `--no-server`).
pub struct Pick {
    pub php: Vec<String>,
    pub server: Option<String>,
    pub no_php: bool,
    pub no_server: bool,
    /// Install the desktop app (an installed one is always updated).
    pub gui: bool,
    /// Never ask for a password: a system setup step that needs sudo is skipped and reported, as
    /// for an update started from the desktop app with no terminal.
    pub no_sudo: bool,
}

/// The exit code of `setup --no-sudo` when it skipped a sudo step.
pub const NEEDS_PASSWORD_EXIT: u8 = 3;

/// The message for a skipped sudo step; the desktop app looks for "needs your password".
pub const NEEDS_PASSWORD: &str =
    "system setup (.test DNS, ports 80/443, CA trust) needs your password: run `devid setup` in Terminal";

/// PHP versions to install and the version to make the default. Without `--php`, the configured
/// default is installed only when no PHP is installed at all, so a re-run changes nothing.
fn php_plan(requested: &[String], default: &str, installed: &[String]) -> (Vec<String>, Option<String>) {
    if requested.is_empty() {
        return (if installed.is_empty() { vec![default.to_string()] } else { vec![] }, None);
    }
    let mut todo: Vec<String> = Vec::new();
    for v in requested {
        if !installed.contains(v) && !todo.contains(v) {
            todo.push(v.clone());
        }
    }
    (todo, Some(requested[0].clone()).filter(|v| v != default))
}

pub fn setup(paths: &Paths, dry_run: bool, pick: Pick) -> Result<(), String> {
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
        // D-040: the running commands start again in the new daemon. An older daemon does not know
        // the request, and none may run; either way there is nothing to note.
        let _ = request(paths, &Request::RememberRunning);
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
    let mut needs_password = false;
    if !dry_run && h::status(&plan).iter().all(|(_, ok)| *ok) {
        println!("  ok  system setup (.test DNS, ports 80/443, CA trust) already in place");
    } else if pick.no_sudo && !dry_run {
        // Everything else still installs; the caller finishes this step in Terminal.
        println!("  !!  {NEEDS_PASSWORD}");
        needs_password = true;
    } else {
        println!("  {} system setup (.test DNS, ports 80/443, CA trust) via sudo:", if dry_run { "-" } else { ".." });
        helper(HelperAction::Install { dry_run }, paths)?;
    }

    // After the sudo step, so the password prompt comes before the downloads.
    let config = Config::load_or_create(&paths.config()).map_err(|e| e.to_string())?;
    if !pick.no_php {
        let installed = php::installed(paths);
        let (todo, default) = php_plan(&pick.php, &config.default_php, &installed);
        if todo.is_empty() && default.is_none() {
            println!("  ok  PHP {} already installed", installed.join(", "));
        }
        for version in todo {
            step(dry_run, &format!("download and install PHP {version} (sha256-checked)"), || {
                request(paths, &Request::PhpInstall { version: version.clone(), from: None })
                    .map(|v| v["version"].as_str().unwrap_or_default().to_string())
            })?;
        }
        if let Some(version) = default {
            step(dry_run, &format!("make PHP {version} the default"), || {
                request(paths, &Request::PhpUse { version: version.clone() }).map(|_| String::new())
            })?;
        }
    }
    if !pick.no_server {
        let server = pick.server.unwrap_or(config.server.clone());
        if webserver::installed(paths, &server) {
            println!("  ok  {server} already installed");
        } else {
            step(dry_run, &format!("install {server}"), || {
                request(paths, &Request::ServerInstall { server: server.clone(), from: None }).map(|_| String::new())
            })?;
        }
        if server != config.server {
            step(dry_run, &format!("serve sites with {server}"), || {
                request(paths, &Request::ServerUse { server: server.clone() }).map(|_| String::new())
            })?;
        }
    }
    // The desktop app (Phase 7 G4): installed on request, and kept in step with devid once it is
    // there, so `devid self-update` (which runs the new binary's setup) updates it too.
    let app = install::app_bundle();
    if pick.gui || app.exists() {
        step(dry_run, &format!("install the Devid app ({}, sha256-checked)", app.display()), || {
            install_app(paths).map(|_| "open it from Spotlight or ~/Applications".into())
        })?;
    }
    if needs_password {
        return Err(NEEDS_PASSWORD.into());
    }
    if !dry_run {
        println!("\ndone. Next: `devid park ~/Projects` (or `devid link` in one project), then `devid doctor`.");
    }
    Ok(())
}

/// The release asset with Devid.app (scripts/bundle-app.sh).
const APP_ASSET: &str = "Devid-darwin-x86_64.zip";

/// Installs or updates ~/Applications/Devid.app from this devid's own release (`devid-v<version>`),
/// so the app and the daemon have the same version.
fn install_app(paths: &Paths) -> Result<std::path::PathBuf, String> {
    use devid_daemon::releases;
    let release = format!("devid-v{}", devid_daemon::VERSION);
    let get = |name: &str| releases::fetch_devid(paths, &release, name).map_err(|e| e.to_string());
    let sums = std::fs::read_to_string(get("checksums.txt")?).map_err(|e| e.to_string())?;
    let expected = checksum_for(&sums, APP_ASSET)?;
    let zip = get(APP_ASSET)?;
    let placed = place_app(&zip, &expected, &install::app_bundle());
    let _ = std::fs::remove_file(&zip);
    placed
}

/// Checks `zip` against `expected`, unpacks it next to `dest` and swaps it in whole: a running
/// copy keeps its files (it restarts into the new one, D-027), and a failed swap puts the old
/// bundle back.
fn place_app(zip: &Path, expected: &str, dest: &Path) -> Result<std::path::PathBuf, String> {
    let actual = devid_daemon::releases::sha256_hex(zip).map_err(|e| e.to_string())?;
    if actual != expected {
        return Err(format!("checksum mismatch for {APP_ASSET}; not installed"));
    }
    let dir = dest.parent().ok_or("no folder for the app")?;
    let (staging, old) = (dir.join(".Devid.app.new"), dir.join(".Devid.app.old"));
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir_all(&old);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let unzip = std::process::Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(zip).arg(&staging).output();
    let new = staging.join("Devid.app");
    if !matches!(&unzip, Ok(o) if o.status.success()) || !new.join("Contents/MacOS/devid-gui").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("{APP_ASSET} does not hold Devid.app"));
    }
    let had_old = dest.exists();
    if had_old {
        std::fs::rename(dest, &old).map_err(|e| e.to_string())?;
    }
    if let Err(e) = std::fs::rename(&new, dest) {
        if had_old {
            let _ = std::fs::rename(&old, dest);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&staging);
    Ok(dest.to_path_buf())
}

/// The SHA-256 of `asset` in a release's checksums.txt.
fn checksum_for(sums: &str, asset: &str) -> Result<String, String> {
    sums.lines()
        .find_map(|l| l.split_once("  ").filter(|(_, f)| *f == asset).map(|(h, _)| h.to_string()))
        .ok_or(format!("no checksum for {asset} in the release"))
}

/// One `doctor` result.
pub struct Check {
    pub ok: bool,
    pub what: String,
    pub fix: String,
}

/// Print the checks; fails if any problem was found.
pub fn doctor(paths: &Paths) -> Result<(), String> {
    let checks = doctor_checks(paths)?;
    for c in &checks {
        if c.ok {
            println!("ok       {}", c.what);
        } else {
            println!("PROBLEM  {}\n         fix: {}", c.what, c.fix);
        }
    }
    match checks.iter().filter(|c| !c.ok).count() {
        0 => {
            println!("\nall good");
            Ok(())
        }
        n => Err(format!("{n} problem(s) found")),
    }
}

/// Read-only health checks with a fix for every problem (also shown in the TUI Settings page).
pub fn doctor_checks(paths: &Paths) -> Result<Vec<Check>, String> {
    let mut checks = Vec::new();
    let mut check = |ok: bool, what: &str, fix: &str| {
        checks.push(Check { ok, what: what.to_string(), fix: fix.to_string() });
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
            let server = s["server"].as_str().unwrap_or("nginx");
            check(
                s["server_status"] == "running",
                &format!("{server} {}", s["server_status"].as_str().unwrap_or("?")),
                &format!("`devid server install {server}`; logs in ~/.devid/log/"),
            );
            let default = s["default_php"].as_str().unwrap_or("?");
            let running =
                s["php"].as_array().is_some_and(|l| l.iter().any(|p| p["default"] == true && p["status"] == "running"));
            check(running, &format!("default PHP {default} runs"), &format!("`devid php install {default}`"));
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

    for version in php::installed(paths) {
        let names: Vec<String> = phpext::installed(paths, &version).into_iter().map(|e| e.name).collect();
        if names.is_empty() {
            continue;
        }
        let want: Vec<&str> = names.iter().map(String::as_str).collect();
        let loads = phpext::preflight(paths, &version, &want);
        check(
            loads.is_ok(),
            &format!(
                "PHP {version} loads {}{}",
                names.join(", "),
                loads.err().map(|e| format!(" ({e})")).unwrap_or_default()
            ),
            &format!("`devid php ext install {version} <name>` again, or `devid php ext remove {version} <name>`"),
        );
    }

    let valet = std::process::Command::new("pgrep")
        .args(["-f", "/usr/local/opt/dnsmasq"])
        .output()
        .is_ok_and(|o| o.status.success());
    check(!valet, "Valet is stopped", "Valet still runs (its dnsmasq); once Devid works, stop it with `valet stop`");
    Ok(checks)
}

pub fn uninstall(paths: &Paths, dry_run: bool, purge: bool) -> Result<(), String> {
    println!("devid uninstall{}:", if dry_run { " (dry run, nothing changes)" } else { "" });
    let plan = helper_plan(paths)?;
    if dry_run || h::status(&plan).iter().any(|(_, ok)| *ok) {
        println!("  {} undo system setup via sudo:", if dry_run { "-" } else { ".." });
        helper(HelperAction::Uninstall { dry_run }, paths)?;
    }
    step(dry_run, "stop the daemon and remove the LaunchAgents", || {
        install::unload_agent();
        let _ = std::fs::remove_file(install::agent_plist_path());
        let _ = std::fs::remove_file(install::app_agent_plist_path());
        Ok(String::new())
    })?;
    let app = install::app_bundle();
    step(dry_run, &format!("quit and delete the Devid app ({})", app.display()), || {
        let bin = install::default_app();
        let _ = std::process::Command::new("/usr/bin/pkill").arg("-f").arg(&bin).output();
        let _ = std::fs::remove_dir_all(&app);
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

fn fetch_latest(paths: &Paths, name: &str) -> Result<std::path::PathBuf, String> {
    devid_daemon::releases::fetch_latest(paths, name).map_err(|e| e.to_string())
}

/// The newest released devid version (the daemon's periodic check uses the same code, D-028).
pub fn latest_devid(paths: &Paths) -> Result<String, String> {
    devid_daemon::releases::latest_devid(paths).map_err(|e| e.to_string())
}

/// `devid self-update`: download the newest devid from the `devid-latest` release, verify it against
/// that release's checksums.txt, then run its `setup`, which installs it and restarts the daemon.
pub fn self_update(paths: &Paths, check: bool, no_sudo: bool) -> Result<(), String> {
    use devid_daemon::releases;
    let get = |name: &str| fetch_latest(paths, name);
    let latest = latest_devid(paths)?;
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
    let expected = checksum_for(&sums, asset)?;
    let bin = get(asset)?;
    let actual = releases::sha256_hex(&bin).map_err(|e| e.to_string())?;
    if actual != expected {
        let _ = std::fs::remove_file(&bin);
        return Err(format!("checksum mismatch for {asset}; not installed"));
    }
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).map_err(|e| e.to_string())?;
    // The new binary installs itself (copy to ~/.devid/bin, LaunchAgent reload); same steps as setup.
    let mut setup = std::process::Command::new(&bin);
    setup.arg("setup");
    if no_sudo {
        setup.arg("--no-sudo");
    }
    let status = setup.status().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&bin);
    match status.code() {
        Some(0) => Ok(()),
        Some(code) if no_sudo && code == i32::from(NEEDS_PASSWORD_EXIT) => {
            Err(format!("devid {latest} is installed, but its {NEEDS_PASSWORD}"))
        }
        _ => Err(format!("devid {latest} setup failed ({status})")),
    }
}

#[cfg(test)]
mod tests {
    use super::php_plan;

    /// Devid.app is macOS only, and these tests use macOS's `ditto` (CI's Linux job has none).
    #[cfg(target_os = "macos")]
    mod app {
        use super::super::{checksum_for, place_app};
        use std::{fs, path::Path, process::Command};

        /// A fake Devid.app holding `marker`, zipped as the release does; returns the zip's SHA-256.
        fn app_zip(dir: &Path, marker: &str, with_binary: bool) -> (std::path::PathBuf, String) {
            let src = dir.join(format!("src-{marker}"));
            let bin_dir = src.join("Devid.app/Contents/MacOS");
            fs::create_dir_all(&bin_dir).unwrap();
            if with_binary {
                fs::write(bin_dir.join("devid-gui"), marker).unwrap();
            }
            let zip = dir.join(format!("{marker}.zip"));
            let ok = Command::new("/usr/bin/ditto")
                .args(["-c", "-k", "--keepParent"])
                .arg(src.join("Devid.app"))
                .arg(&zip)
                .status()
                .unwrap();
            assert!(ok.success());
            let sum = devid_daemon::releases::sha256_hex(&zip).unwrap();
            (zip, sum)
        }

        #[test]
        fn the_app_is_checked_unpacked_and_swapped_in_whole() {
            let dir = std::env::temp_dir().join(format!("devid-app-test-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            let dest = dir.join("Applications/Devid.app");
            fs::create_dir_all(dest.parent().unwrap()).unwrap();
            let binary = |d: &Path| fs::read_to_string(d.join("Contents/MacOS/devid-gui")).unwrap();

            let (v1, sum1) = app_zip(&dir, "v1", true);
            place_app(&v1, &sum1, &dest).unwrap();
            assert_eq!(binary(&dest), "v1");

            let (v2, sum2) = app_zip(&dir, "v2", true);
            place_app(&v2, &sum2, &dest).unwrap();
            assert_eq!(binary(&dest), "v2", "an update replaces the bundle");
            let left: Vec<_> = fs::read_dir(dest.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
            assert_eq!(left, ["Devid.app"], "no staging or backup folders left");

            assert!(place_app(&v1, &sum2, &dest).unwrap_err().contains("checksum mismatch"));
            let (empty, sum) = app_zip(&dir, "empty", false);
            assert!(place_app(&empty, &sum, &dest).unwrap_err().contains("does not hold Devid.app"));
            assert_eq!(binary(&dest), "v2", "a failed install keeps the app that was there");

            assert_eq!(
                checksum_for(&format!("{sum1}  Devid-darwin-x86_64.zip\n"), "Devid-darwin-x86_64.zip").unwrap(),
                sum1
            );
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn php_plan_installs_only_what_is_missing() {
        // No --php: the configured default on a fresh machine, nothing on a re-run.
        assert_eq!(php_plan(&[], "8.5", &[]), (s(&["8.5"]), None));
        assert_eq!(php_plan(&[], "8.5", &s(&["8.4"])), (vec![], None));
        // --php: missing ones (no duplicates); the first becomes the default unless it already is.
        assert_eq!(php_plan(&s(&["8.3", "8.4", "8.3"]), "8.5", &s(&["8.4"])), (s(&["8.3"]), Some("8.3".into())));
        assert_eq!(php_plan(&s(&["8.5"]), "8.5", &s(&["8.5"])), (vec![], None));
    }
}
