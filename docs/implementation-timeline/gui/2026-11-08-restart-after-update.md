# Fix: Devid.app moves to a newly installed version by itself (v0.10.1)

> Date: 2026-11-08 · Branch: `fix/app-restarts-after-update`

## Problem

The owner closed the window (the app kept running as the menu-bar icon, D-027), ran `devid self-update` to 0.10.0, then opened Devid again. The window said "Devid 0.10.0 is installed. This window is still 0.9.1. Close it, and the menu-bar icon restarts as the new version."
- `devid setup` swaps the whole `~/Applications/Devid.app` (`place_app`) but leaves the running process alone.
- Opening the app from Spotlight or Finder sends "reopen" to that running process (pid 81251, running since 2026-11-06 through several in-place restarts), which opened its own 0.9.1 window.
- The app only moved to the binary on disk when its window closed (D-027), and the window had closed before the update.

## Fix

- At start, the app records its binary's device and inode. A swapped bundle puts another file at the same path.
- **Menu-bar icon:** on each 15 s status poll, if the binary at the path changed and no window is open, the process `exec`s the new binary with `--background` (same pid, as the D-027 restart does).
- **Opening the window** (reopen from Spotlight or Finder, Open Devid, Settings…): if the binary changed, the process `exec`s the new binary without `--background`, which opens its window in the new version. This covers the up to 15 s before the poll.
- An open window is never replaced; the version banner still covers an update while the window is open.
- It never restarts because the daemon's version differs, and not while the bundle is being swapped (no file at the path). If `exec` fails, the app keeps running and does not try again for that binary, so a failed app install cannot cause a restart loop.
- Version 0.10.1.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`: pass. New unit test `restarts_only_for_another_binary_at_the_path`.
- On the owner's Mac, with a copy of the app in a test folder (not `~/Applications/Devid.app`), started with `--background`, its bundle then swapped the way `place_app` does: 13 s later the same pid ran the new file (`lsof` txt inode 58083690 → 58083717), still with `--background`.
- Not run by hand: the reopen path (it needs a click in the menu bar or LaunchServices routing to the test copy). It uses the same check and the start path a normal launch uses.
- The fix works from the update after the one that installs 0.10.1: the app running during that update is still the old one.
