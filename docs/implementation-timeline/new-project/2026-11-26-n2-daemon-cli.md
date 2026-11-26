# Phase 15 N2: `devid new` in the daemon and the CLI

> Date: 2026-11-26 · Branch: `phase15-n2` · Plan and results: `docs/roadmap/phase-15.md`, "N1 results" and "N2 results" · Decision: D-063

## What shipped

- **`crates/daemon/src/newproject.rs`:**
  - **The templates:** `php`, `laravel`, `wordpress`, `symfony`, `codeigniter` and `static`, with their kits and databases.
  - **The checks:** `check_name`, `check_dir` and `choose`.
  - **The steps and their progress:** `steps` and `Job`.
  - **`Runner`:** Devid's PHP and its ini for Composer, artisan and WP-CLI; the commands' environment; the log, with passwords hidden.
  - **`set_env`:** `.env` editing.
  - **`run`:** each template's steps.
  - **`templates_json`:** what `--list` shows.
- **`server.rs`:**
  - `NewTemplates`, `NewProject` and `NewProjectProgress`;
  - `new_project`, which does the checks, runs the job, then links, isolates and saves commands in one `mutate`;
  - `JobHost`, which starts a database engine if needed and makes the account.
- **`install.rs`:** `wp_cli_phar` and `ensure_wp_cli` (the newest release, SHA-512 checked), and `wp` among the shims.
- **`releases.rs`:** `sha512_hex`.
- **`commands.rs`:** `env_for` is public, so the job's tools get the same environment as commands.
- **CLI:**
  - **`devid new`:** with `--list`, `--path`, `--php`, `--db`, `--kit` and `--open`;
  - **The offer to park `~/Sites`:** made in a terminal when no folder is parked;
  - **Its output:** each step as it starts, then the URL, the account, WordPress's admin login, the commands and the log.
- **The `wp` shim:** WP-CLI with `error_reporting = E_ALL & ~E_DEPRECATED`.

## Checked

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- `crates/cli/tests/new.rs` runs every PHP template with a fake PHP that plays Composer and artisan.
- On the owner's M1, a second daemon with its own home and ports made every template for real; see "N2 results".
