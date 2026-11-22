# Dev servers on demand in the desktop app and the TUI (Phase 10 O5)

> Date: 2026-11-22 · Branch: `feat/on-demand-clients` · Phase 10 O5 (`docs/roadmap/phase-10.md`, D-054)

## What it does

The app, its menu bar and the TUI show and change on-demand, which O3 and O4 built in the daemon and the CLI.

- **App, site detail of a proxy site:** an On demand card under Commands:
  - the "Start on the first visit" switch;
  - the command, as a segmented control;
  - "Stop when idle": 5, 10, 15, 30 or 60 min, or Never.
- **App, command rows:** the on-demand command carries an "On demand" chip and says "Sleeping", "Starting · pid …" or "Running · pid … · idle N min". Stop adds that it sleeps until the next visit.
- **App, elsewhere:**
  - the site detail's toolbar has Open in browser, which also wakes a sleeping site;
  - the Sites table shows a "Sleeping" chip, with "Sleeping" in place of "Not running", or an "On demand" chip while the command runs.
- **Menu bar:** "Commands · 2 running · 1 sleeping". Each sleeping site is listed as "Open devid-site (sleeping)"; a click opens it in the browser, which wakes it.
- **TUI:**
  - a proxy site's menu adds `z` On demand… (the command or Off, then the idle time);
  - Commands… marks "sleeping, on demand";
  - the Sites table shows `· sleeping`, or `· ● on demand` when that command is the only one running.

## Design

The board came first, as the plan asks: "Devid — Dev servers on demand (Phase 10 O5)" on the Devid Redesign canvas, page "Fase 10: on demand".

- **Source:** an interactive copy of the approved Phase 8 board, so it has the same look.
- **Approval:** the owner approved it on 2026-11-22.
- **The app** follows the board: the card, the chips, the status texts, the Quick menu's count, and Open in browser in the detail's header.
- **Two changes from the plan's wording:**
  - one card per site instead of a switch on a command row, since only one command per site can be on demand;
  - the TUI key `z`, since `o` already opens the site.

## How

- **`crates/gui/ui/app.js`:**
  - `onDemandCard`, `onDemandChip`, `sleepsNote`, `IDLE_STOPS`;
  - `commandRow` takes the site's `on_demand`;
  - the Sites page's detail tools.
  - Each change is one `app_set` with `on_demand` and `idle_stop`, as the CLI sends it. A time set from the CLI that is not in the list (`--idle 1`) still shows, before Never.
- **`crates/gui/src/tray.rs`:**
  - `View.sleeping` from `status.sleeping` and `commands_header`;
  - `site-open:<site>` items, which open `api::site_url` with `/usr/bin/open`;
  - the count next to the icon stays the running commands.
- **`crates/cli/src/tui/`:**
  - `PickFor::OnDemand` and `PickFor::OnDemandIdle` and `ON_DEMAND_IDLE` in `app.rs`;
  - the Sites column text in `ui.rs`, kept short since the column is 34 characters wide.

## Checks

- **The app's UI, in the app's browser:**
  - **Setup:** the real `app.js` and `app.css`, light and dark. A fake `invoke` answered with the owner's live `status` and `sites` replies (daemon 0.16.1), with on-demand states added: devid-site sleeping, erd-builder-pro's `start` running on demand with 200 s idle.

  | Case | Result |
  |---|---|
  | Sites table | devid-site: "Sleeping" chip and "Sleeping"; erd-builder-pro: "On demand" chip, "1 running" |
  | devid-site's detail | `dev` with "On demand" and "Sleeping · the next visit starts it"; the card with the switch on, `dev`, After 10 min and the sleeping line |
  | Open in browser | `open` for the site; `dev` running, "idle 0 min", the card's running line |
  | Stop when idle → After 30 min | `app_set {on_demand: "dev", idle_stop: 30}`, toast "…stops after 30 min idle" |
  | Switch off, then on | `app_set {on_demand: ""}`, the card down to the switch, the chip gone; then `{on_demand: "dev", idle_stop: 10}` |
  | Stop `dev` | toast "Stopped dev; dev sleeps until the next visit to devid-site.test", sleeping again |
  | erd-builder-pro (light) | two commands in the control, `start` pressed, "Running · pid 41551 · idle 3 min"; picking `dev:tauri` sends it with the same idle time |
  | A PHP site | no card, the usual Commands note |
  | A proxy site without commands | the switch disabled, "Add the command that starts the dev server first, such as npm run dev." |

- **Unit tests:**
  - the TUI's `z` flow: entry text, current marks, default idle time, Never, Off, only for proxy sites with commands;
  - Commands… marks and the Sites column;
  - the menu bar's view and Commands header.
- **Repository checks:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, including `-p devid-gui`, pass.
- **Not checked:** the real Devid.app against a daemon, and a click on the menu bar's Open item. They come with the installed build in O6, when the owner checks devid-site.

## Then: the site detail's explanations in tooltips (D-055)

After seeing the page, the owner asked for the explanations to move into tooltips, and for less repeated narration (2026-11-22):

- **Tooltips:** every title on the site detail has an (i) with its explanation. The notes under App, Commands, On demand and Share became their headings' tooltips.
- **On the page:** the warnings stay: PHP not installed; a busy port, now "In use by … Devid serves it once the port is free."; a stopped cloudflared; why the on-demand switch is disabled.
- **Said once:**
  - the On demand card lost its status line, since the command's row says the same;
  - that row says "Sleeping" instead of "Sleeping · the next visit starts it", and "Starting · pid …" while the port does not accept yet;
  - the chip's tooltip says when the command starts and stops.
- **The tooltip:** `tip()` and `titled()` in `app.js`, `.dv-tip` and `.dv-tooltip` in `app.css`.
  - It is fixed on the body as the menus are, so the scroll area does not clip it.
  - It opens on hover and on keyboard focus, and Esc closes it. The (i) is focusable, with the text as its label.
  - `renderPage` waits while it is open, as it does for a menu.
- **Checked in the same harness:**
  - **Pages:** devid-site (dark) and erdbpro-saas (light).
  - **Tooltips:** 10 tooltips, each focusable with its label. Hover on "Serve as" and on the Commands heading showed the text above the (i), not clipped.
  - **Keyboard and redraws:** focus showed it, a redraw waited while it was open, and Esc closed it and ran the redraw.
  - **Notes:** none left on the page.
  - **Warnings:** a missing PHP 8.3 and a busy port 8001, both still on the page.
