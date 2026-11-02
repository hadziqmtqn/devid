# Phase 4 Plan: TUI

> Scope: the terminal UI that `devid` opens with no arguments (D-012), and the daemon features the approved prototype needs
> Verified against: 2026-11-02
> Status: T1 done (#21); T2 done in code (Sites actions); T3–T5 proposed

Phase 4 gives Devid the interface from the approved prototype (`devid-prototype/index.html`) as a TUI. It takes the prototype's structure and flows, not its look (D-017).

## Ground rules

- **Client rule (D-012):** the TUI only renders daemon state and sends daemon requests. Every action the TUI offers is also a CLI subcommand, and new behaviour goes into the daemon first.
- **Colors (D-017):** the terminal's 16-color palette used semantically. Every state also has a symbol and text, and `NO_COLOR` gives a monochrome UI.
- **Resources:** the TUI runs only while it is open. It polls `status` every 2 s and stops polling when it exits. Nothing new runs in the background.
- **Keyboard first:** `↑↓`/`jk` move, `tab` switches between sidebar and page, `enter` opens the action menu, letter shortcuts are shown in the footer, `?` shows help, `q` quits. There is no mouse support in v1.
- **Dependencies:** `ratatui` 0.30 with its `crossterm` backend (0.29), justified in T1. There is no async runtime.

## Prototype → TUI pages

| Prototype page | TUI page | Content and actions |
|---|---|---|
| Sites | **Sites** | Parked folders (park/unpark), site list (URL, PHP, folder, isolated). Per site: open in browser, open in Terminal, copy URL, isolate/unisolate, view logs, unlink (with confirmation). Link a folder (path input). |
| PHP | **PHP** | Installed versions (running/stopped/failed, default) plus the versions available in the release. Install, set default, update. Per version: **Extensions** (Xdebug, pcov on/off) and **php.ini** (opens `$EDITOR`). |
| Web Server | **Server** | nginx / Caddy / Apache with status; install, switch (D-005); restart, stop, start; open the config in `$EDITOR`, read-only (Devid regenerates it). |
| Logs | **Logs** | Pick a log (daemon, web server, php-fpm per version), see the tail with follow mode, clear it (with confirmation). |
| Settings | **Settings** | Domain (TLD), ports, storage path (read-only in v1); updates (`self-update --check`, `update --check`, then apply); `doctor` results; run setup and uninstall (the TUI exits and hands over to the CLI). |

The prototype's menu-bar items (Start All, Stop All, Open Devid) appear as global keys in the TUI header. A menu-bar app stays out of scope (D-012).

## Milestones

| # | Milestone | Done when |
|---|---|---|
| T1 | **Shell:** `devid` with no arguments (and `devid tui`) opens the TUI. It has a header (daemon status, active server, default PHP), the sidebar with five pages, a page area and a footer with keys; a help overlay; a "daemon is not running" screen with the fix; `NO_COLOR`; resize handling; a clean exit that restores the terminal. | the TUI opens, navigates and exits cleanly; render tests pass with ratatui's `TestBackend` |
| T2 | **Sites page:** list and every per-site action from the table above, the link/park path input and confirmations. | all site actions work from the TUI on the owner's Mac |
| T3 | **PHP page and extensions:** daemon API + CLI `devid php ext <ver> <xdebug\|pcov> on\|off` (writes `conf.d/devid-<ext>.ini` and restarts that version, D-008); the PHP page with install, default, update, extensions and php.ini. | Xdebug can be turned on from the TUI and `php -v` shows it |
| T4 | **Server and Logs:** daemon API + CLI `devid service <name> restart\|stop\|start`, `devid stop-all` / `start-all`, and `devid logs [name] [--follow] [--clear]`; the Server page and the Logs page. | a service restart and a log tail work from the TUI |
| T5 | **Settings and polish:** the Settings page (updates, doctor, setup/uninstall handover); confirmations everywhere something is destructive; the empty and error states from the prototype; README/docs; a release (`v0.2.0`). | `devid self-update` on the owner's Mac brings the TUI |

Each milestone is one worktree and one PR, opened after the previous one is merged (no stacked PRs).

## Testing approach

- **Rendering:** draw each page into `ratatui::backend::TestBackend` from a fake state and assert on the text. This also runs in CI on Linux.
- **Behaviour:** key handling is a pure `fn(state, key) -> (state, Option<Request>)`, tested without a terminal or a daemon.
- **Daemon additions** (extensions, services, logs) get integration tests with the existing fakes.
- **Manual check:** on the owner's Mac, against the real daemon, at the end of every milestone.

## Out of scope for v1

Mouse support, themes, editing the TLD or ports from the TUI (changing the TLD replaces the CA and the resolver; that stays a deliberate CLI step), and a menu-bar app (D-012).
