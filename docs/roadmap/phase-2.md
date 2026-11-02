# Phase 2 Plan: daemon + CLI

> Scope: milestones and technical choices for Phase 2 (daemon, CLI, multi-PHP, `.test`, HTTPS)
> Verified against: 2026-11-02
> Status: done (M1–M7, 2026-11-02)

Phase 2 delivers a working `devid` on the owner's Mac: sites served at `*.test` over HTTPS, a PHP version per site, nginx by default (D-015), without Valet or Homebrew. Each milestone is one worktree and one PR.

## Milestones

| # | Milestone | Needs sudo | Done when |
|---|---|---|---|
| M1 | Cargo workspace; daemon on a unix socket; `devid status`; config file | no | `devid daemon run` + `devid status` work; integration test passes |
| M2 | PHP manager: install a version into `~/.devid/php/<ver>`, supervise one php-fpm master per version, base ini + `cacert.pem` | no | two versions run under the daemon; `devid php list/install/use` |
| M3 | Sites: `link`, `park`, per-site PHP; nginx config rendering and supervision on a high HTTP port (`http_port`, default 8088; 8080 was taken on this Mac) | no | erdbpro-saas served by the daemon over HTTP ✅ |
| M4 | Local CA and per-site certificates in Rust; `cacert.pem` regenerated on change; nginx HTTPS listener on a high port | no | HTTPS works; PHP site-to-site works ✅ |
| M5 | DNS responder for `*.test` in the daemon; helper (sudo, once): `/etc/resolver/test`, CA trust, ports 80/443 | **yes** | `https://erdbpro-saas.test` opens in the browser without warnings |
| M6 | `devid setup`, `doctor`, `uninstall`; LaunchAgent; `php`/`composer` shims | yes (via M5 helper) | fresh install on this Mac from `install.sh` works end to end |
| M7 | Caddy and Apache adapters (D-015 alternatives); `devid server use` | no | switching servers keeps every site working ✅ |

Migration off Valet happens after M6 and is run by the owner (see PROJECT_CONTEXT, machine state).

## Technical choices (proposed, confirm per milestone)

- **One binary.** The `cli` crate builds the `devid` binary. `devid daemon run` is the daemon entry point, which the LaunchAgent starts. `sudo devid helper …` runs the root steps. The `daemon` and `helper` crates are libraries (D-002, D-012).
- **State in `~/.devid/`:** `bin/`, `php/<ver>/`, `nginx/`, `etc/`, `ca/`, `run/`, `log/`, and `config.toml`. This replaces `~/Library/Application Support/Devid`, so the installer, the shims and the state share one root. `DEVID_HOME` overrides it for tests.
- **IPC:** a unix socket at `~/.devid/run/devid.sock`. Each request and each response is one line of JSON (`{"cmd":"status"}` → `{"ok":true,...}`). Only the user can connect (socket mode 0600).
- **Threads, not async.** A blocking socket server with one thread per connection and child process supervision with `std::process`. There is no tokio, which keeps the binary small and idle CPU at zero.
- **Few dependencies:** `serde`/`serde_json`, `toml` and `clap`. Later milestones add `rcgen` (M4) and `ratatui` (Phase 4). Each new dependency is justified in its PR.
- **DNS (M5):** the daemon answers `*.test` → 127.0.0.1 on an unprivileged port (for example 127.0.0.1:53535), and `/etc/resolver/test` points there (`nameserver 127.0.0.1`, `port 53535`). dnsmasq is not needed.
- **Ports 80/443 (M5), decided 2026-11-02 (owner): pf `rdr` on `lo0`** in the `com.apple/devid` anchor (no edit to `/etc/pf.conf`). It redirects 127.0.0.1:80/443 to nginx's 8088/8443, and a one-shot LaunchDaemon reloads it at boot. A test on this Mac showed that a non-root process may bind 80/443 on 0.0.0.0 but not on 127.0.0.1. Binding 0.0.0.0 would expose the sites to the LAN, so it was rejected.
