# Fix: update check with an unreadable installed version (v0.2.2)

> Date: 2026-11-03 · Branch: `fix/update-unknown-version`

## Problem

During the T5 pty session on the owner's Mac, `u` (check updates) reported `php 8.4 →8.4.27`, although 8.4.27 was installed. The empty "from" shows that the daemon could not read the installed version at that moment:
- `php::full_version` runs `php -n -r 'echo PHP_VERSION;'` with a 5 s timeout and returned None;
- `update` turned None into `""`, and `releases::newer(candidate, "")` is true for an unparsable installed version, so the check offered an update;
- `devid update` (or `U` in the TUI) would have downloaded and reinstalled PHP 8.4 for nothing.

A later check on the same machine reported everything up to date, and the probe takes about 0.1 s there. Why the probe failed that once is not proven. The daemon log has no record of it. A likely cause is a slow first start of the binary while the 2-core Mac was busy.

## Fix

- `update` skips an installed PHP, nginx or Caddy whose version cannot be read, and lists it under `unknown` in the reply. It is never offered or reinstalled.
- The CLI prints `skipped: <what> (could not read the installed version; try again, or reinstall it)` and no longer says "everything is up to date" in that case. The TUI shows "version unknown, skipped: …" as ✗.
- Version 0.2.2.

## Verification

- `cargo test`: all pass. `install_and_update_from_a_release` now also checks the case: an installed PHP whose version probe fails is skipped by `update --check`, and `update` leaves its binary untouched.
- On the owner's Mac: `devid update --check` reports `everything is up to date (release 2026.11.02-3)`, and the installed binaries are PHP 8.4.27, PHP 8.5.12 and nginx 1.30.6.
