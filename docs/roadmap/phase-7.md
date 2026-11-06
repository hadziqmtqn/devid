# Phase 7 Plan: desktop app (Tauri)

> Scope: a menu-bar desktop app for macOS, built with Tauri v2 and plain HTML ported from the revised prototype, as another client of the daemon (D-023, D-026)
> Verified against: 2026-11-06
> Status: done 2026-11-06. G1 (spike) and G2 (window pages) 2026-11-05; G3 (menu bar), G4 (Devid.app) and G5 (docs and release) 2026-11-06; see "G1 results" and "G2" to "G5 as built"

Devid v1 has a CLI and a TUI (D-012). Phase 7 adds the desktop app from the revised prototype (D-026: the Claude Design board "Devid — macOS style", copied to `devid-prototype/redesign/`): a menu-bar icon plus a window with the same pages as the TUI. The daemon does the work, as it does for the CLI and the TUI.

## Ground rules

- **Client rule (D-012):**
  - The app only renders daemon state and sends daemon requests.
  - Every action it offers is already a CLI subcommand.
  - New behaviour goes into the daemon first.
- **One way in:**
  - The app's Rust side talks to `~/.devid/run/devid.sock` with the `Request` types from `crates/daemon`, like the CLI.
  - The web view calls only a fixed list of Tauri commands. Tauri v2 capabilities allow those and nothing else: no shell, file system or HTTP plugin.
  - The app loads only its own bundled files, with a strict CSP.
  - Devid opens no network port for it.
- **Frontend:**
  - Plain HTML, CSS and JS ported from the prototype's markup and macOS CSS, wired to real data. Claude Design's runtime is not shipped (D-026).
  - No Node, npm, bundler, React, Tailwind or shadcn (D-023, D-014).
- **Look:**
  - The prototype's, which follows macOS System Settings (D-007).
  - Light and dark follow the system; the prototype already has both.
  - D-017 (terminal palette) is for the TUI only.
  - UI text is English. Results and errors use the CLI's wording and say only what the daemon reported (D-026).
  - The window opens at 1080×680, clamped to the visible screen area, with a minimum of 760×520 and no narrow layout (D-026).
  - Icons: the padlock-and-prompt logo. `icon.icns` comes from per-size PNGs through `iconutil`, and the menu-bar icon is the outline padlock as a template image (D-026).
- **Resources:**
  - At idle only the menu-bar icon runs; there is no Dock icon. While the window is open, Devid is also in the Dock and Cmd+Tab (D-030).
  - The window and its web view are created when opened and destroyed when closed. Then the app restarts itself in place as icon-only, which frees the memory the window left behind (D-027).
  - While the window is visible, the app polls `status` every 2 s, as the TUI does. When the window is hidden, it stops.
- **Builds:**
  - `crates/gui` is a workspace member, but not one of the `default-members`. A plain `cargo build` or `cargo test` of the daemon and CLI never compiles Tauri, which matters on a 2-core Mac.
  - CI checks the app explicitly (`-p devid-gui`).

## Prototype → app

| Prototype | App | Content and actions |
|---|---|---|
| Menu bar | **Menu-bar icon** | State at a glance: running, stopped, or something failed. Start All, Stop All, Restart, Open Devid, Quit. |
| Sites | **Sites** | The same as the TUI's Sites page (`docs/roadmap/phase-4.md`): parked folders, the site list, open, isolate, logs, link and unlink (with confirmation). |
| PHP | **PHP** | Versions (install, default, update). Per version: Extensions, meaning Xdebug and pcov plus the Phase 6 catalog with install and remove, and php.ini (opens the editor). |
| Web Server | **Server** | nginx, Caddy or Apache with status: install, switch, restart, stop, start, view config. |
| Logs | **Logs** | Pick a log, tail it with follow, clear it (with confirmation). |
| Settings | **Settings** | Domain, ports and storage (read-only, as in the TUI); updates; `doctor`; launch at login (off by default). |

## Milestones

| # | Milestone | Done when |
|---|---|---|
| G1 | **Spike:** `crates/gui` with a menu-bar icon and one read-only Sites window filled from the daemon. Measure on the owner's Mac (see "Spike limits"). Decide how CI builds the app: Ubuntu with the WebKitGTK dev packages, or a macOS job. | the numbers are in PROJECT_CONTEXT, and the limits pass or the fallback is decided |
| G2 | **Window pages:** Sites, PHP (with the Phase 6 extensions), Server, Logs and Settings from the prototype. Every action uses an existing daemon request; destructive actions ask first. | the app does everything the TUI does, on the owner's Mac |
| G3 | **Menu bar:** state icon, Start All, Stop All, Restart, Open Devid and Quit. Launch at login is opt-in. Automatic update checks with a dot on the icon (D-028). | the menu works with the window closed, and idle stays within the G1 limits |
| G4 | **Packaging and install:**<br>• `Devid.app`, ad-hoc signed (D-010), built in the devid release workflow on `macos-15-intel` and published with the `devid` binary in `devid-binaries`<br>• `devid setup --gui` installs it into `~/Applications` (no sudo)<br>• `devid self-update` updates it<br>• `devid uninstall` removes it<br>• if the daemon reports another version, the app says to run `devid self-update` | the owner gets `Devid.app` through `devid self-update` and opens it from Finder |
| G5 | **Docs and release:** a README section for the desktop app, the timeline record, and a release. | released and installed on the owner's Mac |

Each milestone is one worktree and one PR.

### Spike limits (G1)

Measured on the owner's Mac with `footprint` and `ps`. The app's WebKit processes (`com.apple.WebKit.*`) count toward the app's total. For comparison, the daemon uses 2.3 MB and php-fpm 8.5's master 16 MB (2026-11-04).

| State | Limit |
|---|---|
| Menu-bar icon only (window never opened) | at most 30 MB in total, about 0% CPU over 5 minutes |
| Window open on Sites | recorded, no limit (it is open briefly) |
| 30 s after the window closes | the app's WebKit processes have exited, and the total is within 5 MB of "icon only" |

Also recorded:
- cold and incremental build time of `devid-gui`;
- the size of `Devid.app`;
- confirmation that `cargo build` of the daemon and CLI is unchanged.

**If the limits had failed:** the fallback keeps Tauri but drops the resident icon. The app runs only while its window is open, like the TUI, so its idle cost is zero. That would be a new decision; a Swift menu-bar app is the other way out.

### G1 results (2026-11-05)

Measured on the owner's Mac (Intel, 2 cores, macOS 14.8) with the release build of `crates/gui`: Tauri 2.12.5, the system WKWebView, a template menu-bar icon (Open Devid, Quit Devid) and a read-only Sites window filled by the `status` and `sites` requests. "Footprint" is `footprint`'s figure, the one Activity Monitor shows. RSS also counts system frameworks shared with every app.

| State | Result | Limit |
|---|---|---|
| Icon only, window never opened | 21 MB footprint (41 MB RSS), 3 threads. CPU time 0.00 s over 5 minutes. No WebKit process. | ✅ at most 30 MB, about 0% CPU |
| Window open on Sites | app 41–46 MB, WebContent 39–67 MB, Networking 2.5 MB: about 82–115 MB in total | recorded |
| 30 s after the window closes, app only keeps running | WebContent has exited, Networking (2.5 MB) stays, the app stays at 41–46 MB. About 43–48 MB in total, still the same two minutes later. | ❌ within 5 MB of icon only |
| 30 s after the window closes, app restarts itself (D-027) | 21 MB, then 22 MB after two minutes, same pid. No WebKit process. | ✅ |

- **What the closed window leaves behind:** about +20 MB in the app process:
  - IOKit 7.9 MB, IOAccelerator 4.0 MB and 2.0 MB of other graphics memory;
  - malloc 3.6 MB;
  - CoreUI images 0.9 MB.
  Hence D-027: the app `exec`s a fresh `devid-gui --background` when its last window closes.
- **Builds:**
  - **Cold:** debug 8 min 26 s; release 10 min 39 s (`lto = true`, `opt-level = "s"`).
  - **Incremental release** after a `main.rs` change: 2 min 7 s – 2 min 50 s.
  - **First clippy** of the crate: 3 min 52 s.
- **Size:** the release binary is 8.2 MB and links only `/usr/lib` and `/System` libraries. `Devid.app` (G4) adds `icon.icns` and `Info.plist`.
- **Daemon and CLI builds are unchanged:** `crates/gui` is not a default member, so `cargo build`, `cargo clippy` and `cargo test` do not compile Tauri (`cargo tree -i tauri` finds no Tauri without `-p devid-gui`).
- **The ground rules hold:**
  - The web view calls `status` and `sites` only. They are the only app commands, through the app manifest in `build.rs` and `capabilities/main.json`. The capability also allows dragging the window by its toolbar.
  - The CSP is `default-src 'self'`.
  - No Tauri plugin and no network port.
  - The page polls `status` every 2 s while visible, and refetches `sites` when the count changes.
- **Shared client:** the socket client (`protocol::request`) moved from the CLI into `crates/daemon`, so the CLI, the TUI and the app use one implementation.
- **CI:** a `gui` job on `ubuntu-latest` installs `libwebkit2gtk-4.1-dev` and runs clippy and the tests for `-p devid-gui`. A macOS job would cost ten times the minutes. The app ships for macOS only, so the `cfg(target_os = "macos")` lines (Dock icon, title bar, reopen) are checked on the owner's Mac and in the release build (G4).
- **Icons:** `crates/gui/icons/render.swift` draws `icon.png` (1024 px, logo C) and `tray.png` (40×44 px, the outline template) from the logo's SVG geometry with CoreGraphics. The per-size `.icns` (D-026) comes in G4.
- **Not measured yet:**
  - the cold start of `Devid.app` from Finder;
  - the window on a second screen;
  - the menu-bar position after the restart (G3).

### G2 as built (2026-11-05)

The window has the five pages, with every action the TUI has.

**The web view's API** (`crates/gui/src/api.rs`, allowed one by one in `capabilities/main.json`):

| Command | What it does |
|---|---|
| `daemon` | Sends any daemon request except `stop` (the daemon itself). PHP and web server installs always download (`from` is forced to none). |
| `open` | macOS `open` for a site's URL, a site or parked folder in Terminal, a Devid path in Finder, or a php.ini in the default text editor. Each target is looked up in the daemon's own data first, so no other path or URL can be opened. |
| `copy_url` | Copies a site's URL (`pbcopy`). |
| `server_config` | Reads the generated config of a web server named by the daemon, read-only. |
| `choose_folder` | macOS's folder picker (`osascript`, fixed prompts), for Link and Park. |
| `doctor` | `devid doctor` in the user's interactive login shell. Its PATH checks need PATH as Terminal sees it, which an app started from Finder does not have. The lines are parsed into checks. |
| `devid_check` | `devid self-update --check`, its one line. |
| `in_terminal` | `devid setup`, `self-update` or `uninstall` in Terminal (a `.command` file in `~/.devid/run`). They may ask for the sudo password, as from the TUI's handover. |

**TUI to app:**

| Page | TUI | App |
|---|---|---|
| Sites | open, Terminal, copy URL, isolate, use the default PHP, unlink (asks), link, park, unpark | the same. Isolating is the row's PHP menu. Link and park use the folder picker. Unpark asks first (D-026). Parked folders also have Show in Finder. |
| PHP | install, default, Extensions, Xdebug, pcov, php.ini, restart, check and install updates | the same. Updates are on Settings only. php.ini opens in the default editor, and the toast says to restart php-fpm after saving. A GUI editor gives no "editor closed" moment to restart on. |
| Extensions | catalog install (several, with a progress line), remove (asks), Xdebug and pcov | the same dialog. The install confirmation replaces the button row and the remove confirmation replaces the extension's row (D-026). |
| Server | install, use, restart, stop and start, view config, stop all (asks), start all | the same. Use asks first. Stop all and start all are on the sidebar card. View config is a read-only dialog. |
| Logs | pick a log, follow every second, clear (asks) | the same |
| Settings | domain, ports, storage, doctor, check updates (devid and binaries), update binaries (asks), self-update, setup, uninstall (ask) | the same. Self-update, setup and uninstall open Terminal. Self-update and uninstall ask first. |

- **Messages:** results use the CLI's and TUI's wording in sentence case (D-026). The update summary, the extension progress line and the install summary follow `tui/mod.rs` and `cli/src/main.rs` line by line.
- **Not in G2:** launch at login (G3) and the menu-bar menu (G3).
- **Fixed after G2 (v0.5.0):**
  - The folder picker gives the focus back to the window.
  - macOS actions run `/usr/bin/open`, `osascript` and `pbcopy` by full path, because a terminal's PATH can put its own `open` first.
- **Added after G2 (v0.5.0):** Remove site for parked sites, with a Served / Removed filter (D-029).

### G3 as built (2026-11-06)

- **Menu-bar icon** (`crates/gui/src/tray.rs`): it asks the daemon for `status` every 15 seconds and redraws only when something changed. Actions from the window and the menu redraw it right away.
  - **Icon:** one template image per state, drawn by `render.swift`:
    - running: the plain padlock;
    - stopped, or no daemon: dimmed;
    - something failed: a "!" badge.
    Each also comes with a dot when updates are available (D-028).
  - **Menu:**
    - the status line, as on the window's card;
    - "Updates available (n)…", which opens Settings;
    - Stop all and Restart <server> while running, Start all when stopped;
    - Open Devid, Settings… and Quit Devid.
    An action's error shows as a line in the menu until the next refresh.
  - **No confirmation for Stop all here:** the prototype's quick menu has none, and Start all undoes it. The window keeps its confirmation (D-026).
- **Daemon:**
  - **D-028:**
    - the daemon checks for updates 3 minutes after it starts, then every `update_check_minutes` (20). It checks the newer devid release and what `update --check` would change;
    - `status` carries the last result, and an update clears its list;
    - one check or update runs at a time, since they share the downloaded manifest;
    - `devid status` prints an "updates:" line.
  - **D-031:**
    - `autostart` (`devid autostart on|off`);
    - the app's login item (`devid login-item on|off`, LaunchAgent `dev.devid.app`, written for the next login, never loaded at once);
    - `status` carries both, and `devid uninstall` removes the login item.
- **TUI:** the Settings page shows both login settings and the last update check; `a` toggles "serve sites at login".
- **Window:**
  - **Settings, Login section:** "Serve sites at login" and "Open Devid at login". The app's switch passes its own binary through the new `login_item` command, and the generic `daemon` command refuses `login_item`.
  - **Settings, Updates:** filled from the automatic check, with a note on how often it runs.
  - **Sidebar:** Settings shows a dot when there are updates.
  - **Menu bar's "Settings…":** opens the window on Settings.
- **Idle with the polling icon:** release build, 5 minutes, icon only: 9.2 MB footprint (30 MB RSS) and 0.03 s of CPU, with no WebKit process. That is within G1's limits (30 MB, about 0% CPU).
- **Not in G3:**
  - the prototype's site list and default-PHP picker in the menu (the window has both);
  - the icon's position after D-027's restart, which needs `Devid.app` (G4).

### G4 as built (2026-11-06)

- **`scripts/bundle-app.sh`:**
  - **Build:** it builds `devid-gui` in Tauri's production mode (`--features custom-protocol`) and assembles `Devid.app` by hand: `Info.plist`, `icon.icns`, the binary.
  - **Signing and packaging:** it signs the bundle ad hoc (D-010) and zips it with `ditto` as `Devid-darwin-x86_64.zip`.
  - **`Info.plist`:**
    - name "Devid";
    - `dev.devid.app`;
    - `LSUIElement`: menu-bar only until a window opens (D-030);
    - macOS 12 or later.
  - **Where it runs:** on the owner's Mac and in `release.yml`. The release publishes the zip with the devid binary, in `checksums.txt`, to `devid-v<version>` and `devid-latest`.
- **`devid setup --gui`:** installs `~/Applications/Devid.app` with no sudo.
  - **Version:** it comes from the installed devid's own release (`devid-v<version>`), so the app and the daemon match.
  - **Checks:** the zip is checked against `checksums.txt`.
  - **Swap:** the zip is unpacked next to the app and swapped in whole, and a failed install keeps the app that was there.
  - **Updates:** once the app is installed, every `setup` updates it, including the one `devid self-update` runs.
- **`devid uninstall`:** quits the app and deletes it, along with the login item (G3).
- **Version check in the window:** if the daemon's version differs from the app's, a banner says what to do.
  - **Daemon newer:** close the window, and the icon restarts as the new version (D-027).
  - **App newer:** run `devid self-update`.
  The older-daemon error (G2 fix) stays for requests the daemon does not know.
- **Checked on the owner's Mac (local bundle):**
  - macOS shows it as "Devid" (`dev.devid.app`), ad-hoc signed, 8.6 MB (2.0 MB zipped);
  - the web view reaches the daemon in production mode;
  - opened through LaunchServices it is a foreground app with its window, and started with `--background` it is a UIElement with no window until reopened.
  - `spctl` rejects it, as expected for a bundle that is not notarized; Gatekeeper checks only files with a quarantine attribute, which a `curl` download does not have.
- **Decided:**
  - **Opt-in install:** `devid setup --gui`, as proposed; CLI-only users get nothing new.
  - **A bundling script, not the Tauri CLI:** no large `cargo install`, and the same script for local and release builds.

### G5 as built (2026-11-06)

- **README:**
  - The desktop app section and the Install section show one command for both cases: `… | sh -s -- --gui` installs the daemon, the CLI and Devid.app, and `… | sh` installs no app (#53).
  - The `devid-binaries` README says the same (hadziqmtqn/devid-binaries#10).
- **Release notes:** `release.yml` says to add `-s -- --gui` after `sh` for the app. The published notes of `devid-v0.7.0` and `devid-latest` were edited to match.
- **Release:** v0.7.0 (G4). G5 changed docs only, so no new release.
- **Installed on the owner's Mac:** `devid self-update` (0.6.0 to 0.7.0), then `devid setup --gui`. Every other setup step reported "already there", and none asked for sudo. Checked:
  - Spotlight opens Devid with no Gatekeeper prompt. The app has no quarantine attribute (a `curl` download), so Gatekeeper never checks it, although `spctl` rejects it.
  - The Dock and Cmd+Tab show "Devid" with its icon.
  - Closing the window removes the Dock icon. The menu-bar icon stays, with no flicker, and the same process continues as `--background` (D-027).
  - **Memory:** 49 MB RSS with the window open; 5.6 MB footprint (17 MB RSS) at idle afterwards, within the G1 limit of 30 MB.

## Open questions

None. Gatekeeper on a real install was the last one, answered by G5.

## Out of scope

- arm64, Linux and Windows builds (D-010, D-019). The code stays portable; only macOS x86_64 is built and tested.
- Notarization and a paid Apple Developer membership (D-010).
- Electron, a Node toolchain, and a web UI on `localhost` (D-023).
- Notifications. D-012 mentions daemon-sent notifications, which are not built yet; they are a daemon feature, not part of the app.
