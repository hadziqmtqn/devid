# Ignore and unignore a parked site, in the CLI, TUI and app (v0.5.0)

> Date: 2026-11-06 · Repository: `devid` · Branch: `feat/site-ignore`

## What shipped

- **D-029, ignore a site:** a parked site can be removed from Devid and restored later. Its folder is never touched.
  - **Config:** `config.toml` gains `ignored`, the folders of removed parked sites. `sites::resolve` skips them.
  - **Daemon:** two new requests, `ignore` and `unignore`.
    - `ignore` refuses a linked site and points to `devid unlink`.
    - The `sites` reply lists the ignored sites (name and folder).
  - **CLI:**
    - `devid ignore <name>` prints "ignored <name>; the folder is still in <parked folder>".
    - `devid unignore <name>` prints "serving <name> again".
    - `devid sites` lists the ignored names.
  - **TUI:** "Remove site… (the folder stays)" in a parked site's menu asks first. `I` picks an ignored site to serve again. The hint and help lines name it.
  - **App:**
    - "Remove site…" in a parked site's ⋯ menu, with the prototype's confirmation.
    - A Served / Removed filter on the Sites line. Removed sites have Restore.
- **D-028 recorded:** automatic update checks every 20 minutes, shown as a dot in the menu bar and Settings. It is built in G3.
- **Two app bugs the owner found in G2:**
  - **Open in browser went to cmux's in-app browser.** The app was started from a cmux terminal and inherited its PATH, where cmux's `open` comes first. The app now runs `/usr/bin/open`, `/usr/bin/osascript` and `/usr/bin/pbcopy`.
  - **After Cancel in the folder picker, the window went behind other apps.** `choose_folder` now focuses the window again when the picker closes.
- **An older daemon's error:** an older daemon answers a request it does not know with a serde error ("unknown variant `ignore`, expected one of …"). The owner saw it when trying Remove site against the installed 0.4.1. The app now says instead: "The running daemon is Devid 0.4.1, which does not know this action yet. Update it with `devid self-update` (this app is 0.5.0)." G4's version check covers the general case.
- **Version:** 0.5.0, since the CLI and the daemon protocol grow.

## Verification

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` | pass |
| `crates/cli/tests/sites.rs` (a real daemon with a fake nginx) | `ignore api` prints its line, drops `api.test` from the sites and from nginx.conf, and lists it under `ignored`; the folder stays. `ignore shop` (linked) is refused, `unignore api` serves it again, and a second `unignore` is refused. |
| `sites.rs` unit test | an ignored parked folder is skipped; a link of the same name still wins; `ignored_name` maps a folder name to a site name |
| TUI unit tests | a parked site's menu has remove (`otcir`), remove asks and sends `ignore`, and `I` sends `unignore` or says there is nothing to restore |
| `cargo clippy -p devid-gui`, `cargo test -p devid-gui` | pass |
| UI in Chrome with the daemon's real replies plus two ignored sites (mocked actions) | the filter switches between "Served (18)" and "Removed (2)", also by keyboard. Removed sites show Restore. The ⋯ menu has "Remove site…" in red, and its confirmation shows the prototype's wording with Cancel focused. |

## Follow-ups

- Release v0.5.0 and run `devid self-update`. The installed daemon (0.4.1) does not know `ignore` yet, so the app's Remove site works only after the update.
- **The owner checks in the app:**
  - Open in browser goes to the default browser.
  - Cancel in the folder picker keeps the Devid window in front.
- G3: the menu-bar menu, launch at login, and D-028.
