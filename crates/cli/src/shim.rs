//! `php` and `composer` shims: hard links to `devid` in `~/.devid/bin`. The PHP version is the one of
//! the site containing the current folder (if isolated), otherwise the default. Works without the
//! daemon, reading config.toml directly, so it stays fast and usable when the daemon is down.

use devid_daemon::{config::Config, install, paths::Paths, php, sites};
use std::{
    ffi::OsString,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

/// If invoked as `php` or `composer`, run that and never return; otherwise None.
pub fn dispatch() -> Option<ExitCode> {
    let argv0 = std::env::args_os().next()?;
    let name = Path::new(&argv0).file_name()?.to_str()?.to_string();
    if !install::SHIMS.contains(&name.as_str()) {
        return None;
    }
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let err = run(&name, args, &Paths::from_env());
    eprintln!("devid ({name}): {err}");
    Some(ExitCode::from(127))
}

/// The PHP version and binary for the current folder.
pub fn resolve(paths: &Paths, cwd: &Path) -> Result<(String, PathBuf), String> {
    let config = Config::load_or_create(&paths.config()).map_err(|e| e.to_string())?;
    let sites = sites::resolve(&config);
    let version = sites::find_by_dir(&sites, cwd).map_or(config.default_php.clone(), |s| s.php.clone());
    let bin = php::dir(paths, &version).join("bin/php");
    if !bin.is_file() {
        return Err(format!("PHP {version} is not installed; run `devid php install {version} --from <dir>`"));
    }
    Ok((version, bin))
}

/// exec() into PHP; only returns on error.
fn run(name: &str, args: Vec<OsString>, paths: &Paths) -> String {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => return e.to_string(),
    };
    let (version, bin) = match resolve(paths, &cwd) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let etc = php::etc(paths, &version);
    let mut cmd = Command::new(&bin);
    // D-008: the same ini as php-fpm, never the compiled-in paths.
    cmd.env("PHPRC", &etc).env("PHP_INI_SCAN_DIR", etc.join("conf.d"));
    if name == "composer" {
        let phar = install::composer_phar(paths);
        if !phar.is_file() {
            return format!("{} is missing; `devid setup` downloads it", phar.display());
        }
        cmd.arg(phar);
    } else {
        cmd.arg0("php");
    }
    cmd.args(args).exec().to_string()
}
