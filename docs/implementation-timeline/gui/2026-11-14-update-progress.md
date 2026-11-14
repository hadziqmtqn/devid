# Update progress: real percentages for PHP, the web server and Devid itself

> Date: 2026-11-14 · Branch: `gui/update-progress`

## Problem

"Update" and "Update Devid…" showed only "Updating…" on the button until the end, even for about 100 MB of PHP. The owner: "saat ini hanya ada loading tetapi dalam perasaan (tidak nyata terlihat)". Nothing showed what was downloading, how far it was, or which step failed.

## Change

The design is the "Settings · Update progress" board on the v0.12.x page of the Devid Redesign canvas.

### Daemon

- `update` plans every change first, then installs them one at a time. The order is the same as before: each PHP version, then its changed extensions (php-fpm restarts after them), then nginx and Caddy.
- Each change in the reply now carries its download `size`; the manifest already has it.
- `update_progress` is new (see `ARCHITECTURE.md`, Phase 5 R3). It reports the bytes of the archive on disk, "checking" once the archive is complete, then installing or restarting, and a percentage counted like an extension install's. The job stays as `done`, or `failed` with the error, until the next update starts.
- `install_php` and `install_server` call back once the download is checked and unpacked.
- `releases::Artifact` gains `size`.

### CLI

- `devid self-update` writes its step (`devid`, `install`) to `run/self-update.json`, and the new binary's setup adds `app` before it downloads Devid.app.
- While that file exists, downloads from a devid release keep their response headers in `run/self-update.headers`; their Content-Length is the size.
- Both files go when self-update returns.

### App

- **Two update items.** "Devid" with its logo, and "PHP, web server and extensions". Each has a chip (`0.12.5 → 0.13.0`, `3 updates`, `Up to date`, `Failed`) and a line on what it downloads, with its size when the release gives it.
- **While an update runs:**
  - a large percentage and a bar;
  - "Step n of N" and "MB of MB · speed · about N s left", or "Sites pause for a few seconds", or "Verifying the download";
  - a list of steps: done ✓, in progress with a spinner, waiting with its size, failed ✕, not updated;
  - the percentage also beside Settings in the sidebar.
- **The other item** stays disabled and says when it will be available.
- **Devid's bar:** fixed parts per step, following the bytes inside each download:
  - the devid download: 0–45 %;
  - install and daemon restart: 50 %;
  - the Devid.app download: 55–90 %;
  - opening the new version: 100 %.
- **A failure** names the item and the error and says what is installed and what was not updated. "Try again" picks up from the failed item. For Devid it says whether the installed version still runs.
- **After a PHP and web server update:** "Updated just now" fades after 5 s, and the line says when and what was updated.
- **The confirmations** name what downloads and its total size.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`), `cargo test` (15 test binaries) and `cargo test -p devid-gui` pass.
- **`install_and_update_from_a_release`:**
  - after an update, `update_progress` is `done` at 100 % with PHP before nginx;
  - after a tampered archive it is `failed` at the PHP item with "checksum mismatch".
- **`self_update_checks_verifies_and_hands_over`:**
  - the new binary's setup runs in the `install` step, for 9.9.9;
  - the progress files are gone afterwards.
- **Unit tests:**
  - the update job's phase turns to "checking" when the archive is complete, and it ends at 100;
  - a download's size is the Content-Length of its last response (GitHub's redirects first, `file://` too).
- **`curl -D` on GitHub's real Latest release** gave two 302s with content-length 0, then a 200 with the file's size.
- **The real `app.js` and `app.css` in headless Brave,** with a fake `invoke` that advances an update in time at 10 MB/s with the real asset sizes. Checked in light and dark:
  - idle;
  - PHP downloading at 25 %, installing ("php-fpm 8.4 restarts"), failed at 64 % on the second item, done;
  - Devid downloading, installing, downloading Devid.app, failed, opening.
- **End to end in Devid.app built from this branch, on the owner's Mac.** The setup:
  - a daemon from this branch in its own home (ports 28088/28443/28535, through a short symlink, since the scratchpad path is too long for the socket);
  - fake releases served over HTTPS on 127.0.0.1, with a self-signed certificate that curl trusts through `CURL_HOME/.curlrc`;
  - downloads limited to 6 MB/s there, so the progress shows.

  Clicked through, with screenshots of the window:
  - Check for updates found 3 updates (84.0 MB).
  - Update ran at 14 % ("13.6 MB of 84.0 MB · 6.0 MB/s · about 12 s left"), then 84 % (step 2 of 3), then "Installing · php-fpm 8.5 restarts", and ended "Up to date · Updated just now". `devid update --check` agreed.
  - A release with a wrong checksum for PHP 8.5 failed at 85 %, with the daemon's "checksum mismatch" and "PHP 8.4.29 is installed. nginx 1.30.7 was not updated. Try again picks up from PHP 8.5.14."
  - Try again, after the release was fixed, installed the remaining two.

  The owner's own daemon, PHP and sites were not touched. Everything from the test was removed afterwards.
- **Found by that test and fixed:** the Devid item said "Up to date" after only the PHP and web server check. It now says so only after devid itself was checked in the window: the daemon's check leaves devid empty also when it failed.
- **Not run for real:**
  - Devid's self-update in the app. The new binary's setup would reinstall the owner's LaunchAgent and replace the running daemon. It is covered by the integration test with a fake setup, and shows from the release after this one.
