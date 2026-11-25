#!/usr/bin/env python3
"""Phase 14 D1 spike (D-061): run MySQL, PostgreSQL and MongoDB the way Devid would, measure them,
and check accounts and passwords.

  spike/db/spike.py <work> [--only mysql-8.4,...] [--ports base|usual] [--port-base 43000]
                    [--sock-dir DIR] [--idle 15] [--laravel]

<work> is what fetch.py filled. Each server runs on 127.0.0.1 only, with the soft open-files limit
Devid's daemon passes on (256 under launchd, 1024 under a systemd user unit). The default series of
each engine runs twice, with its defaults and with Devid's settings (MySQL a third time, smaller);
the other series run with Devid's settings only. Results go to <work>/results.jsonl, and a Markdown
summary to stdout. Ports: `base` stays clear of the usual ones (for CI); `usual` takes the usual port
or the next free one, as Devid will. Nothing outside <work> and --sock-dir is touched."""
import argparse
import json
import os
import platform
import secrets
import shutil
import signal
import socket
import string
import subprocess
import time

LINUX = platform.system() == "Linux"
SOFT_NOFILE = 1024 if LINUX else 256
USUAL = {"mysql": 3306, "postgresql": 5432, "mongodb": 27017}
DEFAULT_SERIES = {"mysql": "8.4", "postgresql": "18", "mongodb": "8.0"}
SOCK_MAX = 103  # sun_path holds 104 bytes on macOS, the final NUL included

PHP_CHECK = r"""<?php
// php check.php pdo <dsn> <user> <password>   |   php check.php mongo <uri> <database>
[$kind, $dsn, $a, $b] = array_slice($argv, 1) + [null, null, null, null];
try {
    if ($kind === 'pdo') {
        $pdo = new PDO($dsn, $a, $b, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION, PDO::ATTR_TIMEOUT => 5]);
        echo 'ok ', $pdo->query('SELECT 1')->fetchColumn(), "\n";
    } else {
        $m = new MongoDB\Driver\Manager($dsn, ['serverSelectionTimeoutMS' => 5000]);
        $w = new MongoDB\Driver\BulkWrite();
        $w->insert(['s' => 'ok']);
        $m->executeBulkWrite($a . '.t', $w);
        echo "ok 1\n";
    }
} catch (Throwable $e) {
    echo 'error ', get_class($e), ': ', $e->getMessage(), "\n";
    exit(1);
}
"""

HBA = """# Devid (Phase 14 spike): the socket and the main account need no password; every other account does.
local   all          all                       trust
host    all          postgres   127.0.0.1/32   trust
host    all          postgres   ::1/128        trust
host    all          all        127.0.0.1/32   scram-sha-256
host    all          all        ::1/128        scram-sha-256
local   replication  all                       trust
host    replication  all        127.0.0.1/32   scram-sha-256
"""


def password(n=24):
    chars = string.ascii_letters + string.digits
    return "".join(secrets.choice(chars) for _ in range(n))


def port_in_use(port):
    for family, host in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
        s = socket.socket(family, socket.SOCK_STREAM)
        s.settimeout(0.3)
        try:
            if s.connect_ex((host, port)) == 0:
                return True
        except OSError:
            pass
        finally:
            s.close()
    return False


def pick_port(start):
    port = start
    while port_in_use(port):
        port += 1
    return port


def sh(cmd, env=None, timeout=180, cwd=None):
    try:
        r = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=timeout, cwd=cwd)
        return r.returncode, (r.stdout + r.stderr).strip()
    except subprocess.TimeoutExpired:
        return 124, "timeout"
    except OSError as e:
        return 127, str(e)


def ok(code_out, want=None):
    code, out = code_out
    good = code == 0 and (want is None or want in out)
    return {"ok": good, "out": out[-300:]}


def refused(code_out):
    code, out = code_out
    return {"ok": code != 0, "out": out[-300:]}


def tree(root):
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid="], capture_output=True, text=True).stdout
    children = {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 2:
            children.setdefault(int(parts[1]), []).append(int(parts[0]))
    pids, todo = [], [root]
    while todo:
        pid = todo.pop()
        pids.append(pid)
        todo.extend(children.get(pid, []))
    return pids


def memory(root):
    pids = tree(root)
    out = subprocess.run(["ps", "-o", "rss=", "-p", ",".join(map(str, pids))],
                         capture_output=True, text=True).stdout
    rss = sum(int(x) for x in out.split() if x.isdigit())
    result = {"processes": len(pids), "rss_mb": round(rss / 1024, 1)}
    if LINUX:
        pss = 0
        for pid in pids:
            try:
                with open(f"/proc/{pid}/smaps_rollup") as f:
                    for line in f:
                        if line.startswith("Pss:"):
                            pss += int(line.split()[1])
            except OSError:
                pass
        result["pss_mb"] = round(pss / 1024, 1)
    return result


class Server:
    def __init__(self, work, engine, series, version, port, profile, sock_dir):
        self.engine, self.series, self.version = engine, series, version
        self.port, self.profile, self.sock_dir = port, profile, sock_dir
        self.root = os.path.join(work, f"{engine}-{series}")
        self.base = os.path.join(work, "run", f"{engine}-{series}-{profile}")
        self.data = os.path.join(self.base, "data")
        self.log = os.path.join(self.base, "server.log")
        self.proc = None
        self.env = dict(os.environ)
        if engine == "mongodb" and LINUX:
            self.env["LD_LIBRARY_PATH"] = os.path.join(self.root, "lib")
        shutil.rmtree(self.base, ignore_errors=True)
        os.makedirs(self.base)

    def bin(self, name):
        return os.path.join(self.root, "bin", name)

    def socket_path(self):
        name = {"mysql": "my.sock", "postgresql": f".s.PGSQL.{self.port}",
                "mongodb": f"mongodb-{self.port}.sock"}[self.engine]
        return os.path.join(self.sock_dir, name)

    def sockets(self):
        return len(self.socket_path().encode()) <= SOCK_MAX

    def init(self):
        t = time.monotonic()
        if self.engine == "mysql":
            r = sh([self.bin("mysqld"), "--no-defaults", "--initialize-insecure", f"--basedir={self.root}",
                    f"--datadir={self.data}", f"--log-error={self.log}"], env=self.env, timeout=600)
        elif self.engine == "postgresql":
            # initdb refuses a password method for the host lines without a superuser password, so
            # it starts with trust and Devid writes pg_hba.conf itself.
            r = sh([self.bin("initdb"), "-D", self.data, "-U", "postgres", "-A", "trust", "-E", "UTF8",
                    "--locale=C"], env=self.env, timeout=600)
            if r[0] == 0:
                with open(os.path.join(self.data, "pg_hba.conf"), "w") as f:
                    f.write(HBA)
        else:
            os.makedirs(self.data)
            r = (0, "")
        return r[0], r[1][-400:], round(time.monotonic() - t, 2)

    def command(self):
        if self.engine == "mysql":
            c = [self.bin("mysqld"), "--no-defaults", f"--basedir={self.root}", f"--datadir={self.data}",
                 f"--port={self.port}", "--bind-address=127.0.0.1", f"--socket={self.socket_path()}",
                 "--mysqlx=OFF", f"--log-error={self.log}", f"--pid-file={os.path.join(self.base, 'mysqld.pid')}"]
            if self.profile != "default":
                c += ["--skip-log-bin", "--performance-schema=OFF"]
            if self.profile == "small":
                c += ["--innodb-buffer-pool-size=64M", "--max-connections=50"]
            return c
        if self.engine == "postgresql":
            c = [self.bin("postgres"), "-D", self.data, "-p", str(self.port), "-c", "listen_addresses=127.0.0.1",
                 "-c", "unix_socket_directories=" + (self.sock_dir if self.sockets() else "")]
            if self.profile != "default":
                c += ["-c", "shared_buffers=32MB", "-c", "max_connections=50"]
            return c
        c = [self.bin("mongod"), "--dbpath", self.data, "--port", str(self.port), "--bind_ip", "127.0.0.1",
             "--logpath", self.log, "--logappend", "--auth"]
        c += ["--unixSocketPrefix", self.sock_dir] if self.sockets() else ["--nounixsocket"]
        if self.profile != "default":
            c += ["--wiredTigerCacheSizeGB", "0.25", "--setParameter", "diagnosticDataCollectionEnabled=false"]
        return c

    def start(self):
        cmd = ["/bin/sh", "-c", f'ulimit -S -n {SOFT_NOFILE} && exec "$@"', "sh"] + self.command()
        with open(self.log, "ab") as logf:
            self.proc = subprocess.Popen(cmd, stdout=logf, stderr=subprocess.STDOUT, env=self.env,
                                         cwd=self.base, start_new_session=True)
        t = time.monotonic()
        ready = self.wait_ready(180)
        return ready, round(time.monotonic() - t, 2)

    def ready_once(self):
        if not port_in_use(self.port):
            return False
        if self.engine == "mysql":
            return sh([self.bin("mysqladmin"), "--no-defaults", "-uroot", "-h127.0.0.1", f"-P{self.port}",
                       "ping"], env=self.env, timeout=10)[0] == 0
        if self.engine == "postgresql":
            return sh([self.bin("pg_isready"), "-q", "-h", "127.0.0.1", "-p", str(self.port)],
                      env=self.env, timeout=10)[0] == 0
        try:
            with open(self.log, errors="replace") as f:
                return "Waiting for connections" in f.read()
        except OSError:
            return False

    def wait_ready(self, timeout):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if self.proc.poll() is not None:
                return False
            if self.ready_once():
                return True
            time.sleep(0.1)
        return False

    def stop(self):
        sig = signal.SIGINT if self.engine == "postgresql" else signal.SIGTERM
        t = time.monotonic()
        self.proc.send_signal(sig)
        try:
            code = self.proc.wait(180)
        except subprocess.TimeoutExpired:
            os.killpg(self.proc.pid, signal.SIGKILL)
            self.proc.wait()
            code = "killed after 180 s"
        return code, round(time.monotonic() - t, 2)

    def kill9(self):
        os.killpg(self.proc.pid, signal.SIGKILL)
        self.proc.wait()

    def identity(self):
        pid = self.proc.pid
        info = {"comm": sh(["ps", "-o", "comm=", "-p", str(pid)])[1]}
        if LINUX:
            try:
                info["exe"] = os.readlink(f"/proc/{pid}/exe")
            except OSError as e:
                info["exe"] = str(e)
        return info

    # Clients -------------------------------------------------------------------------------------

    def mysql(self, sql, user="root", pw=None, use_socket=False, db=None):
        cnf = os.path.join(self.base, "client.cnf")
        lines = ["[client]", f"user={user}"]
        if pw is not None:
            lines.append(f"password={pw}")
        lines += [f"socket={self.socket_path()}"] if use_socket else ["host=127.0.0.1", f"port={self.port}"]
        with open(cnf, "w") as f:
            f.write("\n".join(lines) + "\n")
        os.chmod(cnf, 0o600)
        cmd = [self.bin("mysql"), f"--defaults-file={cnf}", "-N", "-e", sql]
        if db:
            cmd.append(db)
        return sh(cmd, env=self.env, timeout=60)

    def psql(self, sql, user="postgres", pw=None, db="postgres", host="127.0.0.1"):
        env = dict(self.env)
        env.pop("PGPASSWORD", None)
        if pw is not None:
            env["PGPASSWORD"] = pw
        return sh([self.bin("psql"), "-X", "-q", "-A", "-t", "-w", "-v", "ON_ERROR_STOP=1", "-h", host,
                   "-p", str(self.port), "-U", user, "-d", db, "-c", sql], env=env, timeout=60)

    def mongosh(self, js, user=None, pw=None, db="admin", auth_db="admin"):
        if user:
            uri = f"mongodb://{user}:{pw}@127.0.0.1:{self.port}/{db}?authSource={auth_db}"
        else:
            uri = f"mongodb://127.0.0.1:{self.port}/{db}"
        mongosh = os.path.join(os.path.dirname(self.root), "mongosh", "bin", "mongosh")
        return sh([mongosh, "--quiet", "--norc", uri, "--eval", js], timeout=90)


def php(work, *args, mongo=False):
    exe = os.path.join(work, "php", "php")
    check = os.path.join(work, "check.php")
    if not os.path.exists(check):
        with open(check, "w") as f:
            f.write(PHP_CHECK)
    cmd = [exe, "-n"]
    if mongo:
        cmd += ["-d", "extension=" + os.path.join(work, "ext-mongodb", "mongodb.so")]
    return sh(cmd + [check] + list(args), timeout=60)


def accounts(s, work):
    """Create an account with a password, reset it, and check what is let in, over 127.0.0.1."""
    r = {}
    pw1, pw2 = password(), password()
    if s.engine == "mysql":
        r["root_tcp_no_password"] = ok(s.mysql("SELECT CURRENT_USER()"), "root@localhost")
        if s.sockets():
            r["root_socket"] = ok(s.mysql("SELECT 1", use_socket=True))
        admin = password()
        r["devid_account"] = ok(s.mysql(f"CREATE USER 'devid'@'localhost' IDENTIFIED BY '{admin}'; "
                                        "GRANT ALL ON *.* TO 'devid'@'localhost' WITH GRANT OPTION;"))
        r["create_account"] = ok(s.mysql(
            f"CREATE DATABASE shop; CREATE USER 'shop'@'%' IDENTIFIED BY '{pw1}'; "
            "GRANT ALL ON shop.* TO 'shop'@'%';", user="devid", pw=admin, use_socket=s.sockets()))
        dsn = f"mysql:host=127.0.0.1;port={s.port};dbname=shop"
        r["php_first_login"] = ok(php(work, "pdo", dsn, "shop", pw1), "ok 1")
        r["php_second_login"] = ok(php(work, "pdo", dsn, "shop", pw1), "ok 1")
        r["php_wrong_password_refused"] = refused(php(work, "pdo", dsn, "shop", "wrong"))
        r["reset"] = ok(s.mysql(f"ALTER USER 'shop'@'%' IDENTIFIED BY '{pw2}';", user="devid", pw=admin,
                                use_socket=s.sockets()))
        r["php_new_password"] = ok(php(work, "pdo", dsn, "shop", pw2), "ok 1")
        r["php_old_password_refused"] = refused(php(work, "pdo", dsn, "shop", pw1))
        r["client_with_password"] = ok(s.mysql("CREATE TABLE t (id INT PRIMARY KEY); INSERT INTO t VALUES (1);",
                                               user="shop", pw=pw2, db="shop"))
        root_pw = password()
        r["main_account_reset"] = ok(s.mysql(f"ALTER USER 'root'@'localhost' IDENTIFIED BY '{root_pw}';",
                                             user="devid", pw=admin, use_socket=s.sockets()))
        r["main_account_needs_new_password"] = refused(s.mysql("SELECT 1"))
        r["main_account_new_password"] = ok(s.mysql("SELECT 1", pw=root_pw))
        r["main_account_back_to_none"] = ok(s.mysql("ALTER USER 'root'@'localhost' IDENTIFIED BY '';",
                                                    user="devid", pw=admin, use_socket=s.sockets()))
        r["open_files_limit"] = s.mysql("SELECT @@open_files_limit, @@max_connections")[1]
        laravel_db = ("shop", "shop", pw2)
    elif s.engine == "postgresql":
        r["main_tcp_no_password"] = ok(s.psql("SELECT current_user"), "postgres")
        if s.sockets():
            r["main_socket"] = ok(s.psql("SELECT 1", host=s.sock_dir))
        r["create_account"] = ok(s.psql(f"CREATE ROLE shop LOGIN PASSWORD '{pw1}';"))
        r["create_database"] = ok(s.psql("CREATE DATABASE shop OWNER shop;"))
        r["account_needs_password"] = refused(s.psql("SELECT 1", user="shop", db="shop"))
        dsn = f"pgsql:host=127.0.0.1;port={s.port};dbname=shop"
        r["php_first_login"] = ok(php(work, "pdo", dsn, "shop", pw1), "ok 1")
        r["php_wrong_password_refused"] = refused(php(work, "pdo", dsn, "shop", "wrong"))
        r["reset"] = ok(s.psql(f"ALTER ROLE shop PASSWORD '{pw2}';"))
        r["php_new_password"] = ok(php(work, "pdo", dsn, "shop", pw2), "ok 1")
        r["php_old_password_refused"] = refused(php(work, "pdo", dsn, "shop", pw1))
        r["client_with_password"] = ok(s.psql("CREATE TABLE t (id int PRIMARY KEY); INSERT INTO t VALUES (1);",
                                              user="shop", pw=pw2, db="shop"))
        laravel_db = ("shop", "shop", pw2)
    else:
        admin, root_pw = password(), password()
        # The localhost exception: with access control on and no account yet, the first account
        # can be created without credentials, from 127.0.0.1 only.
        r["devid_account"] = ok(s.mongosh(
            f"db.getSiblingDB('admin').createUser({{user: 'devid', pwd: '{admin}', roles: ['root']}}); print('ok')"),
            "ok")
        r["unauthenticated_refused"] = refused(s.mongosh("db.getSiblingDB('shop').t.find().toArray()"))
        r["main_account"] = ok(s.mongosh(
            f"db.getSiblingDB('admin').createUser({{user: 'root', pwd: '{root_pw}', roles: ['root']}}); print('ok')",
            user="devid", pw=admin), "ok")
        r["create_account"] = ok(s.mongosh(
            f"db.getSiblingDB('shop').createUser({{user: 'shop', pwd: '{pw1}', "
            "roles: [{role: 'readWrite', db: 'shop'}]}); print('ok')", user="devid", pw=admin), "ok")
        uri = f"mongodb://shop:{{}}@127.0.0.1:{s.port}/shop?authSource=shop"
        r["php_first_login"] = ok(php(work, "mongo", uri.format(pw1), "shop", mongo=True), "ok 1")
        r["php_wrong_password_refused"] = refused(php(work, "mongo", uri.format("wrong"), "shop", mongo=True))
        r["reset"] = ok(s.mongosh(f"db.getSiblingDB('shop').changeUserPassword('shop', '{pw2}'); print('ok')",
                                  user="devid", pw=admin), "ok")
        r["php_new_password"] = ok(php(work, "mongo", uri.format(pw2), "shop", mongo=True), "ok 1")
        r["php_old_password_refused"] = refused(php(work, "mongo", uri.format(pw1), "shop", mongo=True))
        r["main_account_login"] = ok(s.mongosh("print(db.runCommand({ping: 1}).ok)", user="root", pw=root_pw), "1")
        try:
            with open(s.log, errors="replace") as f:
                r["rlimit_warning"] = "rlimits" in f.read()
        except OSError:
            pass
        laravel_db = None
        s.mongo_admin = admin
    return r, laravel_db


def data_survives(s, check):
    """SIGKILL the server, start it again, and check the row written before is still there."""
    s.kill9()
    ready, seconds = s.start()
    res = {"ready": ready, "restart_s": seconds}
    if ready:
        if s.engine == "mysql":
            res["row"] = ok(s.mysql("SELECT COUNT(*) FROM shop.t"), "1")
        elif s.engine == "postgresql":
            res["row"] = ok(s.psql("SELECT count(*) FROM t", user="shop", pw=check, db="shop"), "1")
        else:
            res["row"] = ok(s.mongosh("print(db.getSiblingDB('shop').t.countDocuments())",
                                      user="devid", pw=s.mongo_admin))
    return res


def laravel(work, engine, port, db, user, pw):
    app = os.path.join(work, "laravel")
    exe = os.path.join(work, "php", "php")
    if not os.path.isdir(app):
        phar = os.path.join(work, "composer.phar")
        base = "https://getcomposer.org/download/latest-stable/composer.phar"
        if sh(["curl", "-fsSL", "--retry", "3", "-o", phar, base])[0] != 0:
            return {"ok": False, "out": "composer download failed"}
        want = sh(["curl", "-fsSL", base + ".sha256sum"])[1].split()[0]
        got = sh(["shasum", "-a", "256", phar] if not LINUX else ["sha256sum", phar])[1].split()[0]
        if want != got:
            return {"ok": False, "out": f"composer sha256 {got} != {want}"}
        code, out = sh([exe, "-d", "memory_limit=-1", phar, "create-project", "laravel/laravel", app,
                        "--no-interaction", "--prefer-dist", "--quiet"], timeout=900)
        if code != 0:
            return {"ok": False, "out": out[-400:]}
    env = dict(os.environ, DB_CONNECTION="mysql" if engine == "mysql" else "pgsql", DB_HOST="127.0.0.1",
               DB_PORT=str(port), DB_DATABASE=db, DB_USERNAME=user, DB_PASSWORD=pw)
    return ok(sh([exe, "artisan", "migrate", "--force", "--no-interaction"], env=env, cwd=app, timeout=300),
              "")


def profiles(engine, series):
    if series != DEFAULT_SERIES[engine]:
        return ["devid"]
    return ["default", "devid", "small"] if engine == "mysql" else ["default", "devid"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("work")
    ap.add_argument("--only", default="")
    ap.add_argument("--ports", choices=["base", "usual"], default="base")
    ap.add_argument("--port-base", type=int, default=43000)
    ap.add_argument("--sock-dir", default="")
    ap.add_argument("--idle", type=int, default=15)
    ap.add_argument("--laravel", action="store_true")
    args = ap.parse_args()
    work = os.path.abspath(args.work)
    sock_dir = os.path.abspath(args.sock_dir) if args.sock_dir else os.path.join(work, "s")
    os.makedirs(sock_dir, exist_ok=True)
    with open(os.path.join(work, "fetched.json")) as f:
        fetched = json.load(f)
    only = {s for s in args.only.split(",") if s}
    results_path = os.path.join(work, "results.jsonl")
    rows = []

    def record(res):
        rows.append(res)
        with open(results_path, "a") as f:
            f.write(json.dumps(res) + "\n")

    hard = sh(["/bin/sh", "-c", "ulimit -H -n"])[1]
    for srv in fetched["servers"]:
        engine, series, version = srv["engine"], srv["series"], srv["version"]
        if only and f"{engine}-{series}" not in only:
            continue
        for profile in profiles(engine, series):
            start_port = USUAL[engine] if args.ports == "usual" else args.port_base + USUAL[engine] % 1000
            port = pick_port(start_port)
            s = Server(work, engine, series, version, port, profile, sock_dir)
            res = {"platform": fetched["platform"], "engine": engine, "series": series, "version": version,
                   "profile": profile, "port": port, "soft_nofile": SOFT_NOFILE, "hard_nofile": hard,
                   "socket": s.socket_path() if s.sockets() else "off (path too long)",
                   "command": " ".join(s.command()[1:])}
            print(f"== {engine} {version} ({profile}) on {port}", flush=True)
            code, out, res["init_s"] = s.init()
            if code != 0:
                tail = open(s.log, errors="replace").read()[-600:] if os.path.exists(s.log) else ""
                res["error"] = f"init failed: {out} {tail}"
                record(res)
                continue
            ready, res["start_s"] = s.start()
            if not ready:
                res["error"] = "did not start: " + open(s.log, errors="replace").read()[-600:]
                if s.proc.poll() is None:
                    s.kill9()
                record(res)
                continue
            res["identity"] = s.identity()
            time.sleep(args.idle)
            res["idle"] = memory(s.proc.pid)
            if profile == "devid":
                res["accounts"], laravel_db = accounts(s, work)
                if args.laravel and laravel_db and series == DEFAULT_SERIES[engine]:
                    res["laravel_migrate"] = laravel(work, engine, port, *laravel_db)
                res["after_kill"] = data_survives(s, laravel_db[2] if laravel_db else None)
                if not res["after_kill"]["ready"]:
                    res["error"] = "did not start again after SIGKILL: " + open(s.log, errors="replace").read()[-600:]
                    record(res)
                    continue
                res["used"] = memory(s.proc.pid)
            res["stop_code"], res["stop_s"] = s.stop()
            record(res)
            shutil.rmtree(s.base, ignore_errors=True)
    print_summary(rows)


def checks(res):
    acc = res.get("accounts") or {}
    tests = {k: v for k, v in acc.items() if isinstance(v, dict)}
    failed = [k for k, v in tests.items() if not v["ok"]]
    if not tests:
        return ""
    return "all %d ok" % len(tests) if not failed else "FAILED: " + ", ".join(failed)


def print_summary(rows):
    print("\n| Server | Profile | Init s | Ready s | Idle MB | Stop s | Restart after SIGKILL s | Accounts | Laravel |")
    print("|---|---|---|---|---|---|---|---|---|")
    for r in rows:
        idle = r.get("idle", {})
        mb = idle.get("pss_mb", idle.get("rss_mb", ""))
        if "pss_mb" in idle:
            mb = f"{idle['pss_mb']} PSS ({idle['rss_mb']} RSS)"
        after = r.get("after_kill", {})
        lv = r.get("laravel_migrate")
        print(f"| {r['engine']} {r['version']} | {r['profile']} | {r.get('init_s', '')} | {r.get('start_s', '')} "
              f"| {mb} | {r.get('stop_s', '')} | {after.get('restart_s', '')} | {checks(r)} "
              f"| {'' if lv is None else ('ok' if lv['ok'] else 'FAILED')} |")
        if r.get("error"):
            print(f"|  | error: {r['error'][:200]} | | | | | | | |")


if __name__ == "__main__":
    main()
