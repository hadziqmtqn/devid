# The app updates devid in the background and reopens as the new version (D-039, v0.12.1)

> Date: 2026-11-09 · Branch: `feat/app-update-in-background`

## Problem

Updating from Settings opened Terminal for `devid self-update`. On the owner's Mac (0.10.1 → 0.12.0) Terminal stayed open at "[Process completed]", and the app window still said "Devid 0.10.1" until it was closed and opened again.

## Change

- **CLI:** `devid setup --no-sudo` and `devid self-update --no-sudo`. A system setup step that would need the password is skipped and reported (exit code 3); the rest installs. `self-update --no-sudo` passes the flag to the new binary's setup and says "devid X is installed, but its system setup … needs your password".
- **App:**
  - "Update Devid…" asks once, then runs `self_update` (`devid self-update --no-sudo`, up to 10 min) with "Updating Devid…" on the button.
  - When it is done, `relaunch` replaces the process with the new binary, with its window on Settings (`--page settings`).
  - Only a "needs your password" reply offers Terminal, for `devid setup`.
- **Version:** 0.12.1.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`), `cargo test`: pass.
- `self_update` integration test: `--no-sudo` reaches the new binary's setup; a setup that exits 3 gives the "needs your password" error.
- Not run by hand: the click in Devid.app. It takes effect from the update after 0.12.1 is installed, because the running app performs the update.
