# Phase 14 D3: accounts, the CLI, the TUI, the terminal commands and uninstall

> Date: 2026-11-25 · Branch: `db-d3-v2` (replaces `db-d3`, PR #148) · Plan and results: `docs/roadmap/phase-14.md`, "D3 results" · Decision: D-061

## What shipped

- **`db.rs`:**
  - `Account` (password and database) in `Accounts.users`. D2's `accounts.toml`, which held the password alone, still reads.
  - `check_name`, `check_password` and `main_account`.
  - `Admin`, Devid's own way into a running server:
    - MySQL: Devid's account `devid` over the socket, through a `--defaults-file` only the user can read;
    - PostgreSQL: `postgres` over the socket in `~/.devid/run` (trusted);
    - MongoDB: the administrator `devid`, through a mongosh script only the user can read.

    Its methods are `users`, `add_user`, `set_password`, `remove_user` and `create_database`.
  - `env_lines`, `CLIENTS`, `clients`, `client_engine` and `client_bin`, `mysql_target`, `pg_host` and `data_root`.
- **`protocol.rs` and `server.rs`:** the requests `DbUserAdd`, `DbUserPassword`, `DbUserList`, `DbUserRemove`, `DbEnv`, `DbCreate`, `DbPort` and `DbRemove`.
- **`install.rs`:** `link_clients` and `linked_clients`. `install_binary` links the installed engines' clients again; `terminal_status` lists them.
- **`platform`:** `EXIT_TIMEOUT`, 75 s, in the LaunchAgent (`ExitTimeOut`) and the systemd unit (`TimeoutStopSec`). `stop_service` waits that long.
- **`crates/cli`:**
  - `main.rs`: `devid db use|port|create|env|remove` and `devid db user add|password|list|remove`;
  - `shim.rs`: the database clients;
  - `setup.rs`: the doctor checks, and `uninstall --purge [--delete-databases]`;
  - `tui/`: the Databases page, the Accounts view and the password popup.
- **Tests:**
  - unit tests in `db.rs`, `shim.rs`, `setup.rs` and `tui/`;
  - `crates/cli/tests/db.rs` (`accounts_env_port_and_remove`) and `crates/cli/tests/shim.rs`;
  - `scripts/db-e2e.sh`, with accounts on the real builds and a port move.

## How `devid db user add mysql shop` goes

1. **Checks:** the CLI sends `DbUserAdd`. The daemon checks the name, refuses the server's own accounts, and checks a given password or generates one: 24 letters and digits from the system's random source.
2. **Running server:** it waits for MySQL to run (up to 10 s), then asks the server for its accounts and refuses `shop` if it exists already.
3. **One `mysql` call** as `devid`, with these statements:
   - `CREATE DATABASE IF NOT EXISTS`;
   - `CREATE USER 'shop'@'%' IDENTIFIED BY …`;
   - `GRANT ALL ON shop.*`.
4. **Saving:** the password and the database go into `~/.devid/data/mysql-8.4/accounts.toml` (mode 0600).
5. **Reply:** the password and the `.env` lines. The CLI prints them, and the TUI shows them once with keys to copy either.

## Verification

- **Local:** on the M1, `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` passed. As always, `paths::tests::passwd_home_matches_home` fails under a fake HOME and passes with the real one. `~/.zshrc` and `~/.zshenv` were unchanged after the run.
- **CI:** `db-e2e` runs the accounts on the real builds on Ubuntu and macOS.
- **The M1 check** (details in the plan's "D3 results"):
  - **Setup:** a test daemon with a home of its own and spare ports. The home was a short path under `/tmp`, linked to the scratch folder, because of the socket limits. Its http, https and DNS ports were 18088, 18443 and 15353.
  - **Next to DBngin:** MySQL 8.4.13 ran on 3307 and PostgreSQL 18.7 on 5433, one at a time.
  - **Accounts:** the CLI and the TUI each made two accounts on each engine and gave one a new password.
  - **Laravel:** a new Laravel app on Devid's PHP 8.5 migrated with every account.
  - **How the TUI was driven:** a small Python script ran the real TUI in a pseudo-terminal (`pty.fork`, 120×40). It sent the keys (`5`, Enter, Enter, `a`, `a`, the name, Enter, Esc, `j`, `p`, `y`), and read the screen with the escape codes stripped. After each step it checked the result with `devid db env` and `devid db user list`.

## Gotchas

- **The fake clients** in `crates/cli/tests/db.rs` remember accounts in `$DEVID_HOME/db-users-<client>`. They parse only the statements Devid writes: `CREATE USER '…'`, `CREATE ROLE … LOGIN`, `createUser({user: "…"`, and the matching drops. A new kind of statement needs a line there too.
- **The help popup:** the TUI's help must fit in 20 rows (a test draws it at 100×20). The Databases line replaced the blank line under the global keys.
- **Page numbers:** the Databases page took number 5, so Logs and Settings moved to 6 and 7.
- **The main account's `.env`:** an empty `DB_DATABASE=` would mean no database in Laravel, so the line is left out rather than empty.
- **GitGuardian** reads password-like literals in tests as hardcoded passwords:
  - `"pw123"` in the TUI tests, now a generated value;
  - the arguments `"password", "postgresql", "postgres"` in the CLI test, now taken from the daemon crate.

  Its pull request check scans every commit of a PR, so a flagged commit keeps it red until the incident is marked in GitGuardian. The changes came back as one commit on `db-d3-v2` (#150) after #148; the CLI test's arguments were found only then.
- **`db-e2e`:** D2's PostgreSQL check made a role `shop` by hand, which made D3's `devid db user add postgresql shop` refuse it, as it should. That role is now `probe`, with a random password.
