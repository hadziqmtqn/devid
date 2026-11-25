//! Phase 14 D2 (D-061): Devid installs a database from its database release, sets up the data,
//! runs the server with the daemon and gives Devid its own account. Fake servers (a shell script
//! and perl, which listen on the port and make the socket as the real ones do) stand in for MySQL,
//! PostgreSQL and MongoDB; the real ones ran through the same steps in D1's spike (spike/db/).

mod common;

use common::*;
use serde_json::{Value, json};
use std::{fs, net::TcpListener, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum").args(["-a", "256"]).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap().to_string()
}

fn free_tcp_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// Listens on its port and makes its socket like a database, writes its pid file, and on SIGTERM
/// or SIGINT removes the socket and exits: perl <port> <socket or ""> <pid file or "">.
const SERVE: &str = r#"exec perl -MIO::Socket::INET -e '
my ($port, $sock, $pidfile) = @ARGV;
my $s = IO::Socket::INET->new(LocalAddr => "127.0.0.1", LocalPort => $port, Listen => 5, ReuseAddr => 1)
    or die "listen on $port: $!\n";
if ($pidfile) { open(my $p, ">", $pidfile) or die; print $p "$$\n"; close $p; }
if ($sock) { open(my $f, ">", $sock) or die; close $f; }
$SIG{TERM} = $SIG{INT} = sub { unlink $sock if $sock; exit 0 };
while (1) { my $c = $s->accept; close $c if $c; }
' "$port" "$sock" "$pidfile""#;

/// A fake server or client: `body` sets `port`, `sock` and `pidfile` from the arguments (or exits),
/// then SERVE runs. Every call is logged to $DEVID_HOME/db-calls.log.
fn script(name: &str, body: &str) -> String {
    format!("#!/bin/sh\necho \"{name} $*\" >> \"$DEVID_HOME/db-calls.log\"\nport=; sock=; pidfile=\n{body}\n")
}

fn mysqld() -> String {
    script(
        "mysqld",
        &format!(
            "for a in \"$@\"; do case \"$a\" in\n\
               --initialize-insecure) init=1 ;;\n\
               --datadir=*) data=\"${{a#--datadir=}}\" ;;\n\
               --port=*) port=\"${{a#--port=}}\" ;;\n\
               --socket=*) sock=\"${{a#--socket=}}\" ;;\n\
               --pid-file=*) pidfile=\"${{a#--pid-file=}}\" ;;\n\
             esac; done\n\
             if [ -n \"$init\" ]; then mkdir -p \"$data/mysql\"; exit 0; fi\n{SERVE}"
        ),
    )
}

fn postgres() -> String {
    script(
        "postgres",
        &format!(
            "while [ $# -gt 0 ]; do case \"$1\" in\n\
               -D) shift; data=\"$1\" ;;\n\
               -p) shift; port=\"$1\" ;;\n\
               unix_socket_directories=*) dir=\"${{1#unix_socket_directories=}}\" ;;\n\
             esac; shift; done\n\
             [ -n \"$dir\" ] && sock=\"$dir/.s.PGSQL.$port\"\n\
             pidfile=\"$data/postmaster.pid\"\n{SERVE}"
        ),
    )
}

fn mongod() -> String {
    script(
        "mongod",
        &format!(
            "while [ $# -gt 0 ]; do case \"$1\" in\n\
               --port) shift; port=\"$1\" ;;\n\
               --unixSocketPrefix) shift; dir=\"$1\" ;;\n\
               --pidfilepath) shift; pidfile=\"$1\" ;;\n\
             esac; shift; done\n\
             [ -n \"$dir\" ] && sock=\"$dir/mongodb-$port.sock\"\n{SERVE}"
        ),
    )
}

/// A client: logs its arguments, and its script (stdin, or mongosh's --file).
fn client(name: &str) -> String {
    script(
        name,
        "file=; prev=\n\
         for a in \"$@\"; do [ \"$prev\" = --file ] && file=\"$a\"; prev=\"$a\"; done\n\
         if [ -n \"$file\" ]; then cat \"$file\"; else cat; fi >> \"$DEVID_HOME/db-calls.log\"\n\
         exit 0",
    )
}

fn initdb() -> String {
    script(
        "initdb",
        "while [ $# -gt 0 ]; do [ \"$1\" = -D ] && { shift; mkdir -p \"$1\"; echo 18 > \"$1/PG_VERSION\"; }; shift; done\nexit 0",
    )
}

/// An archive with `files` (name -> script) in bin/, and its manifest entry for every platform.
fn asset(root: &Path, rel: &Path, name: &str, version: &str, files: &[(&str, String)]) -> Value {
    let stage = root.join(format!("stage-{name}"));
    fs::create_dir_all(stage.join("bin")).unwrap();
    for (file, text) in files {
        let path = stage.join("bin").join(file);
        fs::write(&path, text).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let archive = format!("{name}-{version}.tar.gz");
    let tar = Command::new("tar").arg("-czf").arg(rel.join(&archive)).arg("-C").arg(&stage).arg(".").status().unwrap();
    assert!(tar.success());
    let build = json!({"version": version, "url": format!("file://{}/{archive}", rel.display()),
                       "sha256": sha256(&rel.join(&archive)), "size": 1});
    json!({"version": version, "arch": {"x86_64": build.clone(), "arm64": build.clone(), "linux-x86_64": build}})
}

/// The database release (`mysql` is MySQL 8.4's build), and an empty runtime release, so `devid
/// update` reads nothing from the network.
fn publish(root: &Path, rel: &Path, tag: &str, mysql: &str) {
    let manifest = json!({
        "schema": 1, "tag": tag, "repo": "local/test",
        "mysql": {"8.4": asset(root, rel, "mysql-8.4", mysql, &[("mysqld", mysqld()), ("mysql", client("mysql"))])},
        "postgresql": {"18": asset(root, rel, "postgresql-18", "18.7",
            &[("postgres", postgres()), ("initdb", initdb()), ("psql", client("psql"))])},
        "mongodb": {"8.0": asset(root, rel, "mongodb-8.0", "8.0.38", &[("mongod", mongod())])},
        "mongosh": asset(root, rel, "mongosh", "2.14.3", &[("mongosh", client("mongosh"))]),
    });
    fs::write(rel.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let runtime = rel.join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    fs::write(runtime.join("manifest.json"), r#"{"schema": 1, "tag": "r1", "repo": "local/test", "php": {}}"#).unwrap();
}

fn database(home: &Path, engine: &str) -> Value {
    let list = json(home, &["status", "--json"])["databases"].clone();
    list.as_array().unwrap().iter().find(|d| d["engine"] == engine).unwrap().clone()
}

fn calls(home: &Path) -> String {
    fs::read_to_string(home.join("db-calls.log")).unwrap_or_default()
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn pid_of(home: &Path, engine: &str) -> u64 {
    database(home, engine)["pid"].as_u64().unwrap_or_else(|| panic!("{engine} has no pid"))
}

#[test]
fn databases_install_start_stop_and_come_back() {
    let root = temp_root("devid-db-it");
    let home = root.join("home");
    let rel = root.join("release");
    fs::create_dir_all(&rel).unwrap();
    fs::create_dir_all(&home).unwrap();
    publish(&root, &rel, "db-test-1", "8.4.13");
    let (my, pg, mongo) = (free_tcp_port(), free_tcp_port(), free_tcp_port());
    fs::write(
        home.join("config.toml"),
        format!(
            "http_port = {TEST_HTTP_PORT}\nhttps_port = {TEST_HTTPS_PORT}\ndns_port = {}\nupdate_check_minutes = 0\n\
             [databases.mysql]\nport = {my}\n[databases.postgresql]\nport = {pg}\n[databases.mongodb]\nport = {mongo}\n",
            free_udp_port()
        ),
    )
    .unwrap();
    let url = format!("file://{}/manifest.json", rel.display());
    let runtime = format!("file://{}/runtime/manifest.json", rel.display());
    let env = [("DEVID_DB_MANIFEST_URL", url.as_str()), ("DEVID_MANIFEST_URL", runtime.as_str())];
    let daemon = start_daemon_env(&home, &env);

    // Off and not installed at first; `devid db` says how to start each.
    assert_eq!(database(&home, "mysql")["state"], "off");
    let list = out(&devid(&home, &["db"]));
    assert!(list.contains("`devid db start postgresql` starts it"), "{list}");
    let available = out(&devid(&home, &["db", "list", "--available"]));
    assert!(available.contains("db-test-1") && available.contains("mysql       8.4"), "{available}");

    // MySQL: downloaded, set up without a root password, run with D1's settings on its port, and
    // given Devid's own account through root.
    let started = devid(&home, &["db", "start", "mysql"]);
    assert!(started.status.success(), "{}", stderr(&started));
    assert!(out(&started).contains(&format!("MySQL 8.4.13 runs on 127.0.0.1:{my}")), "{}", out(&started));
    let d = database(&home, "mysql");
    assert_eq!(
        (d["state"].as_str(), d["series"].as_str(), d["port"].as_u64()),
        (Some("running"), Some("8.4"), Some(my as u64))
    );
    assert!(home.join("data/mysql-8.4/mysql").is_dir(), "set up by --initialize-insecure");
    let log = calls(&home);
    assert!(log.contains("--initialize-insecure"), "{log}");
    for flag in ["--mysqlx=OFF", "--skip-log-bin", "--performance-schema=OFF", "--bind-address=127.0.0.1"] {
        assert!(log.contains(flag), "{flag}: {log}");
    }
    assert!(log.contains("CREATE USER IF NOT EXISTS 'devid'@'localhost'"), "{log}");
    let accounts = home.join("data/mysql-8.4/accounts.toml");
    assert_eq!(fs::metadata(&accounts).unwrap().permissions().mode() & 0o777, 0o600, "only the user reads it");
    assert!(fs::read_to_string(&accounts).unwrap().contains("devid = "));

    // PostgreSQL: initdb with trust, Devid's own pg_hba.conf, no account of Devid's needed.
    let started = devid(&home, &["db", "start", "pg"]);
    assert!(started.status.success(), "{}", stderr(&started));
    assert_eq!(database(&home, "postgresql")["state"], "running");
    let hba = fs::read_to_string(home.join("data/postgresql-18/pg_hba.conf")).unwrap();
    assert!(hba.contains("postgres   127.0.0.1/32   trust") && hba.contains("all        127.0.0.1/32   scram-sha-256"));
    assert!(!home.join("data/postgresql-18/accounts.toml").exists());

    // MongoDB brings mongosh, runs with access control, and Devid creates its administrator and
    // the main account `root` through the localhost exception.
    let started = devid(&home, &["db", "start", "mongodb"]);
    assert!(started.status.success(), "{}", stderr(&started));
    assert!(home.join("db/mongosh/bin/mongosh").is_file());
    let log = calls(&home);
    assert!(log.contains("--auth") && log.contains("--wiredTigerCacheSizeGB 0.25"), "{log}");
    assert!(log.contains("createUser({user: 'devid'") && log.contains("createUser({user: 'root'"), "{log}");
    assert!(fs::read_to_string(home.join("data/mongodb-8.0/accounts.toml")).unwrap().contains("root = "));

    // A series the release does not build for this computer.
    let missing = devid(&home, &["db", "install", "mysql", "9.7"]);
    assert!(stderr(&missing).contains("MySQL 9.7 is not built for this computer; it has 8.4"), "{}", stderr(&missing));
    assert!(stderr(&devid(&home, &["db", "start", "redis"])).contains("unknown database"));

    // `devid stop` pauses a database like any service; `devid db stop` turns it off for good.
    let pid = pid_of(&home, "mysql");
    assert!(devid(&home, &["stop", "mysql"]).status.success());
    wait_for("mysql paused", || database(&home, "mysql")["state"] == "paused");
    assert!(!alive(pid), "the server stopped");
    assert!(devid(&home, &["start", "mysql"]).status.success());
    wait_for("mysql back", || database(&home, "mysql")["state"] == "running");
    let pg_pid = pid_of(&home, "postgresql");
    assert!(devid(&home, &["db", "stop", "postgresql"]).status.success());
    assert_eq!(database(&home, "postgresql")["state"], "off");
    assert!(!alive(pg_pid));
    assert!(stderr(&devid(&home, &["db", "stop", "postgresql"])).contains("is not on"));

    // A newer build of a series: `devid update` replaces the binaries and restarts the server on
    // them; the data stays.
    assert!(out(&devid(&home, &["update", "--check"])).contains("everything is up to date"));
    publish(&root, &rel, "db-test-2", "8.4.14");
    let check = out(&devid(&home, &["update", "--check"]));
    assert!(check.contains("available: mysql 8.4 8.4.13 -> 8.4.14"), "{check}");
    let before = pid_of(&home, "mysql");
    let updated = devid(&home, &["update"]);
    assert!(updated.status.success(), "{}", stderr(&updated));
    wait_for("mysql on 8.4.14", || {
        let d = database(&home, "mysql");
        d["version"] == "8.4.14" && d["state"] == "running" && d["pid"].as_u64() != Some(before)
    });
    assert!(home.join("data/mysql-8.4/mysql").is_dir() && home.join("data/mysql-8.4/accounts.toml").exists());

    // After the daemon restarts, the databases that are on start again with it; the data stays.
    let (my_pid, mongo_pid) = (pid_of(&home, "mysql"), pid_of(&home, "mongodb"));
    daemon.stop();
    assert!(!alive(my_pid) && !alive(mongo_pid), "the daemon stopped its databases");
    let daemon = start_daemon_env(&home, &env);
    wait_for("mysql and mongodb again", || {
        database(&home, "mysql")["state"] == "running" && database(&home, "mongodb")["state"] == "running"
    });
    assert_eq!(database(&home, "postgresql")["state"], "off");
    assert_eq!(fs::read_to_string(home.join("data/postgresql-18/PG_VERSION")).unwrap().trim(), "18");
    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
