# Phase 14 Plan: Databases

> Scope: Devid installs and runs D-058's MySQL, PostgreSQL and MongoDB builds through the daemon, the CLI, the TUI and the app, on macOS and Linux (D-060)
> Verified against: 2026-11-25 (Devid 0.20.0, main d91d3b8; `db-latest` = `db-2026.11.24-8`; DBngin on the owner's M1; spike runs 45522275157 and 45523404458)
> Status: accepted 2026-11-25 (D-061, "Ikuti rekomendasi"), with the owner's answers in "Owner's answers". **D1 and D2 done 2026-11-25** (see each one's results). **D3 done 2026-11-25** (the CLI, the TUI and accounts, checked on the M1; see "D3 results"). **D4 built 2026-11-25** (the app's Databases page, from the board the owner approved; see "D4 results"); the owner's own try on the M1 is next.

D-060 put this phase right after Phase 13. D-058's builds are published in devid-binaries' `db-latest` release, but no Devid release reads them yet. macOS users have DBngin; Linux users have nothing comparable from Devid.

## What exists

### The builds (D-058)

| Engine | Series (newest build) | Archive size |
|---|---|---|
| MySQL | 8.4 LTS (8.4.13; Intel Macs 8.4.11), 9.7 LTS (9.7.4) | 80–91 MB |
| PostgreSQL | 15 (15.20), 16 (16.16), 17 (17.12), 18 (18.7), 19 (19.0) | 22–29 MB |
| MongoDB | 7.0 (7.0.48), 8.0 (8.0.38), 9.0 (9.0.8), and mongosh 2.14.3 | 42–101 MB; mongosh 82–93 MB |

Each archive exists for macOS on Apple silicon (`arm64`) and Intel (`x86_64`) and for `linux-x86_64`. The manifest (`schema: 1`) lists every series with its own `version`, and each platform entry carries its own `version`, `url`, `sha256` and `size`.

Limits D-058 found:
- **macOS:** MySQL, and MongoDB 8.0 and 9.0, need macOS 14 or later. MongoDB 7.0 needs macOS 11. Devid itself supports macOS 12.
- **Intel Macs:** Oracle stopped building MySQL for them after 8.4.11 and 9.7.4. They get no more security fixes there.
- **CPUs:** MongoDB 5.0 and later need AVX on x86_64.
- **Linux:** there is no arm64 build. `mongod` needs `LD_LIBRARY_PATH=<archive>/lib` for its bundled OpenSSL 3.

### In Devid (main at d91d3b8)

- **Mail is the template:**
  - Mailpit is found in the runtime manifest, downloaded with its sha256 checked, and run by the daemon's supervisor.
  - `devid mail on` changes config.toml only after the change has been applied.
  - Its port is checked against Devid's own ports and against anything listening (`mail::check_port`, `ports::in_use`, `ports::holder`).
  - It has its own status, doctor check, TUI page and app page.
- **The supervisor** (`supervisor.rs`):
  - A `Spec` holds the program, arguments, extra environment, a pid file, the process name for the orphan check, and the stop signal.
  - A service is restarted after a crash, and marked failed after 5 crashes in 60 s.
  - Stopping sends the signal, then SIGKILL after a fixed 10 s, while holding the lock that `status` also needs.
  - What a service prints goes to `daemon.log`, unless the service has a log flag of its own.
  - "Running" means the process exists. There is no readiness check.
  - The orphan check matches `ps -o comm=`: on macOS that is the full path, on Linux only the first 15 characters of the name.
- **Terminal commands (D-053):** a fixed list of names hard-linked to `devid` in `~/.devid/bin`, which comes first on `PATH`.
- **On-demand start (D-054)** wakes a dev server through the web server's error page, so it works only for HTTP. A database client connects straight to the port. Waking on connect would need the daemon to relay TCP, which D-054 chose to avoid.
- **One manifest only:**
  - `releases::artifact` reads one entry per tool (series only for PHP), and that entry's `version`.
  - The download is saved as `downloads/manifest.json`.
  - Update replies carry one release tag.
- **Uninstall:** `devid uninstall --purge` deletes all of `~/.devid`.

### On the owner's M1

DBngin runs two servers, both started, neither at login:
- "Local PG", PostgreSQL 18.4, on port 5432;
- "Local MySQL", MySQL 9.7.2, on port 3306 (socket `/tmp/mysql_3306.sock`).

Devid must never stop them, touch their data or take their ports, as it treats the owner's dev servers.

## What a database adds

- **Data that cannot be rebuilt.** Install, update, uninstall and purge must never delete it by accident.
- **A one-time setup of the data folder:** `mysqld --initialize-insecure`, `initdb`, or an empty folder for `mongod`.
- **A slow clean stop.** A database flushes before it exits, and SIGKILL after 10 s means crash recovery at the next start.
- **Ports people type.** `.env` files and tools such as TablePlus name the port, so it must stay the same.
- **Memory sized for servers.** MongoDB's WiredTiger cache defaults to half of (RAM − 1 GB). MySQL's buffer pool and performance schema take their share too. D-052 makes RAM the tightest resource on the M1.
- **Versions tied to data.** A minor update opens the same data folder. A new series (PostgreSQL 18 to 19, MySQL 8.4 to 9.7) needs a dump and restore, or the engine's own upgrade tool.

## Proposal (D-061)

- **Engines:** MySQL, PostgreSQL and MongoDB from `db-latest`.
  - The second manifest is saved as `downloads/db-manifest.json`. Devid fetches it only when a database is installed or checked for updates.
  - Devid uses each platform entry's own `version`, for MySQL on Intel Macs.
- **One server per engine:**
  - The user chooses a series per engine, for example MySQL 8.4, PostgreSQL 18 and MongoDB 8.0.
  - Switching series stops the old server and keeps its data folder. Both folders stay until the user deletes one (question 2).
- **Folders:**
  - binaries in `~/.devid/db/<engine>-<series>/`;
  - data in `~/.devid/data/<engine>-<series>/`;
  - sockets in `~/.devid/run/`;
  - logs in `~/.devid/log/<engine>.log`.
- **Running:** the daemon's supervisor runs the servers, as it does Mailpit. It gains what a database needs:
  - a stop timeout per service, about a minute for a database, and stops that no longer hold the lock `status` needs;
  - a log file per service;
  - a readiness check (the port answers) before `status` reports a server as running;
  - an orphan check by executable path (`/proc/<pid>/exe` on Linux), so Devid can never signal DBngin's or the system's server.
- **Starting:** a server is off until the user starts it (`devid db start mysql`). From then on it starts with the daemon, like PHP, and `autostart = false` pauses it like every other service. There is no on-demand start in this phase (question 3).
- **Ports:** servers listen on 127.0.0.1 only.
  - Each uses the engine's usual port (3306, 5432, 27017) when it is free, else the next free one.
  - The port is chosen at the first start and saved in config.toml, so `.env` files keep working.
  - On the owner's M1 this gives MySQL 3307 and PostgreSQL 5433, next to DBngin.
  - The same checks as Mail's port apply (question 4).
- **Accounts and passwords** (the owner's wish, see "Owner's answers"): creating several accounts and resetting a password each take one step.
  - `devid db user add mysql shop` creates the account `shop`. By default it also creates a database `shop`, gives the account every right on that database, and generates a password. `--database`, `--password` and `--no-database` change those defaults.
  - `devid db user password mysql shop` resets the password, to a new generated one or to the one given. It works for every account, the main one included.
  - `devid db user list` lists the accounts, and `devid db user remove` removes one.
  - `devid db env mysql shop` prints the account's `.env` lines: `DB_CONNECTION`, `DB_HOST=127.0.0.1`, `DB_PORT`, `DB_DATABASE`, `DB_USERNAME` and `DB_PASSWORD`.
  - The TUI and the app's Databases page offer the same actions, with buttons to copy the password and the `.env` lines.
  - **Generated passwords** are random, 24 letters and digits, with nothing that needs quoting in `.env`.
  - **Where passwords are kept:** Devid keeps the passwords it set in `~/.devid/data/<engine>-<series>/accounts.toml`, a file only the user can read, so `devid db env` can show one again (question 10).
  - **The passwords are enforced:**
    - MySQL and PostgreSQL check them over TCP (PostgreSQL with `scram-sha-256`);
    - MongoDB runs with access control on.
  - **Devid's own way in:** Devid can always manage accounts and reset a lost password, the main account's included, without the user's password. On each engine:
    - MySQL: an account of Devid's own, whose random password is in a file only the user can read, used over the Unix socket;
    - PostgreSQL: the Unix socket in `~/.devid/run`, which only the user can open, with `trust`;
    - MongoDB: an administrator account of Devid's own, kept the same way as MySQL's.
- **The main account:** MySQL's `root` and PostgreSQL's `postgres` have no password from this computer, as in DBngin; the accounts the user adds have passwords (question 5).
  - **MongoDB** cannot keep an account without a password once access control is on. Its main account `root` therefore gets a generated password too, which `devid db env mongodb` prints.
- **Memory:** settings for a laptop, from D1's measurements, such as a capped WiredTiger cache for MongoDB.
- **Apps:**
  - `devid db env mysql` prints the main account's `.env` lines. `devid db create <name>` creates a database without a new account.
  - Devid's PHP already has `pdo_mysql`, `mysqli`, `pdo_pgsql` and `pgsql` built in, and the `mongodb` extension in the catalog.
- **Clients:** `mysql`, `mysqldump`, `psql`, `pg_dump`, `pg_restore` and `mongosh` become terminal commands (D-053).
  - Each defaults to Devid's server when no host or port is given.
  - Each is linked only once its engine is installed, since it comes before DBngin's or Homebrew's on `PATH` (question 8).
- **Updates:** `devid update` also moves each installed series to the newest build in `db-latest`: stop, swap the binaries, start, same data. Moving to a new series never happens on its own; the guide explains dump and restore.
- **Uninstall:**
  - `devid uninstall` stops the servers.
  - `--purge` keeps `~/.devid/data` and says where it is, unless `--delete-databases` is also given (question 6).
- **Limits shown, not hit:** Devid offers only what the computer can run.
  - On macOS 12 and 13: no MySQL, and no MongoDB 8.0 or 9.0.
  - On x86_64 CPUs without AVX: no MongoDB.
  - On Intel Macs: a note that MySQL gets no more fixes there.
  - On Linux, `mongod` gets `LD_LIBRARY_PATH` through `Spec.env`.
- **Interfaces:**
  - `devid db`: `list`, `install`, `start`, `stop`, `use`, `port`, `create`, `env`, `remove`, and `user` (`add`, `password`, `list`, `remove`);
  - a Databases page in the TUI;
  - checks in `devid doctor`;
  - on macOS, a Databases page in the app, built from a design board the owner approves first (D-026; question 7).
- **Next to DBngin and Homebrew:** Devid never starts, stops or reads their servers or data, and only avoids their ports. Moving data out of DBngin is a dump and restore, which the guide shows.

## D1: what the spike must answer

1. **Setup and connections:** for each engine and series, run the setup command, then connect over 127.0.0.1 and over the socket:
   - from the bundled client;
   - from Devid's PHP (`pdo_mysql`, `pdo_pgsql`, the `mongodb` extension);
   - with Laravel's `migrate`.

   In particular, does MySQL's `root@localhost` from `--initialize-insecure` accept connections over TCP from 127.0.0.1?
2. **Memory:** idle memory of each engine with its defaults and with Devid's settings, on the M1 and on Linux. Which settings bring it down without hurting a typical app.
3. **Start and stop:** time to ready, and time to a clean stop, both empty and with some data, plus the behavior after SIGKILL.
4. **Limits:**
   - the 256 open files launchd gives the daemon, which its children inherit, against what `mysqld` and `mongod` need;
   - socket paths, which have a 104-byte limit;
   - the orphan check on Linux, by `/proc/<pid>/exe` instead of the process name.
5. **Ports next to DBngin:** on the M1, Devid's servers take 3307 and 5433 while DBngin keeps 3306 and 5432, without anything of DBngin's being touched.
6. **Minor updates:** a data folder made by one build of a series, opened by the next (MySQL 8.4.x, a PostgreSQL minor, a MongoDB patch).
7. **Accounts:** for each engine:
   - Create an account with a password, then reset it, both through Devid's own way in.
   - The new password works, and the old one and a wrong one are refused.
   - The main account still works without a password (MySQL, PostgreSQL), and `pg_hba.conf` lets only `postgres` in without one.
   - A new account's first login from Devid's PHP over TCP works without TLS, which MySQL's `caching_sha2_password` needs RSA or a secure connection for.
   - On MongoDB, Devid's administrator account is created before access control is turned on.

**Where:** GitHub's macOS runners (Apple silicon and Intel), Ubuntu, and Arch in its VM; and the M1, on spare ports only.

## D1 results (2026-11-25)

**Where it ran:**
- Spike run 45522275157 on GitHub's runners: macOS 15 on Apple silicon and on Intel, Ubuntu 24.04, and Arch in its VM.
  - Every series in `db-2026.11.24-8`, with Devid's PHP 8.5.13 from `runtime-latest`.
  - Each server ran on 127.0.0.1, with the soft open-files limit Devid's daemon passes on: 256 under launchd, 1024 under systemd.
- The owner's M1: MySQL 8.4 and PostgreSQL 18, next to DBngin's servers.
- The scripts are in `spike/db/`.

### Answers

1. **Setup and connections:** every series set up, started and stopped on every platform.
   - **Setup times:** MySQL's `--initialize-insecure` took 1.3 to 5.3 s, `initdb` 0.3 to 2.1 s, and MongoDB needs no setup.
   - **MySQL's `root@localhost`** accepts TCP from 127.0.0.1 without a password, so `DB_HOST=127.0.0.1` works.
   - **Devid's PHP** connected with `pdo_mysql`, `pdo_pgsql` and the `mongodb` extension. For MySQL that included a new account's first `caching_sha2_password` login over TCP without TLS.
   - **Laravel:** `php artisan migrate` on a new Laravel app passed with a Devid account, on MySQL 8.4 and on PostgreSQL 18, on all four platforms.
   - **`initdb` refuses a password method for TCP** unless the superuser has a password. So Devid runs `initdb -A trust` and writes `pg_hba.conf` itself:
     - the socket and `postgres` from 127.0.0.1 need no password;
     - every other account needs one (`scram-sha-256`).
2. **Memory at idle.** Linux figures are PSS. macOS figures are the footprint from the second run (45523404458), which also counts what the memory compressor holds:

   | Engine | Its defaults | Devid's settings | Devid's settings are |
   |---|---|---|---|
   | MySQL 8.4 and 9.7 | 449 MB (Linux), 410–468 MB (macOS) | 210–225 MB (Linux), 175–228 MB (macOS) | `--performance-schema=OFF`, `--skip-log-bin` |
   | PostgreSQL 15 to 19 | 25 MB (18, Linux), 26–36 MB (macOS) | 14–20 MB (Linux), 11–26 MB (macOS) | `shared_buffers` 32 MB and 50 connections, which saved 8 to 12 MB |
   | MongoDB 7.0 to 9.0 | 181–207 MB (8.0, Linux), 26–35 MB (macOS) | 140–190 MB (Linux), 15–31 MB (macOS) | `--wiredTigerCacheSizeGB 0.25`, diagnostic data collection off |

   - **RSS counts more than a server needs.** MongoDB's RSS on macOS was 70 to 113 MB against a 15 to 35 MB footprint: most of it is the clean pages of its 100 MB program, which macOS can drop at any time. Linux's PSS counts those pages too, which is why MongoDB looks larger there.

   - **MySQL's performance schema** is the big cost. Turning it off halves idle memory. A 64 MB buffer pool and 50 connections saved only 1 to 13 MB more, so Devid keeps MySQL's 128 MB pool.
   - **PostgreSQL's settings** matter little at idle. With PostgreSQL's own defaults, `shared_buffers` (128 MB) would fill only as a database is used.
   - **MongoDB's cache cap** barely changes idle memory. It keeps the cache from growing towards half of the computer's memory, which is MongoDB's own default.
   - **On the M1**, under memory pressure, `ps` showed far less (MySQL 27 to 75 MB), because macOS compresses idle pages. That is why the second run added the footprint.
3. **Start and stop:**
   - **Ready:** MySQL in 0.3 to 1.8 s, PostgreSQL 0.1 to 0.3 s, MongoDB 0.1 to 2.4 s.
   - **Clean stop when idle:**
     - MySQL 0.2 to 1.5 s, on SIGTERM;
     - PostgreSQL under 0.05 s, on SIGINT (its fast shutdown);
     - MongoDB 0.3 s at most, on SIGTERM.

     All of these are well inside the supervisor's 10 s. D2 still gives databases a longer timeout, for a server with writes in flight.
   - **After SIGKILL:** every server started again with its data. MySQL took 0.6 to 3.4 s, PostgreSQL 0.1 to 1.9 s, and MongoDB 0.3 to 2.1 s.
4. **Limits:**
   - **Open files:** with a soft limit of 256 or 1024, MySQL raised its own limit to 8161, which the hard limit allows. PostgreSQL adapts. MongoDB logs a "soft rlimits" warning but runs.
   - **Socket paths:** MySQL refuses to start when its socket path is longer than 103 bytes (`[MY-010267] The socket file path is too long`, then "Aborting").
     - `~/.devid/run/mysql.sock` fits any ordinary home folder.
     - On the M1 the spike's long work folder hit the limit. Devid must check the length and say so.
   - **Orphan check:** on Linux, `ps -o comm=` shows only `mysqld`, `postgres` or `mongod`, which would match DBngin's or the system's server as well. `/proc/<pid>/exe` gives the full path. On macOS, `comm` is the full path already.
5. **Next to DBngin on the M1:**
   - With the usual port taken, MySQL took 3307 and PostgreSQL 5433. They ran every check and stopped.
   - DBngin's servers kept 3306 and 5432 throughout, with the same process IDs before and after.
   - **DBngin's `mysqld` also listens on 33060 (MySQL's X Protocol) on every interface,** so Devid's MySQL must run with `--mysqlx=OFF`. Otherwise it cannot start next to DBngin.
6. **Minor updates:** not tested yet, since `db-latest` holds one build per series. D2 tests an update once the next build appears.
7. **Accounts:** every check passed on every platform and series.
   - **MySQL:**
     - `root` over TCP and over the socket without a password, and Devid's own account;
     - an account with its database, and PHP's first and second login;
     - a wrong password, and the old one after a reset, refused;
     - the main account given a password, then without one again.
   - **PostgreSQL:**
     - `postgres` over TCP and over the socket without a password;
     - other accounts refused without a password;
     - a reset, and PHP's login.
   - **MongoDB:**
     - with access control on from the first start, the localhost exception let Devid create its administrator;
     - reads without credentials refused;
     - `root`, and an account with `readWrite` on its database;
     - PHP's login, and a reset.

### What this changes in the plan

D-061 stands. D1 settles these details:
- **MySQL** runs with `--mysqlx=OFF`, `--skip-log-bin` and `--performance-schema=OFF`.
- **PostgreSQL** gets `initdb -A trust` and Devid's own `pg_hba.conf`, with `shared_buffers` at 32 MB and 50 connections.
- **MongoDB** runs with access control, a 0.25 GB cache, and diagnostic data collection off.
- **Devid checks socket path lengths,** and its orphan check reads `/proc/<pid>/exe` on Linux.
- **Idle memory per engine:** MySQL about 175 to 225 MB, PostgreSQL 11 to 26 MB, MongoDB 15 to 35 MB on macOS (140 to 190 MB PSS on Linux). That is why each engine stays off until it is started.
- **Minor updates** are tested in D2.

## D2 results (2026-11-25)

The record is `docs/implementation-timeline/databases/2026-11-25-d2-daemon.md`.

### What shipped

- **The supervisor,** for every service:
  - **Stop timeout:** each service has its own (`Spec.stop_timeout`): 10 s as before, 60 s for a database.
  - **Stopping:** services stop outside the lock `status` needs, and several at once on the daemon's exit, so a database's flush never holds up a request or php-fpm.
  - **Log:** a log file per service (`Spec.log`).
  - **Readiness:** `Spec.ready_port` and `Spec.ready_file`. A database counts as running only once its port answers and its own socket exists, so another program on the port never passes for it. The socket file is removed before each start, so one left by a crash does not count either.
  - **The orphan check:**
    - it reads only the pid file's first line, which suits PostgreSQL's `postmaster.pid`;
    - on Linux it compares `/proc/<pid>/exe` with the program. That also fixes Mailpit's check there, which `comm` (only the name) never matched.
    - An orphan that outlasts the stop timeout gets SIGKILL.
- **The database release** (`releases.rs`):
  - `fetch_db_manifest` reads `db-latest`, or `DEVID_DB_MANIFEST_URL` for tests, into `downloads/db-manifest.json`, beside the runtime's manifest;
  - `db_artifact` takes each platform's own version;
  - `db_series` sorts by number;
  - `download_db` accepts only the release's own URLs.
- **`db.rs`:**
  - **Engines and folders:** binaries in `db/<engine>-<series>`, data in `data/<engine>-<series>`, logs in `log/<engine>.log`, sockets in `run/`.
  - **Install:** a new build of a series replaces the binaries, never the data. MongoDB counts as installed only with mongosh.
  - **Setup and settings:** D1's setup commands and server settings.
  - **Ports and paths:** the port choice, and the socket length check.
  - **Passwords:** `accounts.toml` (mode 0600), and generated passwords.
  - **`bootstrap`:** Devid's own account on MySQL. On MongoDB, the administrator `devid` and the main account `root`, through the localhost exception.
- **`config.toml`:** `[databases.<engine>]` holds `series`, `port` and `enabled`.
- **The daemon:**
  - **Requests:** `DbList`, `DbInstall`, `DbStart` and `DbStop`, and `status.databases`.
  - **Services:** databases count as services, so `devid stop|start mysql` pause and resume them.
  - **Ports:** Mail's port check knows the databases' ports.
  - **Updates:** `devid update` moves each installed series, and mongosh, to the newest build in the database release, and restarts a running server on it. It reads that release only once a database is installed.
- **The CLI:** `devid db` with `list [--available]`, `install`, `start` and `stop`.
- **Tests:**
  - unit tests for the supervisor, `db.rs`, the database manifest and the config;
  - `crates/cli/tests/db.rs`, with fake servers (shell and perl, listening and making their sockets as the real ones do) from a local test release. It covers install, setup, a start with D1's flags, Devid's own accounts, a pause and resume, a stop, an update within a series, and a daemon restart;
  - `scripts/db-e2e.sh`, in CI's new `db-e2e` job (Ubuntu and macOS), with the real builds:
    - each engine starts;
    - the main accounts and Devid's own account log in, and another account is refused without its password;
    - the servers come back with the daemon;
    - `devid db stop` stops them, and the data stays.

### Left for D3

D3 did all of these; see "D3 results".

- **Accounts:** `devid db user`, with `env`, `use`, `port`, `create` and `remove`.
- **The rest of D3:** the TUI page, the terminal commands, and doctor.
- **Uninstall:** today `devid uninstall --purge` would still delete `~/.devid/data`. D3 changes that before any release.

## D3 results (2026-11-25)

The record is `docs/implementation-timeline/databases/2026-11-25-d3-cli-tui-accounts.md`.

### What shipped

- **Accounts** (`db::Admin`, Devid's own way into a running server):
  - **`devid db user add <engine> <name>`** makes an account with a generated password. By default it also gets a database of its own, named after it, with every right on it. `--database`, `--password` and `--no-database` change those defaults.
    - MySQL: `'<name>'@'%'`, with `GRANT ALL` on its database.
    - PostgreSQL: a login role that owns its database. A database that exists already passes to it. If the database step fails, the role goes again.
    - MongoDB: an account in its own database, with `readWrite` and `dbAdmin` there.
  - **`devid db user password <engine> <name>`** gives an account a new password, generated or given. On MySQL it changes every host the account has, `root@localhost` included.
  - **`devid db user list <engine>`** lists the accounts and says for each password whether Devid keeps it, whether none is needed (the main account from this computer), or whether Devid does not know it.
  - **`devid db user remove <engine> <name>`** removes the account; its databases stay. On PostgreSQL its objects pass to `postgres` first, database by database.
  - **Checks:**
    - an account that already exists, or does not, is refused before any SQL runs;
    - `root`, `postgres`, `devid` and `admin` are not offered as new accounts;
    - names take lowercase letters, digits and `_`, at most 32;
    - passwords the user picks take 8 to 64 letters, digits and `-_.~!@#%^*+=`.

    Nothing Devid writes into SQL, JavaScript or `.env` needs quoting.
  - **`accounts.toml`** keeps each account's password and database. D2's file, which held the password alone, still reads.
- **`.env` lines:** `devid db env <engine> [<account>]` prints only the lines, so `>> .env` works. They are:
  - MySQL and PostgreSQL: `DB_CONNECTION`, `DB_HOST=127.0.0.1`, `DB_PORT`, `DB_DATABASE`, `DB_USERNAME` and `DB_PASSWORD`;
  - MongoDB: `DB_CONNECTION=mongodb`, `DB_URI` with the account, its database and `authSource`, and `DB_DATABASE`. A password the user picked is percent-escaped in the URI, since `@` or `%` would break it.

  The main account has no database of its own, so its lines leave `DB_DATABASE` out. Laravel then keeps its default (`laravel`), which `artisan migrate` offers to create; an empty value would mean no database.
- **More of `devid db`:**
  - `use <engine> <series>` runs another series, each with its own data;
  - `port <engine> <port>` moves a server; it restarts there. Mail's checks apply, and the port must be 1024 or higher;
  - `create <engine> <name>` makes a database without an account (MongoDB makes one when an app first writes to it);
  - `remove <engine> <series> [--delete-data]` removes a series that is not in use. Its data stays unless asked.
- **Terminal commands** (D-053): `mysql`, `mysqldump`, `psql`, `pg_dump`, `pg_restore` and `mongosh`.
  - **Linking:** they join `~/.devid/bin` once their engine is installed and leave when no series of it is left. `devid setup` and `self-update` link them again.
  - **Where they connect:** each runs the client of the series in use and reaches Devid's server as its main account, unless the arguments say where to go:
    - `mysql` and `mysqldump`: the socket, or 127.0.0.1 and the port. A port given alone goes over TCP, which plain `mysql` would ignore. A defaults file or login path of the user's own adds nothing.
    - `psql`, `pg_dump` and `pg_restore`: `PGHOST`, `PGPORT` and `PGUSER=postgres`, unless set already or given as flags or a connection string.
    - `mongosh`: `--port` only. Root's password never goes on a command line, which other users can read with `ps`; `mongosh -u root -p` asks for it.
  - **Before the first start** there is no port yet, and the clients run as they are.
- **The TUI:** a Databases page, number 5, so Logs and Settings move to 6 and 7.
  - **The page** lists each engine's series, state and address.
  - **Enter** opens the selected engine's actions: start or stop, restart, Accounts…, copy the main account's `.env` lines, a new database, and the port.
  - **The Accounts view:**
    - `a` adds an account, as `name` or `name database`;
    - `p` gives one a new password, after a confirmation;
    - `c` copies its `.env` lines and `y` its password;
    - `d` removes it, after a confirmation.

    A new password shows once, with the `.env` lines and keys to copy either.
- **`devid doctor`** checks every database that is on. Its fix is the server's error, or how to start it.
- **`devid uninstall --purge`** deletes all of `~/.devid` but `~/.devid/data`, then names the databases it kept. `--delete-databases` deletes them too. A plain `uninstall` keeps `~/.devid` as before.
- **Stopping:** launchd's `ExitTimeOut` and systemd's `TimeoutStopSec` are now 75 s (`platform::EXIT_TIMEOUT`), and `stop_service` waits as long. A database may take up to 60 s to stop cleanly; launchd's default 20 s would have killed it halfway.
- **Tests:**
  - **Unit tests:** accounts, `.env` lines, D2's `accounts.toml`, the shims' arguments, the TUI's page, Accounts view and password popup, and the purge that keeps the data.
  - **`crates/cli/tests/db.rs`:** a second test, with fake clients that remember the accounts made. It covers adding, listing, new passwords (`root`'s included), removing, the `.env` lines, `create`, `port`, `remove`, and every refusal.
  - **`crates/cli/tests/shim.rs`:** the uninstall dry run keeps the databases unless `--delete-databases`.
  - **`scripts/db-e2e.sh`** (CI's `db-e2e`, the real builds) covers each engine's accounts:
    - an app's first migration over TCP with the generated password;
    - a wrong password, and the old one after a reset, refused;
    - another account kept out of the first one's database;
    - the data still there after the account is removed;
    - a PostgreSQL server moved to another port.

### Differences from the plan

- **PostgreSQL's `postgres`** gets no password from `devid db user password`. The plan said every account, the main one included. But `postgres` is trusted from this computer, so a password would never be checked; Devid says so instead.
- **`devid db create`** takes the engine first (`devid db create mysql blog`), like the other commands.

### The M1 check (2026-11-25)

D3's build ran in a test daemon with a home of its own and spare ports, one engine at a time. The home's path was short enough for the sockets. Devid never touched DBngin, whose servers kept their pids throughout.

- **Next to DBngin:**
  - MySQL 8.4.13 ran on 3307, beside DBngin's MySQL 9.7.2 on 3306. Its first start, download included, took 11 s.
  - PostgreSQL 18.7 ran on 5433, beside DBngin's PostgreSQL 18.4 on 5432.
- **From the CLI,** on each engine:
  - Devid made the accounts `shop` and `blog`, each with a database of its own, and gave `shop` a new password.
  - `devid db user password postgresql postgres` was refused, as planned.
- **From the TUI:**
  - The real TUI ran in a pseudo-terminal, with the keys a user presses.
  - On each engine it made `tuiapp` and `tuiblog` from the Accounts view, and gave `tuiapp` a new password.
  - Each password the TUI showed matched the one in `accounts.toml`.
- **Laravel:**
  - A new Laravel app, on Devid's PHP 8.5, migrated with each of the eight accounts, into the account's own database.
  - Each database got the 9 tables; on PostgreSQL the account owned them.
  - Each new MySQL account's first login used `caching_sha2_password` over TCP without TLS.
- **Terminal commands:** `mysql` reached `root@localhost` on 3307 through the socket, and `psql` reached `postgres` on 5433.
- **`devid doctor`:** "PostgreSQL 18 running on 127.0.0.1:5433".
- **Uninstall:**
  - The dry run of `--purge` deleted all but `data/` and kept `mysql-8.4` and `postgresql-18`; with `--delete-databases` it deleted them too.
  - A real uninstall would undo the owner's own setup, so it runs on the Linux VMs with the rc (D5).

## D4 results (2026-11-25)

The record is `docs/implementation-timeline/databases/2026-11-25-d4-app-page.md`.

### The board

- **Where:** the canvas "Devid Redesign", page "Fase 14: databases" (https://claude.ai/artifact/TS8URDyBeQ4D5PaozQsDUa#page-fase14).
- **What it holds:**
  - an interactive prototype, `Databases.dc.html`;
  - four boards that open it in one state each: a new account, the password and its `.env` lines, an account's actions, and MongoDB's first start.
- **Approval:** the owner approved it: "Setuju, lanjutkan D4".

### What shipped

- **The Databases page** in the app, after Mail in the sidebar. The sidebar counts the databases that run.
  - **Tabs** for MySQL, PostgreSQL and MongoDB, each with a dot and its port while it runs. The arrow keys, Home and End move between them.
  - **The server's card:**
    - the series, the state and the address;
    - a switch (`db start` / `db stop`), and Start while it is paused;
    - ⋯: Restart, Copy `.env` for the main account, and Show log (the engine's log on the Logs page).
  - **Accounts:**
    - the main account first, then the rest by name;
    - Copy `.env` on each one;
    - ⋯: Copy password, New password… and Remove…. What cannot be done is disabled, with its reason: `postgres` gets no password, and the main account stays.
    - Add account opens a dialog: a name, a database of its own (it follows the name until edited), and a password, Generate or Choose.
    - A new account or a new password shows the password and the `.env` lines, each with Copy.
  - **Settings:**
    - the port, applied on leaving the field (the daemon refuses a busy port and says who holds it);
    - the series offered by the release, or the installed ones offline. Picking one asks first, then downloads it if needed.
  - **Storage:** the data folder with Show in Finder, and the terminal commands.
  - **Explanations** are behind (i) (D-055).
  - **First start:** a database that never ran shows what Devid will do, Start, then the download's progress. Another series shows its progress in the server's card.
- **The window's quick menu and the menu-bar icon** list the databases that are on, with their port or "Stopped"; each opens the page.
- **The app's API:**
  - `copy_db` copies an account's `.env` lines or its password through `db env`, so they never pass through the page.
  - `db_list`, `db_user_list` and `db_env` count as reads.
  - Show in Finder accepts a database's data folder.
- **The daemon:** `status.databases[].data` is the series' data folder.
- **Accounts load only when they can change:** the page asks for them when the server on screen changes or after an action, not on every 2 s refresh, since each list runs the engine's client.

### Checked

- **In the app's browser:** the real `app.js` and `app.css`, with a fake `invoke` that answers like a daemon with MySQL running, PostgreSQL paused and MongoDB not installed. Every action on the board ran there:
  - adding an account, giving one a new password, and removing one;
  - a busy port, refused with its holder;
  - the paused PostgreSQL started, with `postgres`'s actions disabled and their reasons;
  - MongoDB's first start, with its progress;
  - MySQL moved to 9.7;
  - the quick menu's Databases;
  - the tabs' arrow keys;
  - the dark and light themes.
- **The repository's checks:**
  - `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass, with `-p devid-gui` too;
  - `node --check` passes on `app.js`.

### Not yet

- **The owner's try on the M1:** adding an account and resetting a password from the page in the real app, against a real daemon. Either a test copy of the app with a test daemon now, or 0.21.0's rc in D5.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| D1 | **Spike** | the answers are in this doc, and D-061 is accepted or changed to match them (done 2026-11-25, see "D1 results") |
| D2 | **Daemon:** the second manifest, install and update, the supervisor's additions (stop timeout, readiness, log file, orphan check by path), config, ports, API | unit tests cover install, ports and the supervisor changes, and a CLI test starts and stops each engine from a local test release (done 2026-11-25, see "D2 results") |
| D3 | **CLI, TUI, accounts, terminal commands, doctor, uninstall** | on the M1, MySQL and PostgreSQL run next to DBngin's on their own ports. Two accounts are created and one password is reset from the CLI and from the TUI. A Laravel site migrates with each account, and uninstall keeps the data (done 2026-11-25, see "D3 results"; a real uninstall runs on the VMs in D5) |
| D4 | **The app's Databases page** (macOS), accounts included | the owner approves a design board, then adds an account and resets a password from the page on the M1 (board approved and page built 2026-11-25, see "D4 results") |
| D5 | **CI, docs and release** | a database check runs on every pull request on macOS, Ubuntu and Arch; the README, the guide's new `/docs/databases` page, PROJECT_CONTEXT and CLAUDE.md are updated; 0.21.0 ships after an rc on the M1 and the Linux VMs |

## Where it is tested

- **GitHub's runners:** macOS (Apple silicon and Intel), Ubuntu (`linux-e2e`) and Arch in a VM (`arch-e2e`). D-058's `db-smoke.sh` already starts each archive there.
- **The owner's M1:** Devid's servers on their own ports next to DBngin's, one engine at a time to spare memory, never stopping DBngin.
- **The Ubuntu, Arch and CachyOS VMs** on the old Mac, for Linux, with Devid reinstalled from the rc.
- **Not used:** a VM on the M1 (D-013).

## Questions for the owner

1. **Engines:** all three in this phase (MySQL, PostgreSQL, MongoDB)? *Recommended: yes, with MongoDB's limits shown.*
2. **Servers per engine:** one at a time (switch series with `devid db use`), or several series of the same engine at once, as DBngin allows? *Recommended: one per engine.*
3. **Starting:** off until started by hand, then with the daemon? *Recommended: yes. On-demand start would need the daemon to relay TCP.*
4. **Ports:** the usual port when free, else the next free one, saved? Or fixed ports of Devid's own? *Recommended: the usual port or the next free one, saved.*
5. **The main account** (MySQL's `root`, PostgreSQL's `postgres`): no password from this computer, as in DBngin, while the accounts you add have passwords? Or a password for the main account too, which Devid generates and can reset? *Recommended: no password for the main account. MongoDB's main account gets a password either way.*
6. **`uninstall --purge`:** keep the databases unless `--delete-databases` is given? *Recommended: keep them.*
7. **The app:** a Databases page in this phase, after a design board? Or later? *Recommended: in this phase, as D4, after the CLI and the TUI.*
8. **Terminal commands** (`mysql`, `psql`, `mongosh`…): added only for installed engines, ahead of DBngin's or Homebrew's on `PATH`? *Recommended: yes.*
9. **Docs that say databases are out of scope:** CLAUDE.md, CONTRIBUTING, PROJECT_CONTEXT, the README and D-006. *Recommended: reword them in D5, once D-061 is accepted. CLAUDE.md only with your OK.*
10. **The passwords Devid sets:** kept in a file only you can read, so `devid db env` can show one again, or shown once and reset when lost? *Recommended: kept. They are development passwords, which `.env` files hold in plain text anyway.*

## Owner's answers (2026-11-25)

- **Order:** databases before the Linux desktop app ("Database dulu"). Claude recommended it: databases add something Devid lacks on every platform, Linux has no DBngin, and the CLI and the TUI already do everything the app does. Since the app's pages are shared, a Databases page built now carries over to a Linux app later.
- **Accounts:** "yang saya harapkan ada fitur mudah membuat beberapa akun database dan mudah reset password". Creating several accounts and resetting passwords are now part of the proposal, in "Accounts and passwords", with D1 item 7 and questions 5 and 10.
- **Questions 1 to 10:** "Ikuti rekomendasi". Every recommendation stands:
  - all three engines;
  - one server per engine;
  - off until started by hand, then with the daemon;
  - the usual port or the next free one, saved;
  - the main MySQL and PostgreSQL account without a password, MongoDB's with one;
  - `--purge` keeps the databases;
  - the app's page in D4;
  - terminal commands only for installed engines;
  - the out-of-scope docs reworded in D5;
  - the passwords Devid sets kept in a file only the user can read.

## Not in this phase

- **Redis or Valkey:** not among D-058's builds. Laravel's cache and queues often use Redis, so a later decision could add it.
- **On-demand databases,** which need a TCP relay in the daemon.
- **Upgrades across series** (`pg_upgrade`, MySQL 8.4 to 9.7), and importing data from DBngin. Both are dump and restore for now.
- **Backups and scheduled dumps.**
- **MariaDB, Elasticsearch and SQLite tools.**
- **Linux on arm64.**
