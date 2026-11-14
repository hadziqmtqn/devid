# The Logs page in two panes: sources by site, levels, search; log text without escape codes

> Date: 2026-11-14 · Branch: `gui/logs-sources`

## Problem

- The Logs page showed every log name as one row of buttons. With a few sites running commands that row held a dozen file names, such as `app-erd-builder-pro-dev:pg:local.log`, with nothing saying which site or command each belongs to.
- Logs from cargo, tauri and vite were full of raw escape codes (`^[[1m^[[96m`) and, for cargo's progress bar, hundreds of `\r` redraws in one line. Both the app and the TUI showed them as they were.

## Change

The design is the "Logs" board on the v0.12.x page of the Devid Redesign canvas.

- **Daemon (`logs::read`):**
  - takes escape sequences out (CSI, OSC, and two- or three-byte escapes);
  - keeps an escape sequence or a UTF-8 character that the writer had not finished for the next read, so a poll never splits one;
  - leaves `\r` in, because a progress line can continue in the next read.
- **TUI:** a line shows what follows its last `\r`.
- **App, Logs page:**
  - **Sources on the left:**
    - grouped as Devid, Web server, PHP, one entry per site (its commands, logs of removed commands, and its share), then Other;
    - a dot for what is running;
    - a filter;
    - sites collapse and show "N running";
    - a red badge counts the error lines a log gained since the page first saw it; it clears when the log is opened. A collapsed site shows its logs' total.
  - **The open log on the right:**
    - its site and command as the title, with the path and running or stopped;
    - a ⋯ menu: Show in Finder, Copy path, Clear log…;
    - All / Warnings / Errors;
    - search with a match count;
    - Follow.
  - **Following:** scrolling up turns Follow off and shows "Jump to latest · N new"; scrolling to the end or the button turns it on again. The log is read every second while the page is open, also with Follow off.
  - **Lines:** an unfinished last line is shown and replaced as it grows, so a progress bar is one line. `[name]` prefixes from concurrently get one of two colors.
  - **Cost while idle:** the badges read only what other logs gained since the last poll, and only while the Logs page is open.
- **App, Rust:**
  - Finder may reveal a log the daemon lists;
  - `copy_log_path` copies a listed log's full path;
  - the pbcopy code is shared with `copy_url`.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`) and `cargo test -p devid-gui` pass. `cargo test` passes apart from `local_port` in `crates/cli/tests/sites.rs`: it failed once in the full parallel run and passed alone and with its file. This change does not touch it.
- **Unit tests:**
  - escape sequences and a cargo progress line taken out;
  - a cut UTF-8 character and a cut escape sequence wait for the next read;
  - the TUI shows a progress line's last draw.
- **The daemon code on the owner's real logs:** `app-erd-builder-pro-tauri.log` (60 KB, 292 `\r` redraws) comes out with no escape left, ending in "Finished `dev` profile … in 15m 55s".
- **The real `app.js` and `app.css` in headless Brave, light and dark:**
  - the setup: a fake `invoke` answering with the owner's live `status`, `sites` and `logs` replies and the cleaned log text, plus simulated new lines;
  - checked:
    - the source groups;
    - the badge on a collapsed site, and clearing it;
    - Follow off on scrolling up (stable in 5 runs) and Jump to latest;
    - Warnings;
    - search with its count;
    - the source filter and its empty text;
    - a progress line ending as "Finished";
    - the ⋯ menu's `open` and `copy_log_path` calls;
    - the site detail's log panel, which keeps the old layout and ignores the page's level and search.
- **Not run by hand:**
  - the page in Devid.app;
  - Show in Finder and Copy path on a real log.
