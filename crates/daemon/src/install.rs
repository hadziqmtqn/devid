//! User-level installation (D-014): the binary and shims in `~/.devid/bin`, a PATH line in the
//! shell's startup files, and composer.phar. The login service that keeps the daemon running (a
//! LaunchAgent or a systemd user unit) is in platform/, the root step in helper.rs.

use crate::paths::{Paths, user_home};
use std::{
    ffi::CStr,
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

/// Names that run through `devid` itself (argv[0] dispatch in the CLI): the PHP of the current
/// folder and composer, then (D-053) one PHP version each, and Devid's own nginx, Caddy and Mailpit.
pub const SHIMS: [&str; 8] = ["php", "composer", "php8.3", "php8.4", "php8.5", "nginx", "caddy", "mailpit"];
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

/// The desktop app's login item (D-031), separate from the daemon's LaunchAgent.
pub const APP_LABEL: &str = "dev.devid.app";

pub fn app_agent_plist_path() -> PathBuf {
    user_home().join("Library/LaunchAgents").join(format!("{APP_LABEL}.plist"))
}

/// The installed desktop app (Phase 7 G4): ~/Applications/Devid.app, no sudo.
pub fn app_bundle() -> PathBuf {
    user_home().join("Applications/Devid.app")
}

/// The app's binary, which the login item runs.
pub fn default_app() -> PathBuf {
    app_bundle().join("Contents/MacOS/devid-gui")
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

/// The user's login shell from the user database. The daemon runs under launchd, whose
/// environment need not have SHELL; $SHELL is the fallback. DEVID_LOGIN_SHELL comes first, for the
/// tests and for anyone whose account names a shell they do not use.
fn login_shell() -> String {
    if let Some(shell) = std::env::var("DEVID_LOGIN_SHELL").ok().filter(|s| !s.is_empty()) {
        return shell;
    }
    let mut buf = vec![0 as libc::c_char; 4096];
    // SAFETY: getpwuid_r (thread-safe, the daemon has threads) fills `pw` with strings that point
    // into `buf`, which outlives the read; `found` is null when there is no record.
    let shell = unsafe {
        let mut pw: libc::passwd = std::mem::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let rc = libc::getpwuid_r(libc::getuid(), &mut pw, buf.as_mut_ptr(), buf.len(), &mut found);
        if rc != 0 || found.is_null() || pw.pw_shell.is_null() {
            None
        } else {
            Some(CStr::from_ptr(pw.pw_shell).to_string_lossy().into_owned())
        }
    };
    shell.filter(|s| !s.is_empty()).or_else(|| std::env::var("SHELL").ok()).unwrap_or_default()
}

/// The startup files that put `~/.devid/bin` on PATH (D-053) for the login shell.
pub fn rc_files() -> Vec<PathBuf> {
    rc_files_for(&login_shell(), &user_home())
}

/// fish gets a file of Devid's own in conf.d on every OS, which fish reads in every shell,
/// interactive or not (D-059); bash and zsh get the OS's files (`platform::rc_files_for`).
pub fn rc_files_for(shell: &str, home: &Path) -> Vec<PathBuf> {
    if shell.ends_with("fish") {
        vec![home.join(".config/fish/conf.d/devid.fish")]
    } else {
        crate::platform::rc_files_for(shell, home)
    }
}

/// The PATH line for one startup file: `fish_add_path` in fish's, `export PATH` in the others.
pub fn rc_line(paths: &Paths, rc: &Path) -> String {
    let bin = bin_dir(paths).display().to_string();
    if rc.extension().is_some_and(|e| e == "fish") {
        // --move puts it first even when PATH already has it further on; --path changes PATH itself,
        // so fish writes no universal variable (Phase 13 A1).
        let quoted = format!("'{}'", bin.replace('\\', "\\\\").replace('\'', "\\'"));
        format!("fish_add_path --global --move --path {quoted} {RC_MARKER}")
    } else {
        format!("export PATH=\"{bin}:$PATH\" {RC_MARKER}")
    }
}

/// Whether `rc` has this install's exact PATH line.
pub fn rc_has_line(paths: &Paths, rc: &Path) -> bool {
    let line = rc_line(paths, rc);
    fs::read_to_string(rc).is_ok_and(|t| t.lines().any(|l| l == line))
}

/// The commands `~/.devid/bin` holds: devid and its shims (D-053).
pub fn commands() -> Vec<&'static str> {
    std::iter::once("devid").chain(SHIMS).collect()
}

/// Put Devid's bin on PATH in every startup file of the login shell, or take it out of them
/// (D-053). A terminal that is already open keeps its PATH until a new one opens. Returns the
/// files that changed.
pub fn set_on_path(paths: &Paths, on: bool) -> io::Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    for rc in rc_files() {
        let did = if on { ensure_rc_line(&rc, &rc_line(paths, &rc)) } else { remove_rc_line(&rc) };
        if did.map_err(|e| err(format!("{}: {e}", rc.display())))? {
            changed.push(rc);
        }
    }
    Ok(changed)
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
    if let Some(dir) = rc.parent() {
        // fish's conf.d may not exist yet.
        fs::create_dir_all(dir)?;
    }
    fs::write(rc, new)?;
    Ok(true)
}

/// Remove every line Devid added (marked), leaving the rest untouched; a file left with nothing
/// in it was Devid's own and goes. Returns whether it changed.
pub fn remove_rc_line(rc: &Path) -> io::Result<bool> {
    let Ok(text) = fs::read_to_string(rc) else { return Ok(false) };
    let kept: Vec<&str> = text.lines().filter(|l| !l.ends_with(RC_MARKER)).collect();
    if kept.len() == text.lines().count() {
        return Ok(false);
    }
    if kept.iter().all(|l| l.trim().is_empty()) {
        fs::remove_file(rc)?;
        return Ok(true);
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
    let actual = crate::releases::sha256_hex(&tmp)?;
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
    fn every_supported_php_has_a_versioned_command() {
        for v in crate::php::SUPPORTED {
            assert!(SHIMS.contains(&format!("php{v}").as_str()), "php{v} is a shim");
        }
        assert_eq!(commands()[0], "devid");
        assert!(commands().contains(&"mailpit"));
    }

    #[test]
    fn rc_line_is_added_once_and_removed_cleanly() {
        let dir = std::env::temp_dir().join(format!("devid-rc-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rc = dir.join(".zshrc");
        fs::write(&rc, "export FOO=1\nexport PATH=\"/x:$PATH\"").unwrap(); // no trailing newline
        let paths = Paths { home: PathBuf::from("/Users/me/.devid") };
        let line = rc_line(&paths, &rc);
        assert_eq!(line, "export PATH=\"/Users/me/.devid/bin:$PATH\" # added by devid");

        assert!(ensure_rc_line(&rc, &line).unwrap());
        assert!(!ensure_rc_line(&rc, &line).unwrap(), "idempotent");
        let text = fs::read_to_string(&rc).unwrap();
        assert!(text.ends_with(&format!("$PATH\"\n{line}\n")), "appended last so Devid's php wins");

        assert!(remove_rc_line(&rc).unwrap());
        assert_eq!(fs::read_to_string(&rc).unwrap(), "export FOO=1\nexport PATH=\"/x:$PATH\"\n");
        assert!(!remove_rc_line(&rc).unwrap());

        let own = dir.join(".zshenv"); // a file Devid created goes again
        assert!(ensure_rc_line(&own, &line).unwrap());
        assert!(remove_rc_line(&own).unwrap());
        assert!(!own.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// fish (D-059): Devid's own file in conf.d, on every OS, made with its folder and deleted again.
    #[test]
    fn fish_gets_its_own_conf_d_file() {
        let home = std::env::temp_dir().join(format!("devid-fish-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let rcs = rc_files_for("/usr/bin/fish", &home);
        assert_eq!(rcs, [home.join(".config/fish/conf.d/devid.fish")]);
        assert_eq!(rc_files_for("/bin/bash", &home), crate::platform::rc_files_for("/bin/bash", &home));

        let paths = Paths { home: PathBuf::from("/home/me/.devid") };
        let fish = &rcs[0];
        let line = rc_line(&paths, fish);
        assert_eq!(line, "fish_add_path --global --move --path '/home/me/.devid/bin' # added by devid");
        assert!(!rc_has_line(&paths, fish));
        assert!(ensure_rc_line(fish, &line).unwrap(), "conf.d is made too");
        assert!(rc_has_line(&paths, fish));
        assert_eq!(fs::read_to_string(fish).unwrap(), format!("{line}\n"));
        assert!(remove_rc_line(fish).unwrap());
        assert!(!fish.exists(), "Devid's own file goes");
        let odd = Paths { home: PathBuf::from("/home/o'neil/.devid") };
        assert_eq!(
            rc_line(&odd, fish),
            r"fish_add_path --global --move --path '/home/o\'neil/.devid/bin' # added by devid"
        );
        fs::remove_dir_all(&home).unwrap();
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
}
