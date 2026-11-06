//! User-level installation (D-014, macOS): the binary and shims in `~/.devid/bin`, a PATH line in
//! the shell rc file, and a LaunchAgent that keeps the daemon running. Root steps live in helper.rs.

use crate::paths::{Paths, user_home};
use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

pub const AGENT_LABEL: &str = "dev.devid.daemon";
/// Names that run through `devid` itself (argv[0] dispatch in the CLI).
pub const SHIMS: [&str; 2] = ["php", "composer"];
const RC_MARKER: &str = "# added by devid";
const COMPOSER_URL: &str = "https://getcomposer.org/download/latest-stable/composer.phar";

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

pub fn bin_dir(paths: &Paths) -> PathBuf {
    paths.home.join("bin")
}

pub fn installed_bin(paths: &Paths) -> PathBuf {
    bin_dir(paths).join("devid")
}

pub fn composer_phar(paths: &Paths) -> PathBuf {
    paths.home.join("composer/composer.phar")
}

pub fn agent_plist_path() -> PathBuf {
    user_home().join("Library/LaunchAgents").join(format!("{AGENT_LABEL}.plist"))
}

/// The desktop app's login item (D-031), separate from the daemon's LaunchAgent.
pub const APP_LABEL: &str = "dev.devid.app";

pub fn app_agent_plist_path() -> PathBuf {
    user_home().join("Library/LaunchAgents").join(format!("{APP_LABEL}.plist"))
}

/// Where the app is installed (`Devid.app` in ~/Applications, Phase 7 G4).
pub fn default_app() -> PathBuf {
    user_home().join("Applications/Devid.app/Contents/MacOS/devid-gui")
}

/// Opens the desktop app's menu-bar icon at login (`--background`), or stops doing so.
/// Only writes or removes the plist: loading it now would start a second copy next to a running
/// app; launchd reads it at the next login. Returns the app it opens, if any.
pub fn set_app_agent(enabled: bool, app: Option<&str>) -> Result<Option<String>, String> {
    let plist = app_agent_plist_path();
    if !enabled {
        return match fs::remove_file(&plist) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(format!("{}: {e}", plist.display())),
            _ => Ok(None),
        };
    }
    let app = app.map(PathBuf::from).unwrap_or_else(default_app);
    if !app.is_file() {
        return Err(format!("the Devid app is not installed at {}", app.display()));
    }
    let app = app.display().to_string();
    fs::create_dir_all(plist.parent().expect("LaunchAgents folder")).map_err(|e| e.to_string())?;
    fs::write(&plist, app_agent_plist(&app)).map_err(|e| format!("{}: {e}", plist.display()))?;
    Ok(Some(app))
}

/// The app the login item opens, if there is one.
pub fn app_agent_program() -> Option<String> {
    let text = fs::read_to_string(app_agent_plist_path()).ok()?;
    let args = &text[text.find("<key>ProgramArguments</key>")?..];
    let start = args.find("<string>")? + "<string>".len();
    let end = start + args[start..].find("</string>")?;
    Some(xml_unescape(&args[start..end]))
}

fn app_agent_plist(app: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{APP_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--background</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <!-- The menu bar exists only in the login (Aqua) session. -->
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
"#,
        xml_escape(app)
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// The rc file for the login shell: zsh (the macOS default) or bash.
pub fn rc_file() -> PathBuf {
    let shell = std::env::var("SHELL").unwrap_or_default();
    user_home().join(if shell.ends_with("bash") { ".bash_profile" } else { ".zshrc" })
}

pub fn rc_line(paths: &Paths) -> String {
    format!("export PATH=\"{}:$PATH\" {RC_MARKER}", bin_dir(paths).display())
}

/// Append the PATH line once. Appending puts Devid's bin in front of earlier PATH entries, so its
/// `php` wins over Homebrew's. Returns whether the file changed.
pub fn ensure_rc_line(rc: &Path, line: &str) -> io::Result<bool> {
    let text = fs::read_to_string(rc).unwrap_or_default();
    if text.lines().any(|l| l == line) {
        return Ok(false);
    }
    let mut new = text;
    if !new.is_empty() && !new.ends_with('\n') {
        new.push('\n');
    }
    new.push_str(line);
    new.push('\n');
    fs::write(rc, new)?;
    Ok(true)
}

/// Remove every line Devid added (marked), leaving the rest untouched. Returns whether it changed.
pub fn remove_rc_line(rc: &Path) -> io::Result<bool> {
    let Ok(text) = fs::read_to_string(rc) else { return Ok(false) };
    let kept: Vec<&str> = text.lines().filter(|l| !l.ends_with(RC_MARKER)).collect();
    if kept.len() == text.lines().count() {
        return Ok(false);
    }
    let mut new = kept.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    fs::write(rc, new)?;
    Ok(true)
}

/// Copy the running binary to `~/.devid/bin/devid` (unless it already runs from there) and point
/// the shims at it.
pub fn install_binary(paths: &Paths) -> io::Result<()> {
    let target = installed_bin(paths);
    fs::create_dir_all(bin_dir(paths))?;
    let current = std::env::current_exe()?;
    if fs::canonicalize(&current).ok() != fs::canonicalize(&target).ok() {
        let tmp = target.with_extension("new");
        fs::copy(&current, &tmp)?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        fs::rename(tmp, &target)?;
    }
    for shim in SHIMS {
        let link = bin_dir(paths).join(shim);
        let _ = fs::remove_file(&link);
        // A hard link, not a symlink: PHP resolves symlinks for PHP_BINARY, and child processes
        // (`artisan serve`, parallel tests) must come back through the shim, not run `devid`.
        fs::hard_link(&target, &link)?;
    }
    Ok(())
}

pub fn agent_plist(paths: &Paths) -> String {
    let log = paths.log_dir().join("daemon.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>daemon</string>
        <string>run</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>DEVID_HOME</key>
        <string>{home}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <!-- Restart after a crash, but not after `devid daemon stop` (a clean exit). -->
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <!-- No ProcessType: "Background" ran nginx and php-fpm at the lowest CPU priority, so pages
         took seconds whenever the Mac was busy. -->
    <key>StandardErrorPath</key>
    <string>{log}</string>
    <key>StandardOutPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        bin = installed_bin(paths).display(),
        home = paths.home.display(),
        log = log.display(),
    )
}

fn gui_domain() -> String {
    // SAFETY: getuid has no preconditions.
    format!("gui/{}", unsafe { libc::getuid() })
}

/// Write the LaunchAgent and (re)load it so it starts the daemon now and at every login.
pub fn load_agent(paths: &Paths) -> io::Result<()> {
    let plist = agent_plist_path();
    fs::create_dir_all(plist.parent().expect("plist has a parent"))?;
    fs::create_dir_all(paths.log_dir())?;
    fs::write(&plist, agent_plist(paths))?;
    unload_agent();
    // Even after the old job is gone, launchd can briefly refuse the new one ("Bootstrap failed: 5:
    // Input/output error"); retry instead of leaving the sites down.
    let mut last = String::new();
    for _ in 0..10 {
        let out = Command::new("/bin/launchctl").arg("bootstrap").arg(gui_domain()).arg(&plist).output()?;
        if out.status.success() {
            return Ok(());
        }
        last = String::from_utf8_lossy(&out.stderr).trim().to_string();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Err(err(format!(
        "launchctl bootstrap failed: {last}; retry with `launchctl bootstrap {} {}`",
        gui_domain(),
        plist.display()
    )))
}

/// Stop the agent (launchd sends SIGTERM; the daemon stops PHP and nginx cleanly) and wait until
/// launchd has really released it: `bootout` returns before teardown finishes, and a `bootstrap`
/// right after it fails (this took the owner's sites down during the first upgrade).
pub fn unload_agent() {
    let _ = Command::new("/bin/launchctl").arg("bootout").arg(format!("{}/{AGENT_LABEL}", gui_domain())).output();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while agent_loaded() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn agent_loaded() -> bool {
    Command::new("/bin/launchctl")
        .arg("print")
        .arg(format!("{}/{AGENT_LABEL}", gui_domain()))
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Download composer.phar with its published SHA-256, if missing.
// shortcut: trusts getcomposer.org over HTTPS plus its sha256; verifying Composer's signature comes with the Phase 5 pipeline.
pub fn ensure_composer(paths: &Paths) -> io::Result<bool> {
    let target = composer_phar(paths);
    if target.is_file() {
        return Ok(false);
    }
    fs::create_dir_all(target.parent().expect("phar has a parent"))?;
    let tmp = target.with_extension("download");
    let fetch = |url: &str, out: &Path| -> io::Result<()> {
        let status =
            Command::new("/usr/bin/curl").args(["-fsSL", "--proto", "=https", "-o"]).arg(out).arg(url).status()?;
        if status.success() { Ok(()) } else { Err(err(format!("download failed: {url}"))) }
    };
    fetch(COMPOSER_URL, &tmp)?;
    let sum_file = tmp.with_extension("sha256");
    fetch(&format!("{COMPOSER_URL}.sha256sum"), &sum_file)?;
    let expected = fs::read_to_string(&sum_file)?.split_whitespace().next().unwrap_or_default().to_string();
    let actual = Command::new("/usr/bin/shasum").args(["-a", "256"]).arg(&tmp).output()?;
    let actual = String::from_utf8_lossy(&actual.stdout).split_whitespace().next().unwrap_or_default().to_string();
    let _ = fs::remove_file(&sum_file);
    if expected.len() != 64 || expected != actual {
        let _ = fs::remove_file(&tmp);
        return Err(err("composer.phar checksum mismatch; not installed"));
    }
    fs::rename(tmp, target)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shims_are_hard_links_to_the_installed_binary() {
        use std::os::unix::fs::MetadataExt;
        let paths = Paths { home: std::env::temp_dir().join(format!("devid-shim-test-{}", std::process::id())) };
        for _ in 0..2 {
            install_binary(&paths).unwrap(); // twice: re-linking an existing install works
            let ino = fs::metadata(installed_bin(&paths)).unwrap().ino();
            for shim in SHIMS {
                let link = bin_dir(&paths).join(shim);
                assert!(!fs::symlink_metadata(&link).unwrap().is_symlink(), "{shim} is not a symlink");
                assert_eq!(fs::metadata(&link).unwrap().ino(), ino, "{shim} is the installed devid");
            }
        }
        fs::remove_dir_all(&paths.home).unwrap();
    }

    #[test]
    fn rc_line_is_added_once_and_removed_cleanly() {
        let dir = std::env::temp_dir().join(format!("devid-rc-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rc = dir.join(".zshrc");
        fs::write(&rc, "export FOO=1\nexport PATH=\"/x:$PATH\"").unwrap(); // no trailing newline
        let paths = Paths { home: PathBuf::from("/Users/me/.devid") };
        let line = rc_line(&paths);
        assert_eq!(line, "export PATH=\"/Users/me/.devid/bin:$PATH\" # added by devid");

        assert!(ensure_rc_line(&rc, &line).unwrap());
        assert!(!ensure_rc_line(&rc, &line).unwrap(), "idempotent");
        let text = fs::read_to_string(&rc).unwrap();
        assert!(text.ends_with(&format!("$PATH\"\n{line}\n")), "appended last so Devid's php wins");

        assert!(remove_rc_line(&rc).unwrap());
        assert_eq!(fs::read_to_string(&rc).unwrap(), "export FOO=1\nexport PATH=\"/x:$PATH\"\n");
        assert!(!remove_rc_line(&rc).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn app_login_item_round_trips_its_path() {
        let plist = app_agent_plist("/Users/a&b/Devid.app/Contents/MacOS/devid-gui");
        assert!(plist.contains("<string>/Users/a&amp;b/Devid.app/Contents/MacOS/devid-gui</string>"));
        assert!(plist.contains("<string>--background</string>") && plist.contains("<string>Aqua</string>"));
        let args = &plist[plist.find("<key>ProgramArguments</key>").unwrap()..];
        let start = args.find("<string>").unwrap() + 8;
        let end = start + args[start..].find("</string>").unwrap();
        assert_eq!(xml_unescape(&args[start..end]), "/Users/a&b/Devid.app/Contents/MacOS/devid-gui");
    }

    #[test]
    fn agent_plist_runs_the_installed_binary() {
        let paths = Paths { home: PathBuf::from("/Users/me/.devid") };
        let plist = agent_plist(&paths);
        assert!(plist.contains("<string>/Users/me/.devid/bin/devid</string>"));
        assert!(plist.contains("<string>dev.devid.daemon</string>"));
        assert!(plist.contains("<key>SuccessfulExit</key>\n        <false/>"), "no restart after a clean stop");
        assert!(plist.contains("/Users/me/.devid/log/daemon.log"));
        assert!(!plist.contains("<key>ProcessType</key>"), "sites must not run at background priority");
    }
}
