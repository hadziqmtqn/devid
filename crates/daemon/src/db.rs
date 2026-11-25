//! Databases (D-061, Phase 14): Devid installs MySQL, PostgreSQL and MongoDB from devid-binaries'
//! `db-latest` release (D-058) and runs one server per engine under the supervisor, on 127.0.0.1.
//! Binaries live in `db/<engine>-<series>`; data lives in `data/<engine>-<series>`, which nothing in
//! Devid deletes without being asked. The settings are D1's (docs/roadmap/phase-14.md).

use crate::{paths::Paths, supervisor::Spec};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Engine {
    Mysql,
    Postgresql,
    Mongodb,
}

pub const ENGINES: [Engine; 3] = [Engine::Mysql, Engine::Postgresql, Engine::Mongodb];

/// A database gets this long to stop cleanly before SIGKILL; D1's idle servers stopped within 1.5 s.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(60);

/// The longest unix socket path: macOS's sun_path holds 104 bytes with the final NUL. MySQL refuses
/// to start with a longer one (D1); PostgreSQL and MongoDB then run without a socket.
pub const SOCKET_MAX: usize = 103;

/// pg_hba.conf for Devid's PostgreSQL (D-061, D1): the socket in `run/` (only the user can open it)
/// and `postgres` from this computer need no password; every other account needs one.
const PG_HBA: &str = "# Devid (D-061): the socket and the main account need no password; every other account does.\n\
local   all          all                       trust\n\
host    all          postgres   127.0.0.1/32   trust\n\
host    all          postgres   ::1/128        trust\n\
host    all          all        127.0.0.1/32   scram-sha-256\n\
host    all          all        ::1/128        scram-sha-256\n\
local   replication  all                       trust\n\
host    replication  all        127.0.0.1/32   scram-sha-256\n";

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::Mysql => "mysql",
            Engine::Postgresql => "postgresql",
            Engine::Mongodb => "mongodb",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Engine::Mysql => "MySQL",
            Engine::Postgresql => "PostgreSQL",
            Engine::Mongodb => "MongoDB",
        }
    }

    pub fn parse(name: &str) -> Option<Engine> {
        match name.to_ascii_lowercase().as_str() {
            "mysql" => Some(Engine::Mysql),
            "postgresql" | "postgres" | "pgsql" | "pg" => Some(Engine::Postgresql),
            "mongodb" | "mongo" => Some(Engine::Mongodb),
            _ => None,
        }
    }

    /// The port the engine's docs and Laravel's `.env` use.
    pub fn usual_port(self) -> u16 {
        match self {
            Engine::Mysql => 3306,
            Engine::Postgresql => 5432,
            Engine::Mongodb => 27017,
        }
    }

    /// The series a first `devid db start` takes.
    pub fn default_series(self) -> &'static str {
        match self {
            Engine::Mysql => "8.4",
            Engine::Postgresql => "18",
            Engine::Mongodb => "8.0",
        }
    }

    fn server(self) -> &'static str {
        match self {
            Engine::Mysql => "mysqld",
            Engine::Postgresql => "postgres",
            Engine::Mongodb => "mongod",
        }
    }

    /// Part of Devid's server path, for the orphan check on macOS (`ps -o comm=` is the whole path):
    /// DBngin's and Homebrew's servers live elsewhere.
    fn comm(self) -> &'static str {
        match self {
            Engine::Mysql => "/db/mysql-",
            Engine::Postgresql => "/db/postgresql-",
            Engine::Mongodb => "/db/mongodb-",
        }
    }

    /// The supervisor's name, also what `devid stop|start` take.
    pub fn service(self) -> &'static str {
        self.name()
    }
}

fn key(engine: Engine, series: &str) -> String {
    format!("{}-{series}", engine.name())
}

pub fn dir(paths: &Paths, engine: Engine, series: &str) -> PathBuf {
    paths.home.join("db").join(key(engine, series))
}

pub fn bin(paths: &Paths, engine: Engine, series: &str, name: &str) -> PathBuf {
    dir(paths, engine, series).join("bin").join(name)
}

/// Where every database's data lives: `devid uninstall --purge` keeps it (D-061).
pub fn data_root(paths: &Paths) -> PathBuf {
    paths.home.join("data")
}

/// The server's data. Devid never deletes it unless asked.
pub fn data(paths: &Paths, engine: Engine, series: &str) -> PathBuf {
    data_root(paths).join(key(engine, series))
}

pub fn log(paths: &Paths, engine: Engine) -> PathBuf {
    paths.log_dir().join(format!("{}.log", engine.name()))
}

/// mongosh, which MongoDB needs for Devid's own accounts; installed with MongoDB.
pub fn mongosh(paths: &Paths) -> PathBuf {
    paths.home.join("db").join("mongosh").join("bin/mongosh")
}

/// The passwords Devid set on a server (Devid's own account, and the main one on MongoDB), in a
/// file only the user can read.
pub fn accounts_file(paths: &Paths, engine: Engine, series: &str) -> PathBuf {
    data(paths, engine, series).join("accounts.toml")
}

fn version_file(paths: &Paths, engine: Engine, series: &str) -> PathBuf {
    dir(paths, engine, series).join(".devid-version")
}

/// The installed build of a series ("8.4.13"), as `install` recorded it.
pub fn version(paths: &Paths, engine: Engine, series: &str) -> Option<String> {
    fs::read_to_string(version_file(paths, engine, series)).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn installed(paths: &Paths, engine: Engine, series: &str) -> bool {
    bin(paths, engine, series, engine.server()).is_file() && (engine != Engine::Mongodb || mongosh(paths).is_file())
}

/// The installed series of `engine`, oldest first.
pub fn installed_series(paths: &Paths, engine: Engine) -> Vec<String> {
    let prefix = format!("{}-", engine.name());
    let mut out: Vec<String> = fs::read_dir(paths.home.join("db"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().to_str()?.strip_prefix(&prefix).map(String::from))
                .filter(|series| !series.ends_with(".old") && installed(paths, engine, series))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|s| s.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>());
    out
}

/// Where the server's unix socket goes: one MySQL runs at a time, PostgreSQL and MongoDB name theirs
/// after the port.
pub fn socket(paths: &Paths, engine: Engine, port: u16) -> PathBuf {
    paths.run_dir().join(match engine {
        Engine::Mysql => "mysql.sock".to_string(),
        Engine::Postgresql => format!(".s.PGSQL.{port}"),
        Engine::Mongodb => format!("mongodb-{port}.sock"),
    })
}

pub fn socket_fits(path: &Path) -> bool {
    path.as_os_str().len() <= SOCKET_MAX
}

/// Why a server cannot start here: MySQL needs its socket, and refuses a path over `SOCKET_MAX`.
pub fn check_socket(paths: &Paths, engine: Engine, port: u16) -> Result<(), String> {
    let sock = socket(paths, engine, port);
    if engine == Engine::Mysql && !socket_fits(&sock) {
        return Err(format!(
            "MySQL's socket path {} is longer than {SOCKET_MAX} bytes, which MySQL refuses; use a shorter DEVID_HOME",
            sock.display()
        ));
    }
    Ok(())
}

/// Put a build in place as `db/<engine>-<series>`, replacing an older build of the series (its data
/// stays in `data/`), and record its version. `from` is the unpacked archive.
pub fn install(paths: &Paths, engine: Engine, series: &str, from: &Path, version: &str) -> io::Result<()> {
    if !from.join("bin").join(engine.server()).is_file() {
        return Err(err(format!("{} has no bin/{}; not a {} build", from.display(), engine.server(), engine.label())));
    }
    put_in_place(from, &dir(paths, engine, series))?;
    fs::write(version_file(paths, engine, series), format!("{version}\n"))
}

/// Put mongosh in place as `db/mongosh` and record its version.
pub fn install_mongosh(paths: &Paths, from: &Path, version: &str) -> io::Result<()> {
    if !from.join("bin/mongosh").is_file() {
        return Err(err(format!("{} has no bin/mongosh", from.display())));
    }
    let target = paths.home.join("db").join("mongosh");
    put_in_place(from, &target)?;
    fs::write(target.join(".devid-version"), format!("{version}\n"))
}

/// The installed mongosh's version, as `install_mongosh` recorded it.
pub fn mongosh_version(paths: &Paths) -> Option<String> {
    let file = paths.home.join("db").join("mongosh").join(".devid-version");
    fs::read_to_string(file).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn put_in_place(from: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target.parent().expect("db folder has a parent"))?;
    let old = PathBuf::from(format!("{}.old", target.display()));
    let _ = fs::remove_dir_all(&old);
    if target.exists() {
        fs::rename(target, &old)?;
    }
    fs::rename(from, target)?;
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

/// Whether the data folder is set up: MySQL's system schema, PostgreSQL's PG_VERSION, or MongoDB's
/// folder.
pub fn initialized(paths: &Paths, engine: Engine, series: &str) -> bool {
    let d = data(paths, engine, series);
    match engine {
        Engine::Mysql => d.join("mysql").is_dir(),
        Engine::Postgresql => d.join("PG_VERSION").is_file(),
        Engine::Mongodb => d.is_dir(),
    }
}

/// Set up the data folder once (D1): MySQL without a root password; PostgreSQL with `trust` and
/// Devid's own pg_hba.conf, since initdb refuses a password method for TCP unless the superuser has
/// a password; an empty folder for MongoDB, whose accounts come at its first start (`bootstrap`).
pub fn init(paths: &Paths, engine: Engine, series: &str) -> io::Result<()> {
    if initialized(paths, engine, series) {
        return Ok(());
    }
    let d = data(paths, engine, series);
    let existed = d.exists();
    fs::create_dir_all(d.parent().expect("data folder has a parent"))?;
    fs::create_dir_all(paths.log_dir())?;
    let done = match engine {
        Engine::Mysql => run(
            Command::new(bin(paths, engine, series, "mysqld"))
                .args(["--no-defaults", "--initialize-insecure"])
                .arg(format!("--basedir={}", dir(paths, engine, series).display()))
                .arg(format!("--datadir={}", d.display()))
                .arg(format!("--log-error={}", log(paths, engine).display())),
            "mysqld --initialize-insecure",
        ),
        Engine::Postgresql => run(
            Command::new(bin(paths, engine, series, "initdb")).arg("-D").arg(&d).args([
                "-U",
                "postgres",
                "-A",
                "trust",
                "-E",
                "UTF8",
                "--locale=C",
            ]),
            "initdb",
        )
        .and_then(|_| fs::write(d.join("pg_hba.conf"), PG_HBA)),
        Engine::Mongodb => fs::create_dir_all(&d),
    };
    if let Err(e) = done {
        // A half-made folder would make the next try fail ("the data directory has files in it").
        if !existed {
            let _ = fs::remove_dir_all(&d);
        }
        return Err(e);
    }
    fs::set_permissions(&d, fs::Permissions::from_mode(0o700))
}

fn run(cmd: &mut Command, what: &str) -> io::Result<()> {
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| err(format!("{what}: {e}")))?;
    if out.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let text = if text.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { text };
    Err(err(format!("{what} failed ({}): {}", out.status, tail(&text, 600))))
}

/// The last `n` characters of `text`.
fn tail(text: &str, n: usize) -> &str {
    let start = text.char_indices().rev().nth(n.saturating_sub(1)).map_or(0, |(i, _)| i);
    &text[start..]
}

/// How the supervisor runs a server (D1's settings), on 127.0.0.1 only:
/// - MySQL without its X Protocol (DBngin's MySQL holds 33060 on every interface), binary log or
///   performance schema, which halves its idle memory;
/// - PostgreSQL with 32 MB of shared buffers and 50 connections;
/// - MongoDB with access control, a 0.25 GB cache, no diagnostic data, and on Linux its bundled
///   OpenSSL 3 (D-058).
pub fn spec(paths: &Paths, engine: Engine, series: &str, port: u16) -> Spec {
    let d = data(paths, engine, series);
    let sock = socket(paths, engine, port);
    let sockets = socket_fits(&sock);
    let run_dir = paths.run_dir();
    let log = log(paths, engine);
    let mut env: Vec<(String, OsString)> = Vec::new();
    let (args, pid_file, stop_signal): (Vec<OsString>, PathBuf, libc::c_int) = match engine {
        Engine::Mysql => {
            let pid = run_dir.join("mysql.pid");
            let args = vec![
                "--no-defaults".into(),
                format!("--basedir={}", dir(paths, engine, series).display()).into(),
                format!("--datadir={}", d.display()).into(),
                format!("--port={port}").into(),
                "--bind-address=127.0.0.1".into(),
                format!("--socket={}", sock.display()).into(),
                "--mysqlx=OFF".into(),
                "--skip-log-bin".into(),
                "--performance-schema=OFF".into(),
                format!("--log-error={}", log.display()).into(),
                format!("--pid-file={}", pid.display()).into(),
            ];
            (args, pid, libc::SIGTERM)
        }
        Engine::Postgresql => {
            let socket_dir = if sockets { run_dir.display().to_string() } else { String::new() };
            let args = vec![
                "-D".into(),
                d.clone().into(),
                "-p".into(),
                port.to_string().into(),
                "-c".into(),
                "listen_addresses=127.0.0.1".into(),
                "-c".into(),
                format!("unix_socket_directories={socket_dir}").into(),
                "-c".into(),
                "shared_buffers=32MB".into(),
                "-c".into(),
                "max_connections=50".into(),
            ];
            // PostgreSQL's own pid file; its first line is the postmaster's pid.
            (args, d.join("postmaster.pid"), libc::SIGINT)
        }
        Engine::Mongodb => {
            let pid = run_dir.join("mongod.pid");
            let mut args: Vec<OsString> = vec![
                "--dbpath".into(),
                d.clone().into(),
                "--port".into(),
                port.to_string().into(),
                "--bind_ip".into(),
                "127.0.0.1".into(),
            ];
            if sockets {
                args.extend(["--unixSocketPrefix".into(), run_dir.clone().into()]);
            } else {
                args.push("--nounixsocket".into());
            }
            args.extend([
                "--logpath".into(),
                log.clone().into(),
                "--logappend".into(),
                "--auth".into(),
                "--wiredTigerCacheSizeGB".into(),
                "0.25".into(),
                "--setParameter".into(),
                "diagnosticDataCollectionEnabled=false".into(),
                "--pidfilepath".into(),
                pid.clone().into(),
            ]);
            if cfg!(target_os = "linux") {
                env.push(("LD_LIBRARY_PATH".into(), dir(paths, engine, series).join("lib").into()));
            }
            (args, pid, libc::SIGTERM)
        }
    };
    Spec {
        program: bin(paths, engine, series, engine.server()),
        args,
        env,
        pid_file,
        write_pid: false,
        comm: engine.comm(),
        stop_signal,
        stop_timeout: STOP_TIMEOUT,
        log: Some(log),
        ready_port: Some(port),
        // Its own socket shows that the server on the port is Devid's (D1: DBngin's MySQL could
        // hold a port), when the path fits.
        ready_file: sockets.then_some(sock),
    }
}

/// The port a first start takes: the engine's usual one when Devid does not use it and nothing
/// listens there, else the next free one (D-061). Next to DBngin's 3306 and 5432 that is 3307 and
/// 5433.
pub fn pick_port(engine: Engine, taken: &[u16], in_use: impl Fn(u16) -> bool) -> Option<u16> {
    let start = engine.usual_port();
    (start..start.saturating_add(100)).find(|p| !taken.contains(p) && !in_use(*p))
}

/// An account whose password Devid set (D-061), and the database it was made for.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "AccountToml")]
pub struct Account {
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

/// How `accounts.toml` holds an account: a table, or D2's password alone (`root = "…"`).
#[derive(Deserialize)]
#[serde(untagged)]
enum AccountToml {
    Password(String),
    Table {
        #[serde(default)]
        password: String,
        #[serde(default)]
        database: Option<String>,
    },
}

impl From<AccountToml> for Account {
    fn from(entry: AccountToml) -> Self {
        match entry {
            AccountToml::Password(password) => Account { password, database: None },
            AccountToml::Table { password, database } => Account { password, database },
        }
    }
}

/// The passwords Devid set on one server, in `accounts.toml` (mode 0600).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    /// Devid's own account (`devid`), with every right: how Devid manages accounts and resets a
    /// lost password on MySQL and MongoDB.
    pub devid: String,
    /// Accounts whose password Devid set, by name.
    pub users: BTreeMap<String, Account>,
}

pub fn read_accounts(paths: &Paths, engine: Engine, series: &str) -> Option<Accounts> {
    toml::from_str(&fs::read_to_string(accounts_file(paths, engine, series)).ok()?).ok()
}

fn write_accounts(path: &Path, accounts: &Accounts) -> io::Result<()> {
    let text = format!(
        "# Devid (D-061): the passwords Devid set on this server. Only you can read this file.\n{}",
        toml::to_string_pretty(accounts).expect("accounts serialize")
    );
    let tmp = path.with_extension("toml.tmp");
    let _ = fs::remove_file(&tmp);
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
    file.write_all(text.as_bytes())?;
    drop(file);
    fs::rename(tmp, path)
}

/// A random password: 24 letters and digits, nothing that needs quoting in `.env`.
pub fn password() -> String {
    use ring::rand::{SecureRandom, SystemRandom};
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let rng = SystemRandom::new();
    let mut out = String::new();
    let mut byte = [0u8; 1];
    while out.len() < 24 {
        rng.fill(&mut byte).expect("the system's random source works");
        // 248 = 4 × 62: higher bytes are skipped, so every character is as likely.
        if byte[0] < 248 {
            out.push(CHARS[(byte[0] % 62) as usize] as char);
        }
    }
    out
}

/// Each engine's main account: MySQL's `root`, PostgreSQL's `postgres`, MongoDB's `root`.
pub fn main_account(engine: Engine) -> &'static str {
    match engine {
        Engine::Postgresql => "postgres",
        _ => "root",
    }
}

/// Account and database names Devid makes: lowercase letters, digits and `_`, starting with a
/// letter or `_`, at most 32 characters (MySQL's limit for account names). They need no quoting in
/// SQL, in MongoDB or in `.env`, and PostgreSQL keeps them as typed.
pub fn check_name(what: &str, name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 32
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    match ok {
        true => Ok(()),
        false => Err(format!(
            "{what} {name:?}: use lowercase letters, digits and _, starting with a letter or _ (at most 32)"
        )),
    }
}

/// A password the user picks: 8 to 64 letters, digits and `-_.~!@#%^*+=`, nothing that needs
/// quoting in `.env` or SQL.
pub fn check_password(password: &str) -> Result<(), String> {
    let ok = (8..=64).contains(&password.len())
        && password.chars().all(|c| c.is_ascii_alphanumeric() || "-_.~!@#%^*+=".contains(c));
    match ok {
        true => Ok(()),
        false => Err("a password needs 8 to 64 letters, digits and -_.~!@#%^*+= (nothing to quote)".into()),
    }
}

/// Devid's own way into a server it just started for the first time (D-061): MySQL gets the account
/// `devid`, MongoDB the administrator `devid` (through MongoDB's localhost exception, D1) and the
/// main account `root`, all with generated passwords kept in `accounts.toml`. PostgreSQL needs
/// none: its socket and `postgres` from this computer are trusted. Does nothing once done.
pub fn bootstrap(paths: &Paths, engine: Engine, series: &str, port: u16) -> io::Result<()> {
    let file = accounts_file(paths, engine, series);
    if engine == Engine::Postgresql || file.exists() {
        return Ok(());
    }
    let mut accounts = Accounts { devid: password(), users: BTreeMap::new() };
    match engine {
        Engine::Mysql => {
            let mut cmd = Command::new(bin(paths, Engine::Mysql, series, "mysql"));
            cmd.args(["--no-defaults", "-uroot", "--batch"]);
            cmd.args(mysql_target(paths, port));
            feed(
                &mut cmd,
                &format!(
                    "CREATE USER IF NOT EXISTS 'devid'@'localhost' IDENTIFIED BY '{}';\n\
                     GRANT ALL ON *.* TO 'devid'@'localhost' WITH GRANT OPTION;\n",
                    accounts.devid
                ),
                "mysql",
            )?;
        }
        Engine::Mongodb => {
            let root = password();
            mongo_script(
                paths,
                port,
                &format!(
                    "db.getSiblingDB('admin').createUser({{user: 'devid', pwd: '{0}', roles: ['root']}});\n\
                     db.getSiblingDB('admin').auth('devid', '{0}');\n\
                     db.getSiblingDB('admin').createUser({{user: 'root', pwd: '{1}', roles: ['root']}});\n",
                    accounts.devid, root
                ),
            )?;
            accounts.users.insert("root".into(), Account { password: root, database: Some("admin".into()) });
        }
        Engine::Postgresql => unreachable!(),
    }
    write_accounts(&file, &accounts)
}

/// How a MySQL client reaches Devid's server: the socket when it fits and is there, else 127.0.0.1
/// and the port. The host is always given, so a `host` in the user's option files cannot send the
/// client elsewhere.
pub fn mysql_target(paths: &Paths, port: u16) -> Vec<String> {
    let sock = socket(paths, Engine::Mysql, port);
    match socket_fits(&sock) && sock.exists() {
        true => vec!["--host=localhost".into(), format!("--socket={}", sock.display())],
        false => vec!["--host=127.0.0.1".into(), format!("--port={port}")],
    }
}

/// Where a PostgreSQL client finds Devid's server (`-h`, `PGHOST`): the folder of its socket when
/// the socket fits and is there, else 127.0.0.1.
pub fn pg_host(paths: &Paths, port: u16) -> String {
    let sock = socket(paths, Engine::Postgresql, port);
    match socket_fits(&sock) && sock.exists() {
        true => paths.run_dir().display().to_string(),
        false => "127.0.0.1".into(),
    }
}

/// Run a mongosh script against the server on `port` and return what it printed; the script,
/// credentials included, goes in through a file only the user can read rather than the command line.
fn mongo_script(paths: &Paths, port: u16, js: &str) -> io::Result<String> {
    let script = paths.run_dir().join(format!("mongosh-{}.js", std::process::id()));
    let _ = fs::remove_file(&script);
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&script)?;
    file.write_all(js.as_bytes())?;
    drop(file);
    let mut cmd = Command::new(mongosh(paths));
    cmd.args(["--quiet", "--norc", &format!("mongodb://127.0.0.1:{port}/admin"), "--file"]).arg(&script);
    let result = feed(&mut cmd, "", "mongosh");
    let _ = fs::remove_file(&script);
    result
}

/// Run `cmd` with `input` on stdin; what it printed, or why it failed.
fn feed(cmd: &mut Command, input: &str, what: &str) -> io::Result<String> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| err(format!("{what}: {e}")))?;
    child.stdin.take().expect("stdin is piped").write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let text = String::from_utf8_lossy(&out.stderr);
    let text = if text.trim().is_empty() { String::from_utf8_lossy(&out.stdout) } else { text };
    Err(err(format!("{what} failed ({}): {}", out.status, tail(text.trim(), 600))))
}

/// Devid's own way into a running server (D-061), for accounts and databases.
pub struct Admin<'a> {
    paths: &'a Paths,
    engine: Engine,
    series: &'a str,
    port: u16,
}

impl<'a> Admin<'a> {
    pub fn new(paths: &'a Paths, engine: Engine, series: &'a str, port: u16) -> Self {
        Self { paths, engine, series, port }
    }

    fn accounts(&self) -> io::Result<Accounts> {
        match self.engine {
            Engine::Postgresql => Ok(read_accounts(self.paths, self.engine, self.series).unwrap_or_default()),
            _ => read_accounts(self.paths, self.engine, self.series).filter(|a| !a.devid.is_empty()).ok_or_else(|| {
                err(format!(
                    "Devid's own account on {} is missing; `devid db start {}` sets it up",
                    self.engine.label(),
                    self.engine.name()
                ))
            }),
        }
    }

    fn save(&self, accounts: &Accounts) -> io::Result<()> {
        write_accounts(&accounts_file(self.paths, self.engine, self.series), accounts)
    }

    /// SQL as Devid's own MySQL account, or as `postgres` through PostgreSQL's socket (or from
    /// 127.0.0.1, which is trusted for it), in `database`; what it printed.
    fn sql(&self, database: &str, sql: &str) -> io::Result<String> {
        match self.engine {
            Engine::Mysql => {
                let cnf = self.paths.run_dir().join(format!("mysql-devid-{}.cnf", std::process::id()));
                let _ = fs::remove_file(&cnf);
                let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&cnf)?;
                write!(file, "[client]\nuser=devid\npassword={}\n", self.accounts()?.devid)?;
                drop(file);
                let mut cmd = Command::new(bin(self.paths, Engine::Mysql, self.series, "mysql"));
                cmd.arg(format!("--defaults-file={}", cnf.display())).args(["--batch", "-N"]);
                cmd.args(mysql_target(self.paths, self.port));
                let out = feed(&mut cmd, sql, "mysql");
                let _ = fs::remove_file(&cnf);
                out
            }
            Engine::Postgresql => {
                let host = pg_host(self.paths, self.port);
                let mut cmd = Command::new(bin(self.paths, Engine::Postgresql, self.series, "psql"));
                cmd.args(["-X", "-q", "-A", "-t", "-w", "-v", "ON_ERROR_STOP=1", "-h", &host])
                    .args(["-p", &self.port.to_string(), "-U", "postgres", "-d", database, "-f", "-"])
                    .env_remove("PGPASSWORD");
                feed(&mut cmd, sql, "psql")
            }
            Engine::Mongodb => unreachable!("MongoDB takes scripts"),
        }
    }

    /// A mongosh script as Devid's administrator.
    fn js(&self, js: &str) -> io::Result<String> {
        let devid = self.accounts()?.devid;
        mongo_script(
            self.paths,
            self.port,
            &format!("db.getSiblingDB('admin').auth('devid', {});\n{js}", quote(&devid)),
        )
    }

    /// The accounts on the server that people sign in with, Devid's own left out.
    pub fn users(&self) -> io::Result<Vec<String>> {
        let out = match self.engine {
            Engine::Mysql => self.sql(
                "",
                "SELECT User FROM mysql.user WHERE User NOT IN ('devid', '') AND User NOT LIKE 'mysql.%' ORDER BY User;",
            )?,
            Engine::Postgresql => self.sql("postgres", "SELECT rolname FROM pg_roles WHERE rolcanlogin ORDER BY 1;")?,
            Engine::Mongodb => self.js(
                "db.getSiblingDB('admin').system.users.find({user: {$ne: 'devid'}}, {user: 1}).toArray()\
                 .forEach(u => print(u.user));",
            )?,
        };
        let mut users: Vec<String> = out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        users.sort();
        users.dedup();
        Ok(users)
    }

    /// Make an account with `password`, and by default a database of its own with every right on
    /// it (on MongoDB it lives in that database, and reads and writes it).
    pub fn add_user(&self, name: &str, database: Option<&str>, password: &str) -> io::Result<()> {
        let mut accounts = self.accounts()?;
        match self.engine {
            Engine::Mysql => {
                let mut sql = format!("CREATE USER '{name}'@'%' IDENTIFIED BY '{password}';\n");
                if let Some(db) = database {
                    sql =
                        format!("CREATE DATABASE IF NOT EXISTS `{db}`;\n{sql}GRANT ALL ON `{db}`.* TO '{name}'@'%';\n");
                }
                self.sql("", &sql)?;
            }
            Engine::Postgresql => {
                self.sql("postgres", &format!("CREATE ROLE {name} LOGIN PASSWORD '{password}';\n"))?;
                if let Some(db) = database {
                    let owned = self.pg_own_database(name, db);
                    if owned.is_err() {
                        let _ = self.sql("postgres", &format!("DROP ROLE {name};\n"));
                    }
                    owned?;
                }
            }
            Engine::Mongodb => {
                let db = database.unwrap_or(name);
                self.js(&format!(
                    "db.getSiblingDB({0}).createUser({{user: {1}, pwd: {2}, roles: [{{role: 'readWrite', db: {0}}}, {{role: 'dbAdmin', db: {0}}}]}});",
                    quote(db),
                    quote(name),
                    quote(password)
                ))?;
            }
        }
        let database = match self.engine {
            Engine::Mongodb => Some(database.unwrap_or(name).to_string()),
            _ => database.map(String::from),
        };
        accounts.users.insert(name.into(), Account { password: password.into(), database });
        self.save(&accounts)
    }

    /// Give an account a new password; the main one included on MySQL and MongoDB.
    pub fn set_password(&self, name: &str, password: &str) -> io::Result<()> {
        let mut accounts = self.accounts()?;
        match self.engine {
            Engine::Mysql => {
                let users: Vec<String> = self
                    .mysql_hosts(name)?
                    .iter()
                    .map(|host| format!("'{name}'@'{}' IDENTIFIED BY '{password}'", mysql_str(host)))
                    .collect();
                self.sql("", &format!("ALTER USER {};\n", users.join(", ")))?;
            }
            Engine::Postgresql => {
                self.sql("postgres", &format!("ALTER ROLE {name} PASSWORD '{password}';\n"))?;
            }
            Engine::Mongodb => {
                let db = self.mongo_db(&accounts, name)?;
                self.js(&format!(
                    "db.getSiblingDB({}).changeUserPassword({}, {});",
                    quote(&db),
                    quote(name),
                    quote(password)
                ))?;
            }
        }
        let entry = accounts.users.entry(name.into()).or_default();
        entry.password = password.into();
        if self.engine == Engine::Mongodb && entry.database.is_none() {
            entry.database = Some(self.mongo_db(&Accounts::default(), name)?);
        }
        self.save(&accounts)
    }

    /// Remove an account; its databases and their data stay (on PostgreSQL they pass to `postgres`).
    pub fn remove_user(&self, name: &str) -> io::Result<()> {
        let mut accounts = self.accounts()?;
        match self.engine {
            Engine::Mysql => {
                let users: Vec<String> =
                    self.mysql_hosts(name)?.iter().map(|host| format!("'{name}'@'{}'", mysql_str(host))).collect();
                self.sql("", &format!("DROP USER {};\n", users.join(", ")))?;
            }
            Engine::Postgresql => {
                // The role's tables and databases pass to postgres, database by database.
                let dbs = self.sql("postgres", "SELECT datname FROM pg_database WHERE datallowconn ORDER BY 1;")?;
                for db in dbs.lines().map(str::trim).filter(|l| !l.is_empty()) {
                    self.sql(db, &format!("REASSIGN OWNED BY {name} TO postgres;\nDROP OWNED BY {name};\n"))?;
                }
                self.sql("postgres", &format!("DROP ROLE {name};\n"))?;
            }
            Engine::Mongodb => {
                let db = self.mongo_db(&accounts, name)?;
                self.js(&format!("db.getSiblingDB({}).dropUser({});", quote(&db), quote(name)))?;
            }
        }
        accounts.users.remove(name);
        self.save(&accounts)
    }

    /// A database without an account of its own; MongoDB makes one when an app first writes to it.
    pub fn create_database(&self, name: &str) -> io::Result<()> {
        match self.engine {
            Engine::Mysql => self.sql("", &format!("CREATE DATABASE `{name}`;\n")).map(|_| ()),
            Engine::Postgresql => self.sql("postgres", &format!("CREATE DATABASE {name};\n")).map(|_| ()),
            Engine::Mongodb => Ok(()),
        }
    }

    /// The hosts a MySQL account has: `%` for the ones Devid makes, `localhost` for `root`.
    fn mysql_hosts(&self, name: &str) -> io::Result<Vec<String>> {
        let out = self.sql("", &format!("SELECT Host FROM mysql.user WHERE User = '{name}';\n"))?;
        let hosts: Vec<String> = out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        match hosts.is_empty() {
            true => Err(err(format!("MySQL has no account {name:?}"))),
            false => Ok(hosts),
        }
    }

    /// Give a new PostgreSQL account its database: a new one, or `db` if it is there already.
    fn pg_own_database(&self, name: &str, db: &str) -> io::Result<()> {
        let there = self.sql("postgres", &format!("SELECT 1 FROM pg_database WHERE datname = '{db}';\n"))?;
        // CREATE DATABASE cannot run in a transaction, so on its own.
        let sql = match there.trim().is_empty() {
            true => format!("CREATE DATABASE {db} OWNER {name};\n"),
            false => format!("ALTER DATABASE {db} OWNER TO {name};\n"),
        };
        self.sql("postgres", &sql).map(|_| ())
    }

    /// The database a MongoDB account lives in: from accounts.toml, else asked of the server.
    fn mongo_db(&self, accounts: &Accounts, name: &str) -> io::Result<String> {
        if let Some(db) = accounts.users.get(name).and_then(|a| a.database.clone()) {
            return Ok(db);
        }
        let out = self.js(&format!(
            "const u = db.getSiblingDB('admin').system.users.findOne({{user: {}}}); if (u) print(u.db);",
            quote(name)
        ))?;
        out.lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(String::from)
            .ok_or_else(|| err(format!("MongoDB has no account {name:?}")))
    }
}

/// A MySQL string's inside, for a host name read back from `mysql.user`.
fn mysql_str(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "''")
}

/// A JavaScript string literal for mongosh.
fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("a string serializes")
}

/// The `.env` lines a Laravel app needs for an account (D-061). On MongoDB, `DB_URI` carries the
/// account and the database it lives in (laravel-mongodb reads it).
pub fn env_lines(engine: Engine, port: u16, user: &str, password: &str, database: Option<&str>) -> Vec<String> {
    match engine {
        Engine::Mongodb => {
            let db = database.unwrap_or("admin");
            // A password of the user's own may hold `@` or `%`, which a URI must escape.
            let password = uri_escape(password);
            vec![
                "DB_CONNECTION=mongodb".into(),
                format!("DB_URI=mongodb://{user}:{password}@127.0.0.1:{port}/{db}?authSource={db}"),
                format!("DB_DATABASE={db}"),
            ]
        }
        _ => {
            let mut lines = vec![
                format!("DB_CONNECTION={}", if engine == Engine::Mysql { "mysql" } else { "pgsql" }),
                "DB_HOST=127.0.0.1".into(),
                format!("DB_PORT={port}"),
            ];
            // An account without a database of its own (the main one) leaves DB_DATABASE as the app
            // has it: an empty value would mean no database, while Laravel's default (`laravel`) is
            // one `artisan migrate` offers to create.
            if let Some(db) = database {
                lines.push(format!("DB_DATABASE={db}"));
            }
            lines.push(format!("DB_USERNAME={user}"));
            lines.push(format!("DB_PASSWORD={password}"));
            lines
        }
    }
}

/// Percent-escape everything in `text` but RFC 3986's unreserved characters.
fn uri_escape(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The clients Devid puts in `~/.devid/bin` once an engine is installed (D-053), each set to reach
/// Devid's own server unless told otherwise.
pub const CLIENTS: [&str; 6] = ["mysql", "mysqldump", "psql", "pg_dump", "pg_restore", "mongosh"];

pub fn clients(engine: Engine) -> &'static [&'static str] {
    match engine {
        Engine::Mysql => &CLIENTS[0..2],
        Engine::Postgresql => &CLIENTS[2..5],
        Engine::Mongodb => &CLIENTS[5..],
    }
}

pub fn client_engine(name: &str) -> Option<Engine> {
    ENGINES.into_iter().find(|e| clients(*e).contains(&name))
}

/// The program behind a client: the series' own, or mongosh's folder.
pub fn client_bin(paths: &Paths, engine: Engine, series: &str, name: &str) -> PathBuf {
    match name {
        "mongosh" => mongosh(paths),
        _ => bin(paths, engine, series, name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(home: &str) -> Paths {
        Paths { home: PathBuf::from(home) }
    }

    fn args(spec: &Spec) -> Vec<String> {
        spec.args.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn engines_parse_and_have_their_usual_ports() {
        assert_eq!(Engine::parse("MySQL"), Some(Engine::Mysql));
        assert_eq!(Engine::parse("postgres"), Some(Engine::Postgresql));
        assert_eq!(Engine::parse("pg"), Some(Engine::Postgresql));
        assert_eq!(Engine::parse("mongo"), Some(Engine::Mongodb));
        assert_eq!(Engine::parse("redis"), None);
        let ports: Vec<u16> = ENGINES.iter().map(|e| e.usual_port()).collect();
        assert_eq!(ports, [3306, 5432, 27017]);
    }

    /// D1's settings, and the paths: binaries in db/, data in data/, logs in log/.
    #[test]
    fn each_server_runs_with_d1s_settings_on_127_0_0_1() {
        let p = paths("/h/.devid");
        let mysql = spec(&p, Engine::Mysql, "8.4", 3307);
        assert_eq!(mysql.program, PathBuf::from("/h/.devid/db/mysql-8.4/bin/mysqld"));
        let a = args(&mysql);
        for flag in [
            "--port=3307",
            "--bind-address=127.0.0.1",
            "--mysqlx=OFF",
            "--skip-log-bin",
            "--performance-schema=OFF",
            "--datadir=/h/.devid/data/mysql-8.4",
            "--socket=/h/.devid/run/mysql.sock",
        ] {
            assert!(a.iter().any(|x| x == flag), "{flag} in {a:?}");
        }
        assert_eq!(a[0], "--no-defaults", "MySQL reads no my.cnf of the system's");
        assert_eq!(
            (mysql.stop_signal, mysql.ready_port, mysql.stop_timeout),
            (libc::SIGTERM, Some(3307), STOP_TIMEOUT)
        );
        assert_eq!(mysql.log, Some(PathBuf::from("/h/.devid/log/mysql.log")));

        let pg = spec(&p, Engine::Postgresql, "18", 5433);
        let a = args(&pg).join(" ");
        assert!(a.contains("-D /h/.devid/data/postgresql-18 -p 5433 -c listen_addresses=127.0.0.1"), "{a}");
        assert!(a.contains("unix_socket_directories=/h/.devid/run") && a.contains("shared_buffers=32MB"), "{a}");
        assert_eq!(pg.pid_file, PathBuf::from("/h/.devid/data/postgresql-18/postmaster.pid"));
        assert_eq!(pg.stop_signal, libc::SIGINT, "PostgreSQL's fast shutdown");

        let mongo = spec(&p, Engine::Mongodb, "8.0", 27017);
        let a = args(&mongo).join(" ");
        assert!(
            a.contains("--bind_ip 127.0.0.1") && a.contains("--auth") && a.contains("--wiredTigerCacheSizeGB 0.25")
        );
        assert!(a.contains("diagnosticDataCollectionEnabled=false") && a.contains("--unixSocketPrefix /h/.devid/run"));
        let libs = mongo.env.iter().find(|(k, _)| k == "LD_LIBRARY_PATH");
        assert_eq!(libs.is_some(), cfg!(target_os = "linux"), "only Linux needs the bundled OpenSSL 3 (D-058)");
    }

    /// MySQL refuses a socket path over 103 bytes (D1); PostgreSQL and MongoDB run without one.
    #[test]
    fn a_long_home_turns_sockets_off_or_stops_mysql() {
        let long = format!("/{}", "x".repeat(100));
        let p = paths(&long);
        assert!(check_socket(&p, Engine::Mysql, 3306).unwrap_err().contains("shorter DEVID_HOME"));
        assert!(check_socket(&p, Engine::Postgresql, 5432).is_ok());
        assert!(args(&spec(&p, Engine::Postgresql, "18", 5432)).contains(&"unix_socket_directories=".to_string()));
        assert!(args(&spec(&p, Engine::Mongodb, "8.0", 27017)).contains(&"--nounixsocket".to_string()));
        assert!(check_socket(&paths("/Users/someone/.devid"), Engine::Mysql, 3306).is_ok());
    }

    #[test]
    fn the_port_is_the_usual_one_or_the_next_free() {
        let dbngin = |p: u16| p == 3306 || p == 5432;
        assert_eq!(pick_port(Engine::Mysql, &[], dbngin), Some(3307), "next to DBngin's MySQL");
        assert_eq!(pick_port(Engine::Postgresql, &[], dbngin), Some(5433));
        assert_eq!(pick_port(Engine::Mongodb, &[], dbngin), Some(27017));
        assert_eq!(pick_port(Engine::Mysql, &[3307], dbngin), Some(3308), "a port Devid uses is skipped");
        assert_eq!(pick_port(Engine::Mysql, &[], |_| true), None);
    }

    #[test]
    fn passwords_are_24_letters_and_digits_and_differ() {
        let (a, b) = (password(), password());
        assert_eq!(a.len(), 24);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn installs_find_series_and_keep_the_data() {
        let home = std::env::temp_dir().join(format!("devid-db-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let p = Paths { home: home.clone() };
        let unpacked = |name: &str, bin: &str| {
            let d = home.join("downloads").join(name);
            fs::create_dir_all(d.join("bin")).unwrap();
            fs::write(d.join("bin").join(bin), "#!/bin/sh\n").unwrap();
            d
        };
        let err = install(&p, Engine::Mysql, "8.4", &unpacked("wrong", "postgres"), "8.4.13").unwrap_err();
        assert!(err.to_string().contains("not a MySQL build"), "{err}");
        install(&p, Engine::Mysql, "9.7", &unpacked("m97", "mysqld"), "9.7.4").unwrap();
        install(&p, Engine::Mysql, "8.4", &unpacked("m84", "mysqld"), "8.4.13").unwrap();
        assert_eq!(installed_series(&p, Engine::Mysql), ["8.4", "9.7"]);
        assert_eq!(version(&p, Engine::Mysql, "8.4").as_deref(), Some("8.4.13"));
        // A newer build of the series replaces the binaries, never the data.
        fs::create_dir_all(data(&p, Engine::Mysql, "8.4").join("mysql")).unwrap();
        install(&p, Engine::Mysql, "8.4", &unpacked("m84b", "mysqld"), "8.4.14").unwrap();
        assert_eq!(version(&p, Engine::Mysql, "8.4").as_deref(), Some("8.4.14"));
        assert!(initialized(&p, Engine::Mysql, "8.4"), "the data folder stayed");
        // MongoDB counts as installed only with mongosh, which Devid's accounts need.
        install(&p, Engine::Mongodb, "8.0", &unpacked("g80", "mongod"), "8.0.38").unwrap();
        assert!(!installed(&p, Engine::Mongodb, "8.0"));
        install_mongosh(&p, &unpacked("sh", "mongosh"), "2.14.3").unwrap();
        assert!(installed(&p, Engine::Mongodb, "8.0"));
        assert_eq!(mongosh_version(&p).as_deref(), Some("2.14.3"));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn names_and_passwords_need_no_quoting() {
        for ok in ["shop", "_tmp", "app_2", "a2345678901234567890123456789012"] {
            assert!(check_name("account", ok).is_ok(), "{ok}");
        }
        for bad in ["", "Shop", "2shop", "my-app", "a b", "x'; DROP", "a23456789012345678901234567890123"] {
            assert!(check_name("account", bad).is_err(), "{bad}");
        }
        assert!(check_password("secret-1.2").is_ok());
        assert!(check_password(&password()).is_ok(), "Devid's own pass its own check");
        for bad in ["short", "has space1", "quote'1234", "back\\slash", "dollar$1234", &"x".repeat(65)] {
            assert!(check_password(bad).is_err(), "{bad}");
        }
    }

    /// The `.env` lines Laravel reads (DB_URI for laravel-mongodb).
    /// D2 kept the passwords alone; D3 adds the database, and reads D2's file as it is.
    #[test]
    fn accounts_from_d2_still_read() {
        let d2: Accounts = toml::from_str("devid = \"d\"\n[users]\nroot = \"r\"\n").unwrap();
        assert_eq!(d2.users["root"], Account { password: "r".into(), database: None });
        let mut now = d2.clone();
        now.users.insert("shop".into(), Account { password: "s".into(), database: Some("shop".into()) });
        let text = toml::to_string_pretty(&now).unwrap();
        assert!(text.contains("[users.shop]") && text.contains("database = \"shop\""), "{text}");
        assert_eq!(toml::from_str::<Accounts>(&text).unwrap(), now);
    }

    #[test]
    fn env_lines_for_laravel() {
        assert_eq!(
            env_lines(Engine::Mysql, 3307, "shop", "pw", Some("shop")),
            [
                "DB_CONNECTION=mysql",
                "DB_HOST=127.0.0.1",
                "DB_PORT=3307",
                "DB_DATABASE=shop",
                "DB_USERNAME=shop",
                "DB_PASSWORD=pw"
            ]
        );
        assert_eq!(
            env_lines(Engine::Postgresql, 5433, "postgres", "", None),
            ["DB_CONNECTION=pgsql", "DB_HOST=127.0.0.1", "DB_PORT=5433", "DB_USERNAME=postgres", "DB_PASSWORD="]
        );
        assert_eq!(
            env_lines(Engine::Mongodb, 27017, "shop", "pw", Some("shop"))[1],
            "DB_URI=mongodb://shop:pw@127.0.0.1:27017/shop?authSource=shop"
        );
        // No URI with credentials in the source, which secret scanners flag.
        let uri = &env_lines(Engine::Mongodb, 27017, "shop", "a@b%c!d", Some("shop"))[1];
        assert!(uri.contains(":a%40b%25c%21d@127.0.0.1"), "a password of the user's own is escaped: {uri}");
        assert_eq!(main_account(Engine::Postgresql), "postgres");
        assert_eq!(main_account(Engine::Mongodb), "root");
    }

    #[test]
    fn each_engine_brings_its_clients() {
        assert_eq!(clients(Engine::Mysql), ["mysql", "mysqldump"]);
        assert_eq!(clients(Engine::Postgresql), ["psql", "pg_dump", "pg_restore"]);
        assert_eq!(client_engine("mongosh"), Some(Engine::Mongodb));
        assert_eq!(client_engine("php"), None);
        let p = paths("/h/.devid");
        assert_eq!(
            client_bin(&p, Engine::Mongodb, "8.0", "mongosh"),
            PathBuf::from("/h/.devid/db/mongosh/bin/mongosh")
        );
        assert_eq!(
            client_bin(&p, Engine::Postgresql, "18", "psql"),
            PathBuf::from("/h/.devid/db/postgresql-18/bin/psql")
        );
    }

    #[test]
    fn accounts_are_written_for_the_user_only() {
        let home = std::env::temp_dir().join(format!("devid-db-accounts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let p = Paths { home: home.clone() };
        fs::create_dir_all(data(&p, Engine::Mongodb, "8.0")).unwrap();
        let file = accounts_file(&p, Engine::Mongodb, "8.0");
        let root = Account { password: "b".into(), database: Some("admin".into()) };
        let accounts = Accounts { devid: "a".into(), users: BTreeMap::from([("root".into(), root)]) };
        write_accounts(&file, &accounts).unwrap();
        assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(read_accounts(&p, Engine::Mongodb, "8.0"), Some(accounts));
        assert!(fs::read_to_string(&file).unwrap().starts_with("# Devid (D-061)"));
        fs::remove_dir_all(&home).unwrap();
    }
}
