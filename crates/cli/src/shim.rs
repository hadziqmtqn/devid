//! Shims: hard links to `devid` in `~/.devid/bin`, dispatched by the name they run under.
//! - `php` and `composer`: the PHP version is the one of the site containing the current folder (if
//!   isolated), otherwise the default.
//! - `php8.3`, `php8.4`, `php8.5`, `php8.6`: that version, wherever you are (D-053).
//! - `nginx`, `caddy`, `mailpit`: Devid's own copies, set up to act on Devid's config (D-053).
//!
//! They work without the daemon, reading config.toml directly, so they stay fast and usable when the
//! daemon is down.

use devid_daemon::{caddy, config::Config, install, mail, nginx, paths::Paths, php, phpext, sites};
use std::{
    ffi::OsString,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

/// If invoked under a shim's name, run that and never return; otherwise None.
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
    php_bin(paths, &version).map(|bin| (version, bin))
}

fn php_bin(paths: &Paths, version: &str) -> Result<PathBuf, String> {
    let bin = php::dir(paths, version).join("bin/php");
    if !bin.is_file() {
        return Err(format!("PHP {version} is not installed; run `devid php install {version}`"));
    }
    Ok(bin)
}

/// exec() into the program; only returns on error.
fn run(name: &str, args: Vec<OsString>, paths: &Paths) -> String {
    match command(name, args, paths) {
        Ok(mut cmd) => cmd.exec().to_string(),
        Err(e) => e,
    }
}

/// What a shim runs, with its arguments and environment.
fn command(name: &str, args: Vec<OsString>, paths: &Paths) -> Result<Command, String> {
    match name {
        "nginx" => nginx_command(paths, args),
        "caddy" => {
            let bin = installed(caddy::bin(paths), "Caddy is not installed; run `devid server install caddy`")?;
            let mut cmd = Command::new(bin);
            // The same XDG dirs as the daemon's Caddy (caddy::spec), so `caddy` finds its data.
            cmd.envs(caddy::spec(paths).env).args(args);
            Ok(cmd)
        }
        "mailpit" => mailpit_command(paths, args),
        "php" | "composer" => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let (version, bin) = resolve(paths, &cwd)?;
            php_command(paths, name, &version, bin, args)
        }
        versioned => {
            let version = versioned
                .strip_prefix("php")
                .filter(|v| php::SUPPORTED.contains(v))
                .ok_or("not one of Devid's shims")?;
            php_command(paths, versioned, version, php_bin(paths, version)?, args)
        }
    }
}

fn installed(bin: PathBuf, missing: &str) -> Result<PathBuf, String> {
    if bin.is_file() { Ok(bin) } else { Err(missing.into()) }
}

/// PHP (or composer on it) with Devid's ini, as php-fpm has it.
fn php_command(paths: &Paths, name: &str, version: &str, bin: PathBuf, args: Vec<OsString>) -> Result<Command, String> {
    let etc = php::etc(paths, version);
    let mut cmd = Command::new(&bin);
    // D-008: the same ini as php-fpm, never the compiled-in paths.
    cmd.env("PHPRC", &etc).env("PHP_INI_SCAN_DIR", etc.join("conf.d"));
    // What catalog extensions need, e.g. imagick's MAGICK_CONFIGURE_PATH (the same as php-fpm gets).
    cmd.envs(phpext::env(paths, version));
    if name == "composer" {
        let phar = install::composer_phar(paths);
        if !phar.is_file() {
            return Err(format!("{} is missing; `devid setup` downloads it", phar.display()));
        }
        cmd.arg(phar);
    } else {
        // PHP_BINARY comes from argv[0] through PATH, so child processes (`artisan serve`, parallel
        // tests) come back through this same shim, and `php8.4`'s children stay on 8.4.
        cmd.arg0(name);
    }
    cmd.args(args);
    Ok(cmd)
}

/// The nginx flags that only look or signal; anything else would start a second nginx.
const NGINX_ONE_SHOT: [&str; 7] = ["-t", "-T", "-v", "-V", "-h", "-?", "-s"];

/// Devid's nginx on Devid's config, for `nginx -t`, `nginx -T`, `nginx -s reload`, `nginx -V`.
/// Starting it is the daemon's job: a plain `nginx` would start a second one beside it.
fn nginx_command(paths: &Paths, args: Vec<OsString>) -> Result<Command, String> {
    let bin = installed(nginx::bin(paths), "nginx is not installed; run `devid server install nginx`")?;
    let has = |flag: &str| args.iter().any(|a| a.to_str() == Some(flag));
    if !NGINX_ONE_SHOT.iter().any(|f| has(f)) {
        return Err("Devid runs nginx itself; use `devid start|stop|restart nginx`. \
             This command takes -t, -T, -s reload, -v or -V, on Devid's config."
            .into());
    }
    let mut cmd = Command::new(bin);
    if !has("-e") {
        cmd.arg("-e").arg(nginx::error_log(paths));
    }
    if !has("-p") {
        cmd.arg("-p").arg(nginx::prefix(paths));
    }
    if !has("-c") {
        cmd.arg("-c").arg(nginx::conf(paths));
    }
    cmd.args(args);
    Ok(cmd)
}

/// Devid's Mailpit. `mailpit sendmail` delivers to Devid's inbox (the port in config.toml) unless
/// told otherwise, as PHP's mail() does.
fn mailpit_command(paths: &Paths, args: Vec<OsString>) -> Result<Command, String> {
    let bin = installed(mail::bin(paths), "Mailpit is not installed; run `devid mail on`")?;
    let mut cmd = Command::new(bin);
    let sendmail = args.first().and_then(|a| a.to_str()) == Some("sendmail");
    let has_addr = args.iter().any(|a| a.to_str().is_some_and(|a| a == "-S" || a.starts_with("--smtp-addr")));
    cmd.args(args.iter().take(1));
    if sendmail && !has_addr {
        let config = Config::load_or_create(&paths.config()).map_err(|e| e.to_string())?;
        cmd.arg("-S").arg(format!("127.0.0.1:{}", config.mail.port));
    }
    cmd.args(args.iter().skip(1));
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<OsString> {
        a.iter().map(OsString::from).collect()
    }

    fn argv(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    /// A Devid home with fake nginx and Mailpit binaries.
    fn home(tag: &str) -> Paths {
        let paths = Paths { home: std::env::temp_dir().join(format!("devid-shim-{tag}-{}", std::process::id())) };
        for bin in [nginx::bin(&paths), mail::bin(&paths)] {
            std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
            std::fs::write(&bin, "").unwrap();
        }
        paths
    }

    #[test]
    fn nginx_tests_and_signals_devids_config_but_never_starts_a_second_one() {
        let paths = home("nginx");
        let a = argv(&nginx_command(&paths, args(&["-t"])).unwrap());
        assert_eq!(a[a.len() - 1], "-t");
        assert!(a.windows(2).any(|w| w[0] == "-c" && w[1] == nginx::conf(&paths).display().to_string()));
        assert!(a.windows(2).any(|w| w[0] == "-p" && w[1] == nginx::prefix(&paths).display().to_string()));

        let own = argv(&nginx_command(&paths, args(&["-c", "/tmp/x.conf", "-t"])).unwrap());
        assert_eq!(own.iter().filter(|a| *a == "-c").count(), 1, "the user's -c wins");

        assert!(nginx_command(&paths, args(&["-s", "reload"])).is_ok());
        assert!(nginx_command(&paths, args(&[])).unwrap_err().contains("devid start"));
        assert!(nginx_command(&paths, args(&["-g", "daemon off;"])).is_err());
        std::fs::remove_dir_all(&paths.home).unwrap();
    }

    #[test]
    fn mailpit_sendmail_goes_to_devids_inbox() {
        let paths = home("mailpit");
        let port = Config::load_or_create(&paths.config()).unwrap().mail.port;
        let a = argv(&mailpit_command(&paths, args(&["sendmail", "-t"])).unwrap());
        assert_eq!(a, ["sendmail", "-S", &format!("127.0.0.1:{port}"), "-t"]);
        let own = argv(&mailpit_command(&paths, args(&["sendmail", "-S", "127.0.0.1:1025"])).unwrap());
        assert_eq!(own, ["sendmail", "-S", "127.0.0.1:1025"], "an address of the user's own wins");
        assert_eq!(argv(&mailpit_command(&paths, args(&["version"])).unwrap()), ["version"]);
        std::fs::remove_dir_all(&paths.home).unwrap();
    }

    #[test]
    fn a_missing_program_says_how_to_install_it() {
        let paths = home("php");
        let e = command("php8.4", args(&["-v"]), &paths).unwrap_err();
        assert!(e.contains("devid php install 8.4"), "{e}");
        assert!(command("php9.9", args(&[]), &paths).is_err());
        assert!(command("caddy", args(&[]), &paths).unwrap_err().contains("devid server install caddy"));
        std::fs::remove_dir_all(&paths.home).unwrap();
    }
}
