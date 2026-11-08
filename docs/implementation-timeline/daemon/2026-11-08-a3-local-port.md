# Phase 8 A3: a local port per site, for tunnels (v0.9.0)

> Date: 2026-11-08 · Repository: `devid` · Branch: `feat/a3-local-port`

## What shipped

- **D-037, local port:** a site can also be served on `http://127.0.0.1:<port>`. It is set with `devid app port [site] [PORT|off]`, and without a port it gets the first free one from 8000. With it, a tunnel can reach Devid directly, so `php artisan serve` is no longer needed.
- **Busy ports never break the web server:**
  - A connect test over 127.0.0.1 and ::1 finds a program on the port, also one listening on `*`, which a bind test misses on macOS.
  - That port is left out of the config, and `devid app show` and `sites` name the holder.
  - Asking again takes the port once it is free.
- **Refused ports:** below 1024, Devid's own ports, another site's port, and a dev server port.
- **Web servers:** nginx, Caddy and Apache add a plain-HTTP block that answers any Host. A tunnel's `X-Forwarded-*` headers are kept.
- **Apache fix:** `X-Forwarded-For` was always empty (`%{REMOTE_ADDR}s` is an SSL variable). `X-Forwarded-Host` is now the Host the visitor sent.
- **Version:** 0.9.0.

## Verification

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` | pass (74 tests) |
| `ports.rs`, renderer unit tests | a listener on `*` is in use and named; local-port blocks on all three servers; Apache forwarded headers |
| `crates/cli/tests/sites.rs` `local_port` | first free port; a busy port on `*` left out while other sites stay; taken once free; validation; off |
| Second daemon on the owner's Mac, nginx / Apache / Caddy, requests shaped like cloudflared's | PHP sees the public Host and https; proxy sites get `X-Forwarded-Host`/`-Proto`/`-For` from the tunnel; a held port is reported busy and the rest serve; the freed port is served |

## Follow-ups

- Release, then the owner's migration: stop `php artisan serve`, then `devid app port erdbpro-saas 8000`.
- A5: the port in the TUI and the desktop app.
