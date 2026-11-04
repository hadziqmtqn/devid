//! PHP versions under `~/.devid/php/<ver>/`: install, list, and the files php-fpm runs with (D-008, D-009).

use crate::{paths::Paths, supervisor::Spec};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// D-009: the only versions Devid manages.
pub const SUPPORTED: [&str; 3] = ["8.3", "8.4", "8.5"];

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

pub fn check_supported(version: &str) -> io::Result<()> {
    if SUPPORTED.contains(&version) {
        Ok(())
    } else {
        Err(err(format!("PHP {version} is not supported; choose one of {}", SUPPORTED.join(", "))))
    }
}

/// "8.4" -> "84", used in socket, pid and log file names.
pub fn tag(version: &str) -> String {
    version.replace('.', "")
}

pub fn dir(paths: &Paths, version: &str) -> PathBuf {
    paths.home.join("php").join(version)
}

pub fn fpm_bin(paths: &Paths, version: &str) -> PathBuf {
    dir(paths, version).join("bin/php-fpm")
}

pub fn etc(paths: &Paths, version: &str) -> PathBuf {
    dir(paths, version).join("etc")
}

pub fn socket(paths: &Paths, version: &str) -> PathBuf {
    paths.run_dir().join(format!("php{}.sock", tag(version)))
}

pub fn pid_file(paths: &Paths, version: &str) -> PathBuf {
    paths.run_dir().join(format!("php{}-fpm.pid", tag(version)))
}

/// Installed versions, sorted, newest last.
pub fn installed(paths: &Paths) -> Vec<String> {
    let mut found: Vec<String> =
        SUPPORTED.iter().filter(|v| fpm_bin(paths, v).is_file()).map(|v| v.to_string()).collect();
    found.sort();
    found
}

/// Install `php`, `php-fpm` and `modules/*.so` from a build folder (spike/php/out/<ver>).
// shortcut: installs from a local folder only; downloading release assets comes with the Phase 5 pipeline.
pub fn install(paths: &Paths, version: &str, from: &Path) -> io::Result<()> {
    check_supported(version)?;
    for bin in ["php", "php-fpm"] {
        if !from.join(bin).is_file() {
            return Err(err(format!("{} not found", from.join(bin).display())));
        }
    }
    let reported = php_version(&from.join("php"))?;
    if reported != version {
        return Err(err(format!("{} is PHP {reported}, not {version}", from.display())));
    }

    // Copy into a temp dir and rename, so a failed copy never leaves a half-installed version.
    let target = dir(paths, version);
    let tmp = paths.home.join("php").join(format!(".install-{version}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(tmp.join("bin"))?;
    fs::create_dir_all(tmp.join("modules"))?;
    for bin in ["php", "php-fpm"] {
        fs::copy(from.join(bin), tmp.join("bin").join(bin))?;
    }
    if let Ok(entries) = fs::read_dir(from.join("modules")) {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "so") {
                fs::copy(entry.path(), tmp.join("modules").join(entry.file_name()))?;
            }
        }
    }
    // Keep the owner's edited ini files across a reinstall.
    if target.join("etc").is_dir() {
        copy_dir(&target.join("etc"), &tmp.join("etc"))?;
    }
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&tmp, &target)?;
    write_config(paths, version)
}

/// `php -r` reports "major.minor" of a binary, ignoring any ini.
fn php_version(php: &Path) -> io::Result<String> {
    let out = Command::new(php).args(["-n", "-r", "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;"]).output()?;
    if !out.status.success() {
        return Err(err(format!("{} -r failed: {}", php.display(), String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// Write php.ini (only if missing; it is the owner's to edit) and php-fpm.conf (always; Devid owns it).
pub fn write_config(paths: &Paths, version: &str) -> io::Result<()> {
    let etc = etc(paths, version);
    fs::create_dir_all(etc.join("conf.d"))?;
    fs::create_dir_all(paths.run_dir())?;
    fs::create_dir_all(paths.log_dir())?;

    let ini = etc.join("php.ini");
    if !ini.exists() {
        let ca = crate::ca::cacert(paths);
        fs::write(
            &ini,
            format!(
                "; Devid base php.ini for PHP {version}. Edit freely; Devid never overwrites this file.\n\
                 memory_limit = 512M\n\
                 upload_max_filesize = 64M\n\
                 post_max_size = 64M\n\
                 display_errors = On\n\
                 error_reporting = E_ALL\n\
                 opcache.enable = 1\n\
                 ; JIT off (D-008). `off` would parse as an empty boolean, so use `disable` plus a zero buffer.\n\
                 opcache.jit = disable\n\
                 opcache.jit_buffer_size = 0\n\
                 opcache.validate_timestamps = 1\n\
                 opcache.revalidate_freq = 0\n\
                 openssl.cafile = {ca}\n\
                 curl.cainfo = {ca}\n",
                ca = ca.display()
            ),
        )?;
    }

    let t = tag(version);
    // php-fpm starts workers with a cleared environment, so catalog extensions that need a variable
    // (imagick: MAGICK_CONFIGURE_PATH) get it through the pool.
    let env: String =
        crate::phpext::env(paths, version).iter().map(|(k, v)| format!("env[{k}] = \"{}\"\n", v.display())).collect();
    fs::write(
        etc.join("php-fpm.conf"),
        format!(
            "; Generated by Devid; changes are overwritten.\n\
             [global]\n\
             pid = {pid}\n\
             error_log = {log}\n\
             daemonize = no\n\
             \n\
             [devid-{t}]\n\
             listen = {sock}\n\
             listen.mode = 0600\n\
             pm = ondemand\n\
             pm.max_children = 4\n\
             pm.process_idle_timeout = 10s\n\
             catch_workers_output = yes\n\
             {env}",
            pid = pid_file(paths, version).display(),
            log = paths.log_dir().join(format!("php{t}-fpm.log")).display(),
            sock = socket(paths, version).display(),
        ),
    )
}

/// How the supervisor runs php-fpm for `version`.
pub fn spec(paths: &Paths, version: &str) -> Spec {
    let etc = etc(paths, version);
    Spec {
        program: fpm_bin(paths, version),
        args: vec![
            "--nodaemonize".into(),
            "--fpm-config".into(),
            etc.join("php-fpm.conf").into(),
            "-c".into(),
            etc.join("php.ini").into(),
        ],
        // D-008: never fall back to compiled-in ini paths.
        env: [("PHPRC".into(), etc.clone().into()), ("PHP_INI_SCAN_DIR".into(), etc.join("conf.d").into())]
            .into_iter()
            .chain(crate::phpext::env(paths, version).into_iter().map(|(k, v)| (k, v.into())))
            .collect(),
        pid_file: pid_file(paths, version),
        comm: "php-fpm",
        stop_signal: libc::SIGQUIT,
    }
}

/// Full version of an installed PHP (e.g. "8.4.27"), if it runs.
pub fn full_version(paths: &Paths, version: &str) -> Option<String> {
    let bin = dir(paths, version).join("bin/php");
    let out = crate::webserver::probe(&bin, &["-n", "-r", "echo PHP_VERSION;"]).ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty()).then_some(v)
}

/// Loadable extensions Devid ships as .so files (D-008); off by default.
pub const EXTENSIONS: [&str; 2] = ["xdebug", "pcov"];

fn ext_ini(paths: &Paths, version: &str, ext: &str) -> PathBuf {
    etc(paths, version).join("conf.d").join(format!("devid-{ext}.ini"))
}

/// Whether `ext` is turned on for `version` (its ini exists in conf.d).
pub fn extension_enabled(paths: &Paths, version: &str, ext: &str) -> bool {
    ext_ini(paths, version, ext).is_file()
}

/// Turn a shipped extension on (write conf.d/devid-<ext>.ini) or off (remove it). The caller restarts
/// php-fpm; the CLI shim picks it up on the next run (same PHP_INI_SCAN_DIR).
pub fn set_extension(paths: &Paths, version: &str, ext: &str, enabled: bool) -> io::Result<()> {
    if !EXTENSIONS.contains(&ext) {
        return Err(err(format!("unknown extension {ext:?}; choose one of {}", EXTENSIONS.join(", "))));
    }
    let ini = ext_ini(paths, version, ext);
    if !enabled {
        return match fs::remove_file(&ini) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let so = dir(paths, version).join("modules").join(format!("{ext}.so"));
    if !so.is_file() {
        return Err(err(format!("{} is missing; reinstall PHP {version}", so.display())));
    }
    let body = match ext {
        // Debugging only when an IDE/browser asks for it (XDEBUG_TRIGGER / XDEBUG_SESSION), so normal
        // requests stay fast; `develop` adds the nicer var_dump and error output.
        "xdebug" => format!(
            "; Generated by Devid (`devid php ext {version} xdebug off` removes it)\n\
             zend_extension={so}\n\
             xdebug.mode=debug,develop\n\
             xdebug.start_with_request=trigger\n\
             xdebug.client_port=9003\n",
            so = so.display()
        ),
        _ => format!(
            "; Generated by Devid (`devid php ext {version} pcov off` removes it)\n\
             extension={so}\n\
             pcov.enabled=1\n",
            so = so.display()
        ),
    };
    fs::create_dir_all(ini.parent().expect("ini has a parent"))?;
    fs::write(ini, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_versions_and_tags() {
        assert!(check_supported("8.4").is_ok());
        assert!(check_supported("8.1").is_err());
        assert!(check_supported("8.4; rm -rf /").is_err());
        assert_eq!(tag("8.5"), "85");
    }
}
