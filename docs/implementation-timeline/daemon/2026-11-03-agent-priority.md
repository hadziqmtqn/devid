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
- `plutil -lint` passes on the generated plist.
- **On the owner's Mac after `devid self-update` to v0.2.3** (2026-11-03, 19:07):
  - `launchctl print gui/$UID/dev.devid.daemon` shows `spawn type = daemon (3)`, no longer `background`; the installed plist has no `ProcessType`.
  - The daemon, nginx and php-fpm run at PRI 20 (before: 4). Port 8000 (`php artisan serve`, cloudflared) kept running.
  - `erdbpro-saas.test`, measured at a load average of 3.9–4.2, like the measurements before the fix:

    | | v0.2.2 (Background) | v0.2.3 |
    |---|---|---|
    | `/login`, warm | 0.52–0.97 s | 0.21–0.24 s |
    | `/`, warm | 0.8–1.3 s | 0.32 s |
    | first request after 12 s idle (a new php-fpm worker) | 0.95 s, up to 3.2 s | 0.36 s |
    | spikes of 3–7 s | yes | none seen |

  - The first request after the daemon restart took 2.7 s: opcache starts empty, so every file is compiled once. Later requests are fast.
  - `php artisan serve` on the same app answers in 0.11–0.14 s. The remaining ~0.1 s gap is probably PRI 20 against 31. `ProcessType = Interactive` might close it; a test job with that type did not start, so it is not investigated further for now.

## Open

- php-fpm logged two workers killed with SIGKILL after 18 and 26 s (11:54 and 17:54 on 2026-11-03). Memory was not short (71 % free), Devid sets no `request_terminate_timeout`, and the unified log has no record of the sender. Not explained yet. No new SIGKILL after the v0.2.3 update; keep watching the log.
