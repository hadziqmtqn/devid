# Fix: `.test` sites slow while the Mac is busy (LaunchAgent priority)

> Date: 2026-11-03 · Branch: `fix/agent-priority` · Ships in v0.2.3

## Problem

The owner reported `erdbpro-saas.test` as hard to reach. It answered 200, but slowly:
- static files took 0.04–0.1 s, so nginx and TLS were fine;
- every PHP page took 0.5–1 s, with spikes of 3–7 s while the 2-core Mac had a load average of 4–7.

The LaunchAgent set `ProcessType = Background`. launchd then runs the daemon and everything it starts (nginx, php-fpm) at the lowest CPU priority (`ps` PRI 4, against 31 for a terminal process) with I/O throttling, so PHP lost every time a foreground app was busy.

Measured on the owner's Mac, same app (`/login`), same PHP 8.5 binary:

| How it ran | PRI | Time to first byte |
|---|---|---|
| `php artisan serve` in a terminal | 31 | 0.11–0.17 s |
| launchd job without `ProcessType` (`launchctl submit`) | 20 | 0.21–0.29 s, one cold start of 2.1 s |
| `php -S` under `taskpolicy -b` (background) | 4 | 0.58–0.94 s, spikes of 3.4 s and 6.9 s |
| Devid's php-fpm (Background agent) | 4 | 0.52–0.97 s, cold starts of 3.2 s |

## Fix

- The LaunchAgent plist no longer sets `ProcessType`, so launchd uses its standard limits. A comment in the plist says why, and `agent_plist_runs_the_installed_binary` asserts the key stays out.
- Idle cost does not change: with no requests, the daemon, nginx and php-fpm sleep anyway.
- `devid setup` always rewrites and reloads the plist, so `devid self-update` applies the fix. The reload stops the `.test` sites for a second or two; the cloudflared tunnel (`php artisan serve` on port 8000) is not affected.

## Verification

- `cargo test`: all pass.
- After the v0.2.3 update on the owner's Mac: check that `launchctl print gui/$UID/dev.devid.daemon` no longer says `spawn type = background`, and that the `/login` timings are close to the "without `ProcessType`" row.

## Open

- php-fpm logged two workers killed with SIGKILL after 18 and 26 s (11:54 and 17:54 on 2026-11-03). Memory was not short (71 % free), Devid sets no `request_terminate_timeout`, and the unified log has no record of the sender. Not explained yet; watch the log after this fix.
