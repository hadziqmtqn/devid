# Phase 7 G4: Devid.app, `devid setup --gui`, updates through self-update (v0.7.0)

> Date: 2026-11-06 · Repository: `devid` · Branch: `feat/g4-app-bundle`

## What shipped

The details are in `docs/roadmap/phase-7.md`, "G4 as built".

- **`scripts/bundle-app.sh`:** builds `Devid.app` (Tauri production mode, a hand-made `Info.plist` with `LSUIElement`, `icon.icns`, ad-hoc signed) and `Devid-darwin-x86_64.zip`.
- **`release.yml`:** runs the script and publishes the zip, with its checksum, next to the devid binary. The timeout is 60 minutes for the Tauri build.
- **`devid setup --gui`:** installs `~/Applications/Devid.app` from `devid-v<version>`, sha256-checked and swapped in whole. Every later `setup` (and so `devid self-update`) updates an installed app.
- **Shared release code:** `releases::fetch_devid(release, name)`, used for the version-pinned app download. `checksum_for` is shared by self-update and the app install.
- **`devid uninstall`:** quits and deletes the app.
- **The window:** a banner when the app and the daemon have different versions.
- **D-032:** the source stays private (the owner decided against making it public under MIT), and the landing page links to `devid-binaries`. The release notes carry D-010's notice, and the `devid-binaries` README gains "Install Devid" (hadziqmtqn/devid-binaries#9, merged after this release).
- **Version:** 0.7.0.

## Verification

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` | pass |
| `place_app` unit test (real `ditto` zips) | install; an update replaces the bundle and leaves no staging folders; a wrong checksum and a zip without the binary are refused, and the app that was there stays |
| Local bundle | `codesign -dv`: `dev.devid.app`, ad hoc. `Info.plist` passes `plutil -lint`. 8.6 MB, 2.0 MB zipped. |
| Production-mode web view (temporary log line, removed) | `status` and `sites` reach the daemon |
| Launch paths | `open` (LaunchServices): a foreground app named "Devid" with its window, polling every 2 s. `--background`: a UIElement with no window; reopened, it becomes foreground with its window. |
| Window "off screen" while testing | only cmux had an on-screen window then (the owner was on another Space or in full screen), so every app's new window counted as off screen. It was not a Devid bug. |
| Version comparison | 0.6.0 < 0.7.0, 0.6.10 > 0.6.9, equal versions are equal |

## Follow-ups

- **After the release (v0.7.0):** run `devid self-update`, then `devid setup --gui`. The owner checks:
  - Devid.app opens from Spotlight or Finder without a Gatekeeper prompt;
  - the name "Devid" shows in the menu bar and Cmd+Tab;
  - the menu-bar icon keeps its place after the restart.
- **G5:** docs and the release.
