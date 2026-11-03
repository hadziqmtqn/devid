# `devid setup` installs PHP and a web server (v0.2.3)

> Date: 2026-11-03 · Branch: `feat/setup-installs`

## Problem

After `install.sh` and `devid setup`, a new user still had to run `devid php install 8.5` and `devid server install nginx` before any site worked. Several messages also still suggested `--from <dir>`, which predates release downloads.

## Change

- `devid setup` ends with two more steps, after the sudo step so the password prompt comes before the downloads:
  - **PHP:** without `--php`, it installs the configured default (8.5) only when no PHP is installed. With `--php <ver>` (repeatable), it installs the missing versions and makes the first one the default.
  - **Web server:** without `--server`, it installs the active server (nginx on a new install) if it is missing. With `--server <name>`, it installs that server if needed and switches to it.
  - `--no-php` and `--no-server` skip either step. clap rejects unknown versions and servers before anything changes, using `php::SUPPORTED` and `webserver::SERVERS`.
- Choosing flags instead of prompts: `install.sh` runs as `curl … | sh`, where stdin is not a terminal. The script now passes its arguments through (`… | sh -s -- --php 8.3`).
- `self-update` runs the new binary's `setup` without flags, so on an existing install the new steps change nothing.
- The `--from <dir>` hints in errors, `php list`, `doctor` and the setup "Next:" line now name the release commands. The doctor web server check names the active server instead of always "nginx".
- README (Install, First steps, Roadmap) and D-014 updated. Version 0.2.3.

## Verification

- `cargo test`: all pass. The `php_plan` unit test covers a fresh machine, a re-run, duplicates and the default. `setup_and_uninstall_dry_runs_change_nothing` checks the dry-run plan for the default, `--php 8.3 --php 8.4 --server caddy`, and `--no-php --no-server`.
- On the owner's Mac (dry run with the new binary): `PHP 8.4, 8.5 already installed` and `nginx already installed`, so a re-run changes nothing.
- The real download path reuses `PhpInstall` and `ServerInstall`, which `install_and_update_from_a_release` already tests. setup itself is not run end to end in CI because it loads a LaunchAgent and needs sudo.
