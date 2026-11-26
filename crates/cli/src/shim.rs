//! Shims: hard links to `devid` in `~/.devid/bin`, dispatched by the name they run under.
//! - `php` and `composer`: the PHP version is the one of the site containing the current folder (if
//!   isolated), otherwise the default.
//! - `php8.3`, `php8.4`, `php8.5`, `php8.6`: that version, wherever you are (D-053).
//! - `nginx`, `caddy`, `mailpit`: Devid's own copies, set up to act on Devid's config (D-053).
//! - `wp`: WP-CLI on the current folder's PHP, once `devid new wordpress` has downloaded it (D-063).
//! - `mysql`, `mysqldump`, `psql`, `pg_dump`, `pg_restore`, `mongosh`: the clients of the series
//!   Devid runs, set to reach Devid's server unless told where to go (D-061). Linked only once their
//!   engine is installed.
//!
//! They work without the daemon, reading config.toml directly, so they stay fast and usable when the
//! daemon is down.

use devid_daemon::{
    caddy,
    config::Config,
    db::{self, Engine},
    install, mail, nginx,
    paths::Paths,
    php, phpext, sites,
};
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
    if !install::SHIMS.contains(&name.as_str()) && db::client_engine(&name).is_none() {
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
        client if db::client_engine(client).is_some() => db_client_command(paths, client, args),
        "php" | "composer" | "wp" => {
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
    } else if name == "wp" {
        let phar = install::wp_cli_phar(paths);
        if !phar.is_file() {
            return Err("WP-CLI is not installed yet; `devid new wordpress <name>` downloads it".into());
        }
        // WP-CLI 2.12's own dependencies print PHP 8.5 deprecation notices before every command's
        // output (Phase 15 N1).
        cmd.args(["-d", "error_reporting=E_ALL & ~E_DEPRECATED"]).arg(phar);
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

/// A database client from the series Devid runs (else the newest installed one), set to reach
/// Devid's server as its main account unless the arguments or the environment say where to go
/// (D-061). Before the server's first start there is no port yet, and the client runs as it is.
fn db_client_command(paths: &Paths, name: &str, args: Vec<OsString>) -> Result<Command, String> {
    let engine = db::client_engine(name).ok_or("not one of Devid's shims")?;
    let config = Config::load_or_create(&paths.config()).map_err(|e| e.to_string())?;
    let saved = config.databases.get(engine.name());
    let installed = db::installed_series(paths, engine);
    let series = saved
        .map(|d| d.series.clone())
        .filter(|s| installed.contains(s))
        .or_else(|| installed.last().cloned())
        .ok_or_else(|| format!("{} is not installed; run `devid db install {}`", engine.label(), engine.name()))?;
    let bin = installed_bin(db::client_bin(paths, engine, &series, name), engine, &series)?;
    let port = saved.map(|d| d.port).filter(|p| *p != 0);
    let words: Vec<&str> = args.iter().filter_map(|a| a.to_str()).collect();
    let mut cmd = Command::new(bin);
    match (engine, port) {
        (_, None) => {
            cmd.args(args);
        }
        (Engine::Mysql, Some(port)) => {
            // The option-file flags must come first, so Devid's go after them; a defaults file or a
            // login path of the user's own says where to go by itself.
            let lead = args.iter().take_while(|a| a.to_str().is_some_and(|a| a.starts_with("--defaults-"))).count();
            let own = words.iter().any(|w| w.starts_with("--defaults-") || w.starts_with("--login-path"));
            cmd.args(&args[..lead]);
            if !own {
                if !mysql_given(&words, &["host", "socket", "protocol"], &['h', 'S']) {
                    match mysql_given(&words, &["port"], &['P']) {
                        // A port alone would go to the default socket; make it count.
                        true => {
                            cmd.arg("--host=127.0.0.1");
                        }
                        false => {
                            cmd.args(db::mysql_target(paths, port));
                        }
                    }
                }
                if !mysql_given(&words, &["user"], &['u']) {
                    cmd.arg("--user=root");
                }
            }
            cmd.args(&args[lead..]);
        }
        (Engine::Postgresql, Some(port)) => {
            let given = words.iter().any(|w| {
                ["-h", "-p", "--host", "--port", "postgres://", "postgresql://"].iter().any(|f| w.starts_with(f))
                    || w.contains("host=")
                    || w.contains("port=")
            });
            let env_given =
                ["PGHOST", "PGPORT", "PGHOSTADDR", "PGSERVICE"].iter().any(|v| std::env::var_os(v).is_some());
            if !given && !env_given {
                cmd.env("PGHOST", db::pg_host(paths, port)).env("PGPORT", port.to_string());
                let user =
                    words.iter().any(|w| w.starts_with("-U") || w.starts_with("--username") || w.contains("user="));
                if !user && std::env::var_os("PGUSER").is_none() {
                    cmd.env("PGUSER", "postgres");
                }
            }
            cmd.args(args);
        }
        (Engine::Mongodb, Some(port)) => {
            // A `host:port[/db]` address, unlike a script such as `db.x.find({a:1})`.
            let address = |w: &str| {
                w.split_once(':').is_some_and(|(host, rest)| {
                    !host.is_empty()
                        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
                        && rest.starts_with(|c: char| c.is_ascii_digit())
                })
            };
            let given = words.iter().any(|w| {
                ["--host", "--port", "--nodb", "mongodb://", "mongodb+srv://"].iter().any(|f| w.starts_with(f))
                    || address(w)
            });
            if !given {
                cmd.arg("--port").arg(port.to_string());
            }
            cmd.args(args);
        }
    }
    Ok(cmd)
}

fn installed_bin(bin: PathBuf, engine: Engine, series: &str) -> Result<PathBuf, String> {
    installed(
        bin.clone(),
        &format!("{} is missing; `devid db install {} {series}` puts it back", bin.display(), engine.name()),
    )
}

/// Whether the arguments of a MySQL client set one of these options: `--name`, `--name=…`, `-x`
/// or `-x…`. A lone `--` ends the options.
fn mysql_given(words: &[&str], long: &[&str], short: &[char]) -> bool {
    words.iter().take_while(|w| **w != "--").any(|w| match w.strip_prefix("--") {
        Some(opt) => long.iter().any(|l| opt == *l || opt.starts_with(&format!("{l}="))),
        None => w.strip_prefix('-').and_then(|o| o.chars().next()).is_some_and(|c| short.contains(&c)),
    })
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

    /// A Devid home with MySQL 8.4, PostgreSQL 18 and MongoDB 8.0 "installed", and `port` saved for
    /// each engine (0: never started).
    fn db_home(tag: &str, port: u16) -> Paths {
        let paths = home(tag);
        for (engine, series, programs) in [
            (Engine::Mysql, "8.4", &["mysqld", "mysql", "mysqldump"][..]),
            (Engine::Postgresql, "18", &["postgres", "psql", "pg_dump", "pg_restore"][..]),
            (Engine::Mongodb, "8.0", &["mongod"][..]),
        ] {
            for program in programs {
                let bin = db::bin(&paths, engine, series, program);
                std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
                std::fs::write(&bin, "").unwrap();
            }
        }
        std::fs::create_dir_all(db::mongosh(&paths).parent().unwrap()).unwrap();
        std::fs::write(db::mongosh(&paths), "").unwrap();
        let mut config = Config::load_or_create(&paths.config()).unwrap();
        for (engine, series) in [("mysql", "8.4"), ("postgresql", "18"), ("mongodb", "8.0")] {
            config.databases.insert(
                engine.into(),
                devid_daemon::config::Database { series: series.into(), port, enabled: port != 0 },
            );
        }
        config.save(&paths.config()).unwrap();
        paths
    }

    fn env(cmd: &Command, name: &str) -> Option<String> {
        cmd.get_envs().find(|(k, _)| *k == name).and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
    }

    #[test]
    fn mysql_reaches_devids_server_unless_told_otherwise() {
        let paths = db_home("mysql", 3307);
        let cmd = command("mysql", args(&["shop"]), &paths).unwrap();
        assert_eq!(cmd.get_program(), db::bin(&paths, Engine::Mysql, "8.4", "mysql"));
        // No socket yet (the server is not running here): 127.0.0.1 and the port.
        assert_eq!(argv(&cmd), ["--host=127.0.0.1", "--port=3307", "--user=root", "shop"]);

        let own = argv(&command("mysqldump", args(&["-h", "10.0.0.2", "-uadmin", "shop"]), &paths).unwrap());
        assert_eq!(own, ["-h", "10.0.0.2", "-uadmin", "shop"], "the user's host and user win");
        let port = argv(&command("mysql", args(&["-P3306"]), &paths).unwrap());
        assert_eq!(port, ["--host=127.0.0.1", "--user=root", "-P3306"], "a port alone goes over TCP");
        let file = argv(&command("mysql", args(&["--defaults-file=/x.cnf", "-e", "SELECT 1"]), &paths).unwrap());
        assert_eq!(file, ["--defaults-file=/x.cnf", "-e", "SELECT 1"], "the user's option file says where to go");
        let password = argv(&command("mysql", args(&["-p", "--", "-hfoo"]), &paths).unwrap());
        assert_eq!(password[..3], ["--host=127.0.0.1", "--port=3307", "--user=root"], "-p is the password, not -P");
        std::fs::remove_dir_all(&paths.home).unwrap();
    }

    #[test]
    fn psql_and_mongosh_reach_devids_server_unless_told_otherwise() {
        let paths = db_home("pg", 5433);
        if ["PGHOST", "PGPORT", "PGHOSTADDR", "PGSERVICE", "PGUSER"].iter().all(|v| std::env::var_os(v).is_none()) {
            let cmd = command("psql", args(&["-d", "shop"]), &paths).unwrap();
            assert_eq!(env(&cmd, "PGHOST").as_deref(), Some("127.0.0.1"));
            assert_eq!(env(&cmd, "PGPORT").as_deref(), Some("5433"));
            assert_eq!(env(&cmd, "PGUSER").as_deref(), Some("postgres"));
            let own = command("pg_dump", args(&["-U", "shop", "shop"]), &paths).unwrap();
            assert_eq!(env(&own, "PGUSER"), None, "the user's account wins");
            let uri = command("psql", args(&["postgresql://127.0.0.1:5432/x"]), &paths).unwrap();
            assert_eq!(env(&uri, "PGHOST"), None, "a URI says where to go");
        }
        let mongosh = command("mongosh", args(&["shop"]), &paths).unwrap();
        assert_eq!(mongosh.get_program(), db::mongosh(&paths));
        assert_eq!(argv(&mongosh), ["--port", "5433", "shop"]);
        assert_eq!(argv(&command("mongosh", args(&["mongodb://h:1/x"]), &paths).unwrap()), ["mongodb://h:1/x"]);
        assert_eq!(argv(&command("mongosh", args(&["db2.local:27018/x"]), &paths).unwrap()), ["db2.local:27018/x"]);
        let eval = argv(&command("mongosh", args(&["--eval", "db.runCommand({ping:1})"]), &paths).unwrap());
        assert_eq!(eval[..2], ["--port", "5433"], "a script is not an address");
        std::fs::remove_dir_all(&paths.home).unwrap();

        let fresh = db_home("fresh", 0);
        assert!(argv(&command("mysql", args(&["-V"]), &fresh).unwrap()) == ["-V"], "never started: as it is");
        std::fs::remove_dir_all(&fresh.home).unwrap();
        let none = home("none");
        assert!(command("psql", args(&[]), &none).unwrap_err().contains("devid db install postgresql"));
        std::fs::remove_dir_all(&none.home).unwrap();
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
