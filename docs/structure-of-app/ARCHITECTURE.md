# Architecture

> Scope: planned components, runtime flow and file layout of Devid
> Verified against: 2026-11-02 (Phase 2 M4: local CA and HTTPS)
> Status: partial (M1–M4 implemented; the rest proposed)

## Components (Rust Cargo workspace)

Implemented in M1:
- `crates/daemon` (library `devid-daemon`): paths, `config.toml`, the JSON-lines protocol and the unix socket server.
- `crates/cli` (binary `devid`): `devid status`, `devid daemon run` and `devid daemon stop`.

Implemented in M2:
- `daemon::php`: install from a build folder into `~/.devid/php/<ver>/{bin,modules,etc}`. Devid writes `etc/php.ini` once (the owner's to edit) and rewrites `etc/php-fpm.conf` (Devid-owned).
- `daemon::supervisor`: one php-fpm master per wanted version, with restart on crash (gives up after 5 crashes in 60 s) and a stop sequence of SIGQUIT repeated every second, then SIGKILL after 10 s.
- `~/.devid/ca/cacert.pem` (system bundle for now; the Devid CA is added in M4).
- CLI: `devid php list`, `devid php install <ver> --from <dir>` and `devid php use <ver>`.

Implemented in M3:
- `daemon::sites` resolves sites from `config.toml`:
  - `parked` folders: every non-hidden subfolder with a valid DNS-label name;
  - `links`: name → folder; a link wins over a parked folder with the same name;
  - `isolated`: name → PHP version.
  The docroot is `<folder>/public` when it exists.
- `daemon::nginx` renders one `nginx.conf` for all sites:
  - listens on `127.0.0.1:<http_port>`, with a default server that answers 404;
  - includes the Laravel FastCGI buffer settings;
  - leaves out any path that cannot be quoted safely (quote, backslash, `$`, control characters) instead of injecting it.
  Every new config is checked with `nginx -t` before it replaces the old one. The error log is passed with `-e`.
- `daemon::supervisor` is generic (`Spec`) and runs php-fpm masters and nginx with the same restart and stop logic. Config changes reach a running nginx through SIGHUP.
- Every site-changing command goes through `State::mutate`: apply the new config, and save `config.toml` only if nginx accepted it; otherwise roll back.
- Running PHP masters = the default version plus every version a site uses, if installed.
- CLI: `devid sites`, `link`, `unlink`, `park`, `unpark`, `isolate <ver>`, `unisolate` and `server install nginx --from <file>`. `isolate` and `unisolate` default to the site that contains the current folder.

Implemented in M4:
- `daemon::ca` (rcgen with the ring backend):
  - The Devid Local CA is an ECDSA P-256 key created on the first daemon start: `~/.devid/ca/devid-ca.key` (0600) and `devid-ca.crt`, valid for 10 years with `pathlen=0`. **Name constraints** limit it to `<tld>` (`.test`), so even a trusted, leaked CA key cannot impersonate a real domain. A TLD change replaces the CA. The CA is rebuilt from the stored key and a fixed DN, so no X.509 parser is needed.
  - Per-site leaves in `~/.devid/certs/<host>.{crt,key}` carry a SAN, the `serverAuth` EKU and an AKI, and are valid for 397 days counted from `not_before`. They are reissued 30 days before expiry; the expiry is recorded at issue time.
  - Creating a new CA removes the old leaves.
- `~/.devid/ca/cacert.pem` = the system bundle plus the Devid CA, rewritten only when it changes. It is set as `curl.cainfo`/`openssl.cafile` in php.ini (D-008).
- nginx: every site also listens on `127.0.0.1:<https_port>` (default 8443) with `ssl` and `http2`. The HTTPS default server uses `ssl_reject_handshake`, so unknown hosts get no certificate.
- The CA is **not** trusted system-wide yet; browsers warn until the M5 helper (or the owner) adds it to the keychain.

`helper` is created in M5. The table below is the target layout.

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
- **Install:** downloaded per version on demand from GitHub Releases into `~/.devid/php/<version>/` (all state lives under `~/.devid`, see `docs/roadmap/phase-2.md`). The download is checked against a `manifest.json` (version, sha256).
- **Configuration:** php-fpm and the CLI shims always start with `PHPRC` and `PHP_INI_SCAN_DIR` pointing at that version's folder. Compiled-in ini paths are never used.
- **TLS:** the base ini sets `openssl.cafile` and `curl.cainfo` to a Devid `cacert.pem` (Mozilla bundle plus the Devid CA). The file is regenerated whenever the CA changes.
- **Xdebug and pcov:** shipped as separate `.so` files per version. They are off by default. Enabling one drops an ini into the scan dir and restarts that version's php-fpm.
- **OPcache:** on. JIT is off (`opcache.jit = disable` with `opcache.jit_buffer_size = 0`; a bare `off` parses as an empty boolean), so no JIT entitlement is needed under the hardened runtime.
- **Shims:** `php` and `composer` live in `~/.devid/bin`. A shim resolves the version from the current directory: an isolated site's version if the directory belongs to one, otherwise the global default.

## Client rule (D-012)

All behaviour lives in `daemon` and is reached through its socket API. `cli` and the TUI only render state and send commands. A future GUI must be just another client of the same API.

## Trust boundaries

- Only `helper` runs as root, and only during setup. `daemon` and `cli` run as the user.
- Downloaded PHP binaries are trusted only after the sha256 check against the manifest. All binaries are codesigned in CI.

## Unknowns

- Whether future macOS releases keep `/usr/sbin/httpd` (used for the Apache adapter; see PROJECT_CONTEXT spike results).
- The daemon socket protocol and the on-disk config format are not designed yet.
