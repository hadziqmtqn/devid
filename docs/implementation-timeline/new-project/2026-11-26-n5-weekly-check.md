# Phase 15 N5: the weekly check of every template, and 0.22.0-rc.1

> Date: 2026-11-26 · Branch: `phase15-n5` · Plan and results: `docs/roadmap/phase-15.md`, "N5 results" · Decision: D-063

## What changed

- **`scripts/new-e2e.sh`:** `devid new` with the real create tools.
  - **The setup:** a daemon in a throwaway `DEVID_HOME` on spare ports, with the binary, the `php`, `composer` and `wp` shims and `composer.phar` as `devid setup` installs them, and a parked folder. The daemon runs without `CI` and `GITHUB_ACTIONS`.
  - **The projects:** 24, from every template, with Laravel's databases and starter kits and Symfony's databases and skeleton.
  - **The checks:** each site's own page over HTTPS, the dev servers through the proxy, and checks per template (the kits' assets, `.env`, Symfony's database, WordPress's `siteurl` and login page).
  - **The report:** every project runs even after a failure, and a table gives each one's time, first visit and result.
  - **`SPARE_PORTS=1`** keeps the databases and dev servers off their usual ports, for the owner's M1. `NEW_E2E_LOGS` keeps the logs.
- **`.github/workflows/new-templates.yml`:**
  - **When:** Mondays at 03:00 UTC, by hand, and on pull requests that change `newproject.rs`, the script or the workflow.
  - **Where:** macOS 15 (Apple silicon) and Ubuntu 24.04.
  - **Node:** the newest LTS as nvm's default.
  - **The report:** the table in the run's summary, and the logs as an artifact.
- **Version 0.22.0-rc.1** in `Cargo.toml` and `Cargo.lock`. `release.yml`'s notes name `devid new`.
- **`README.md`:** the weekly check, under "Developing Devid".
- **The docs:** `phase-15.md`'s "N5 results", the N3 note on the first visit's 502, D-063, the docs index and PROJECT_CONTEXT.

## Checked

- **On the owner's M1** (`SPARE_PORTS=1`): all 24 projects were made and answered, next to the owner's Devid 0.21.0 and DBngin. Nothing was left running.
- **The first visit's 502** seen in N3 came back for React, Vue and SvelteKit in the first run. nginx's error log showed the cause: the test moved the port right before the visit, and the old worker used the old one. After a second's pause in the script, all three answered 200 at once.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## Next

- The pull request's run of the workflow on GitHub's runners.
- The owner merges, and creates the pre-release v0.22.0-rc.1.
- The rc on the M1, with the owner's try of the app's sheet, and on the Ubuntu, Arch and CachyOS VMs.
- "Version 0.22.0", then the guide's page merges (devid-site#25, a draft, in English and Indonesian).
