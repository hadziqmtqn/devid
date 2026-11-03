# Fix: `php artisan serve` through the shim (v0.2.1)

> Date: 2026-11-03 · Branch: `fix/shim-home`

## Problem

On the owner's Mac, `php artisan serve` (via Devid's `php` shim) failed with `panicked at crates/daemon/src/paths.rs: HOME is not set`, once per restart attempt. There were two causes:
1. Laravel's `ServeCommand` starts `PHP_BINARY -S …` with a filtered environment, so the child has no `HOME`, and `Paths::from_env` panicked.
2. The shims were symlinks to `devid`. PHP resolves symlinks for `PHP_BINARY`, so the child ran `devid -S …` instead of the shim. With `HOME` fixed alone, it would still have failed.

## Fix

- `paths::user_home()`: `$HOME`, or the passwd entry (`getpwuid_r`) when it is unset; `install.rs` uses the same function.
- Shims are hard links to `~/.devid/bin/devid`, re-created on every install or update, so `PHP_BINARY` is `~/.devid/bin/php` and child processes go through the shim (same PHP version and ini).
- Version 0.2.1.

## Verification

- `cargo test`: all pass. New: `passwd_home_matches_home`, `shims_are_hard_links_to_the_installed_binary` (also re-linking an existing install).
- Real PHP 8.5 on the owner's Mac, Laravel-style: a script started `PHP_BINARY -S` with only `PATH` in the environment.
  - Before: the shim panicked.
  - After: `PHP_BINARY` was the shim, and the child served `8.5.12 ini=~/.devid/php/8.5/etc/php.ini`.
- Existing installs get the hard links on `devid self-update` (setup re-links the shims).
