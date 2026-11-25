# Phase 14 Plan: Databases

> Scope: Devid installs and runs D-058's MySQL, PostgreSQL and MongoDB builds through the daemon, the CLI, the TUI and the app, on macOS and Linux (D-060)
> Verified against: 2026-11-25 (Devid 0.20.0, main d91d3b8; `db-latest` = `db-2026.11.24-8`; DBngin on the owner's M1)
> Status: proposed 2026-11-25. Waits for the owner's answers (see "Questions for the owner"). D-061 is written once they are in.

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
- **Users:** the same as DBngin, reachable only from this computer (question 5):
  - MySQL: `root` without a password;
  - PostgreSQL: `postgres`, with `trust` for local connections;
  - MongoDB: no access control.
- **Memory:** settings for a laptop, from D1's measurements, such as a capped WiredTiger cache for MongoDB.
- **Apps:**
  - `devid db env mysql` prints the `.env` lines: `DB_CONNECTION`, `DB_HOST=127.0.0.1`, `DB_PORT`, `DB_USERNAME` and an empty `DB_PASSWORD`.
  - `devid db create <name>` creates a database.
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
  - `devid db`: `list`, `install`, `start`, `stop`, `use`, `port`, `create`, `env` and `remove`;
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

**Where:** GitHub's macOS runners (Apple silicon and Intel), Ubuntu, and Arch in its VM; and the M1, on spare ports only.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| D1 | **Spike** | the answers are in this doc, and D-061 is accepted or changed to match them |
| D2 | **Daemon:** the second manifest, install and update, the supervisor's additions (stop timeout, readiness, log file, orphan check by path), config, ports, API | unit tests cover install, ports and the supervisor changes, and a CLI test starts and stops each engine from a local test release |
| D3 | **CLI, TUI, terminal commands, doctor, uninstall** | on the M1, MySQL and PostgreSQL run next to DBngin's on their own ports, a Laravel site migrates against each, and uninstall keeps the data |
| D4 | **The app's Databases page** (macOS) | the owner approves a design board, then uses the page on the M1 |
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
5. **Users:** no password and local only, as in DBngin? *Recommended: yes.*
6. **`uninstall --purge`:** keep the databases unless `--delete-databases` is given? *Recommended: keep them.*
7. **The app:** a Databases page in this phase, after a design board? Or later? *Recommended: in this phase, as D4, after the CLI and the TUI.*
8. **Terminal commands** (`mysql`, `psql`, `mongosh`…): added only for installed engines, ahead of DBngin's or Homebrew's on `PATH`? *Recommended: yes.*
9. **Docs that say databases are out of scope:** CLAUDE.md, CONTRIBUTING, PROJECT_CONTEXT, the README and D-006. *Recommended: reword them in D5, once D-061 is accepted. CLAUDE.md only with your OK.*

## Not in this phase

- **Redis or Valkey:** not among D-058's builds. Laravel's cache and queues often use Redis, so a later decision could add it.
- **On-demand databases,** which need a TCP relay in the daemon.
- **Upgrades across series** (`pg_upgrade`, MySQL 8.4 to 9.7), and importing data from DBngin. Both are dump and restore for now.
- **Backups and scheduled dumps.**
- **MariaDB, Elasticsearch and SQLite tools.**
- **Linux on arm64.**
