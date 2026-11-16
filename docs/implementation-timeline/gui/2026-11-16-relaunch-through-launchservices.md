# Fix: the app comes back after an update, through LaunchServices instead of exec (D-048)

> Date: 2026-11-16 · Branch: `fix/gui-relaunch`

## Problem

The owner updated Devid from 0.13.0 to 0.14.0 from the app's Settings page. The app closed and did not come back: no window and no menu-bar icon. The daemon and the sites were fine.

## Findings

- `devid-gui --page settings` still ran with the pid it had before the update (5094, started 2026-11-15 07:23). The app's relaunch had `exec`ed the new 0.14.0 binary, as D-027 and D-039 do.
- It sat in `-[NSApplication run]` with two threads and 12 MB, so Tauri's setup, which creates the icon and the window, had not run.
- The new image's log:
  - its check-in with the process manager;
  - LaunchServices registering the swapped bundle again ("bundle … node has changed", then "Failed to get unit 68144 from store");
  - `SignalReady`;
  - then nothing. On the normal launch the day before, the window appeared 130 ms after `SignalReady`.
- `open ~/Applications/Devid.app` brought it back at once, with the window on Settings and the icon (48 MB). AppKit had been waiting for an event from LaunchServices.
- Not reproduced on demand. A test copy of the app had its own bundle id, was launched by LaunchServices, and called `relaunch` through a test-only hook (not committed). `exec` worked in every case:
  - with the window open;
  - with the icon only;
  - after swapping in another version of the bundle, the way `install_app` does.
- A plain Cocoa app that `exec`s itself worked too, with or without a swapped bundle. So the hang depends on the state of LaunchServices.

## Fix

- `restart(app, args)` hands the relaunch to `tauri::process::restart`: `open -n` on the bundle with the same arguments, then the old process exits.
- All four callers use it:
  - the window closing (D-027);
  - opening the window after an update;
  - `relaunch` after an update from the app (D-039);
  - the menu-bar icon finding a new binary.
- D-048 amends D-027 and D-039: the pid changes on a restart, and the rest stays.

## Verification

- The test copy, launched by LaunchServices:
  - **Relaunch from the open window,** after swapping in another version: a new process with `--page settings`, its menu-bar thread and its window. The old one exited.
  - **The icon's 15 s check after a new binary:** a new process with `--background` and its menu-bar thread.
- `cargo fmt --check`, `cargo clippy -p devid-gui --all-targets -- -D warnings` and `cargo test -p devid-gui` (7 passed).
- Not verified: the original hang with the fix in place, since the hang could not be reproduced on demand.
- The update from 0.14.0 still runs 0.14.0's `exec`. If the app does not come back after that update, opening Devid again brings it back.
