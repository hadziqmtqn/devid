# Architecture

> Scope: planned components, runtime flow and file layout of Devid
> Verified against: 2026-11-02 (no code exists; everything here is proposed)
> Status: proposed

## Components (Rust Cargo workspace)

| Crate | Runs as | Responsibility |
|---|---|---|
| `daemon` | User | Supervises one php-fpm pool per PHP version, the active web server, the `.test` DNS resolver and the local CA. Exposes a unix socket API. |
| `cli` | User | A thin client of the daemon over the unix socket. Subcommands such as `link`, `park`, `use` and `secure`. |
| `helper` | Root, one time | Writes `/etc/resolver/test`, trusts the CA, installs a LaunchDaemon that binds ports 80/443. |
| `cli` (TUI mode) | User | Running `devid` with no arguments opens a ratatui TUI with pages for Sites, PHP, Server, Logs and Settings, mapped from the prototype. Every action is also a plain subcommand. |

## Request flow

```text
browser → foo.test
  → /etc/resolver/test → Devid DNS resolver → 127.0.0.1
  → :443 (LaunchDaemon from helper) → active web server (Caddy | nginx | Apache)
  → php-fpm socket for the site's PHP version (pm = ondemand)
```

There is exactly one active web server globally.

## PHP runtime (accepted, D-008)

- **Binaries:** static php-fpm and php CLI from static-php-cli, built by us for x86_64 (arm64 deferred, D-010).
- **Install:** downloaded per version on demand from GitHub Releases into `~/Library/Application Support/Devid/php/<version>/`. The download is checked against a `manifest.json` (version, sha256).
- **Configuration:** php-fpm and the CLI shims always start with `PHPRC` and `PHP_INI_SCAN_DIR` pointing at that version's folder. Compiled-in ini paths are never used.
- **TLS:** the base ini sets `openssl.cafile` and `curl.cainfo` to a Devid `cacert.pem` (Mozilla bundle plus the Devid CA). The file is regenerated whenever the CA changes.
- **Xdebug and pcov:** shipped as separate `.so` files per version. They are off by default. Enabling one drops an ini into the scan dir and restarts that version's php-fpm.
- **OPcache:** on. JIT is off (`opcache.jit=off`), so no JIT entitlement is needed under the hardened runtime.
- **Shims:** `php` and `composer` live in `~/.devid/bin`. A shim resolves the version from the current directory: an isolated site's version if the directory belongs to one, otherwise the global default.

## Client rule (D-012)

All behaviour lives in `daemon` and is reached through its socket API. `cli` and the TUI only render state and send commands. A future GUI must be just another client of the same API.

## Trust boundaries

- Only `helper` runs as root, and only during setup. `daemon` and `cli` run as the user.
- Downloaded PHP binaries are trusted only after the sha256 check against the manifest. All binaries are codesigned in CI.

## Unknowns

- Whether future macOS releases keep `/usr/sbin/httpd` (used for the Apache adapter; see PROJECT_CONTEXT spike results).
- The daemon socket protocol and the on-disk config format are not designed yet.
