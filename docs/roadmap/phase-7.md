# Phase 7 Plan: desktop app (Tauri)

> Scope: a menu-bar desktop app for macOS, built with Tauri v2 and plain HTML ported from the revised prototype, as another client of the daemon (D-023, D-026)
> Verified against: 2026-11-05
> Status: planned; starts after Phase 6 is released (E5)

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
  - At idle only the menu-bar icon runs; there is no Dock icon.
  - The window and its web view are created when opened and destroyed when closed.
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
| G3 | **Menu bar:** state icon, Start All, Stop All, Restart, Open Devid and Quit. Launch at login is opt-in. | the menu works with the window closed, and idle stays within the G1 limits |
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

**If the limits fail:** the fallback keeps Tauri but drops the resident icon. The app runs only while its window is open, like the TUI, so its idle cost is zero. That would be a new decision; a Swift menu-bar app is the other way out.

## Open questions

- **Install by default or opt-in?** Proposal: opt-in (`devid setup --gui`), so CLI-only users get nothing new.
- **Tauri's CLI (`cargo tauri build`) or a small bundling script for the `.app`?** The Tauri CLI is another large `cargo install`. Decide in G1 or G4.
- **Gatekeeper:** a file downloaded with `curl` carries no quarantine attribute, so an ad-hoc signed app should open without a prompt. Verify in G4.

## Out of scope

- arm64, Linux and Windows builds (D-010, D-019). The code stays portable; only macOS x86_64 is built and tested.
- Notarization and a paid Apple Developer membership (D-010).
- Electron, a Node toolchain, and a web UI on `localhost` (D-023).
- Notifications. D-012 mentions daemon-sent notifications, which are not built yet; they are a daemon feature, not part of the app.
