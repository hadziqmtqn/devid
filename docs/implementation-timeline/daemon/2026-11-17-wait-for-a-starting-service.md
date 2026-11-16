# Fix: Share and the inbox requests wait for a service that is still starting

> Date: 2026-11-17 · Branch: `fix/wait-starting-service`

## Problem

CI on `main` failed once after PR #98 was merged (run 43981283732). The failing test was `a_failed_share_reports_cloudflareds_error`, which had nothing to do with that PR. `devid share app`, sent right after `devid server install nginx`, got "nginx is not running; start it first", where the test expected cloudflared's error. Rerunning the job passed.

## Cause

- The supervisor starts each service on its own thread. Until that thread has spawned the process, the service's pid is 0 and `status` reports it as `starting`.
- `ShareStart` asked for `running` at once, so a request right after an install or `devid start` could find the web server still starting. The inbox requests (`mail_test`, `mail_list` and the rest) asked the same way about Mailpit, so `devid mail on` followed at once by `devid mail test` could say "Mail is not running".

## Fix

- `wait_running` waits while the service is still starting, for up to 5 s (polling every 50 ms), and then answers whether it runs. A service that failed, or that is not wanted, is answered at once.
- `ShareStart` and the inbox requests use it. `status` still shows the moment's state, since it only reports.
- Test `a_starting_service_is_waited_for`: starting, then running, is true; starting, then failed, is false; no service is false; still starting past the limit is false, within that limit.

## Checks

- The two share tests five times in a row, and the whole `cargo test`, on macOS.
- `cargo clippy --all-targets -D warnings`, also for `-p devid-gui`.
