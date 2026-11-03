# Project Context

> Scope: purpose, scope, constraints, plan and environment of Devid
> Verified against: 2026-11-02 (no code yet; sourced from owner decisions of 2026-11-01/02)
> Status: current (all capabilities are planned, none implemented)

## Purpose

Devid serves local PHP sites at `*.test` with HTTPS and per-site PHP versions on macOS, like Laravel Herd. It uses no Homebrew and no VM.

The driver is the owner's hardware: an Intel Mac (i5-8210Y, 2 cores/4 threads, 16 GB RAM, macOS 14.8). Low idle RAM, CPU and battery use is the primary constraint.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Lerd + Podman | Needs a Podman VM (3–4 GB reserved) |
| Yerd, EnvKit | Apple Silicon builds only |
| Laravel Herd | Owner does not want it |
| zerobrew | Only a faster Homebrew client |
| php.new | One PHP version, no `.test` domains |

## Scope

**In scope (planned):**
- PHP 8.3, 8.4 and 8.5 (D-009), multiple versions side by side, with a per-site version.
- `.test` DNS, a local CA and HTTPS.
- One global web server: nginx (default, D-015), Caddy or Apache.
- A CLI with a TUI mode, and `php`/`composer` shims. There is no GUI in v1 (D-012).

**Non-goals:**
- Databases. The owner uses DBngin for PostgreSQL and MySQL.
- Per-site web server choice.
- Arbitrary extra PHP extensions in v1 (see D-008).
- In v1: mail catcher, dump viewer, log viewer, share/tunnel (deferred, D-011).

## Phases (estimate 2–2.5 months)

| # | Phase | Status |
|---|---|---|
| 1 | Spike (~1 week): prove the risks below | done 2026-11-02 |
| 2 | daemon + CLI with nginx (D-015), multi-PHP, `.test`, HTTPS; plan in `docs/roadmap/phase-2.md` | done 2026-11-02 (M1–M7) |
| 3 | nginx and Apache adapters | done 2026-11-02 (folded into Phase 2 M7, with Caddy) |
| 4 | TUI (ratatui) on top of the daemon API; colors per D-017; plan in `docs/roadmap/phase-4.md` | done 2026-11-03 (T1–T5, released as v0.2.0) |
| 5 | Ad-hoc signing, self-update, PHP build pipeline (notarization only if public, D-010); plan in `docs/roadmap/phase-5.md` | done 2026-11-02 (R1–R5) |

## Open risks (Phase 1 must prove)

1. static-php-cli can build php-fpm 8.3, 8.4 and 8.5 for darwin-x64 with `pdo_pgsql`, `pdo_mysql`, `intl`, `gd`, `zip`, `bcmath`, `redis` and `sodium`.
2. The static php-fpm can load `xdebug.so` (and pcov) built for the same version. **Unknown.** If it fails, the fallback in D-008 applies.
3. HTTPS from PHP (`curl`, `file_get_contents`) works with a Devid-generated `cacert.pem`, both to public hosts and to other `.test` sites.
4. nginx can be built as a static binary for macOS. Apache: build it ourselves or use `/usr/sbin/httpd` (undecided).
5. erdbpro-saas (Laravel 13, PHP ^8.3, PostgreSQL) runs on Caddy and on nginx.
6. GitHub Actions still offers an Intel macOS runner. If not, build x64 locally. **Unverified as of 2026-11-02.**

### Spike results so far

**2026-11-02: PHP 8.5.12 static build on the owner's Mac** (spc 2.8.5; script `spike/php/build.sh`). Risk 1 passed for 8.5. Versions 8.1 and 8.4 are not built yet.

- **Time:** 3.5 min to download sources (656 MB), 45 min for a clean build. OpenSSL takes about 7 min, ICU about 6 min, PostgreSQL about 5 min, and PHP itself about 18 min (cli + fpm).
- **Resources:** peak RAM about 500 MB. Disk use is about 2.6 GB for `source/` and 321 MB for `buildroot/`.
- **Output:** `php` and `php-fpm` are 67 MB each (23 MB gzipped). They link only `/usr/lib` and `/System` libraries, and the minimum macOS is 12.0.
- **Extensions:** all 31 requested extensions load, including `pdo_pgsql`.
- **TLS:** HTTPS from `file_get_contents` and `curl` works with no extra config, because the OpenSSL default cert file is `/etc/ssl/cert.pem`. This does not include the Devid CA, so D-008's `cacert.pem` is still needed.
- **Homebrew contamination:** the first attempt failed at link. clang searched `/usr/local/include` and picked up Homebrew's openssl@4 headers. Fix: `export SDKROOT=$(xcrun --show-sdk-path)` and a clean rebuild. Every build must set `SDKROOT` or run on a Homebrew-free machine.
- **Compiled-in ini paths:** the spc default `/usr/local/etc/php` collides with Homebrew PHP. The build now uses `/opt/devid/etc/php`.

**2026-11-02: fresh Laravel 13.39 app on the static PHP 8.5.12** (`spike/laravel`, runtime config in `spike/runtime/`):

- **Stack:** Caddy 2.11.7 on `http://127.0.0.1:8085`, `php_fastcgi` to a unix socket, and php-fpm with `pm = ondemand`. PostgreSQL 17 (DBngin) uses database `devid_spike`, with session, cache and queue drivers set to database (same as erdbpro-saas). `composer create-project`, `migrate` and page requests all work. Requests return 200: about 0.58 s cold, about 0.03 s warm. Sessions are written to PostgreSQL.
- **erdbpro-saas:** `composer check-platform-reqs` passes with this binary.
- **Missing extension:** `pdo_sqlite` and `sqlite3` were not in the list. The default Laravel skeleton and erdbpro-saas's `phpunit.xml` (`:memory:`) need them. They are added to `build.sh` but not rebuilt yet.
- **Idle RAM:** php-fpm master 15.2 MB with 0 workers (a worker of about 29 MB exits 10 s after the last request), Caddy 30.2 MB, CPU 0%. That is about 45 MB in total.
- **For comparison:** the running Valet stack (Homebrew php-fpm 8.4 with 3 idle workers, nginx, dnsmasq) shows about 41 MB RSS. Its processes are days old, so macOS memory compression may understate it. Devid with Caddy is therefore not lighter than Valet at idle. The gain over Valet is no Homebrew and per-site PHP versions, not RAM. Against Lerd + Podman (a 3–4 GB VM) the difference is large.

**2026-11-02: rebuild with SQLite and shared Xdebug/pcov** (`--build-shared=xdebug,pcov`):

- **Rebuild:** 16 min (libraries reused). `pdo_sqlite` and `sqlite3` are now compiled in, which resolves the gap above. The binaries still link only system libraries.
- **Risk 2 passed.** `xdebug.so` (581 KB) and `pcov.so` (34 KB) load into the static `php` and `php-fpm` via `zend_extension`/`extension`. Xdebug step debugging sent a DBGp `init` packet to a test listener.
- **Caveat:** spc fetched **Xdebug 3.6.0alpha1**. Pin a stable release before shipping.
- **erdbpro-saas test suite (Pest):** static PHP 8.5.12 passes 532/532 tests (11,753 assertions) in 175 s. Homebrew PHP 8.4 passes the same 532/532 in 183 s. The run left the project's git status clean.

**2026-11-02: PHP 8.3 and 8.4, and Xdebug pinned** (`PHP_VER=<x.y> spike/php/build.sh`, output in `out/<x.y>/`). Risk 1 is now proven for all D-009 versions.

| Version | Build (libraries cached) | Binary | Xdebug 3.5.3 in cli + fpm | pcov | Links to `/usr/local` |
|---|---|---|---|---|---|
| 8.5.12 | 15.8 min | 68 MB | yes | yes | none |
| 8.4.27 | 14.2 min | 65 MB | yes | yes | none |
| 8.3.36 | 12.7 min | 62 MB | yes | yes | none |

- **Xdebug pin:** `build.sh` now downloads Xdebug 3.5.3 (stable) from PECL instead of spc's 3.6.0alpha1.
- **erdbpro-saas:** static 8.4.27 passes 532/532 tests (173 s). It cannot run on any PHP 8.3, because its locked dependencies require PHP >= 8.4.1 even though `composer.json` says `^8.3`. This is a constraint issue in that project, not a Devid issue.
- **PHP 8.3:** a fresh Laravel 13.39 app created with static 8.3 resolves to Symfony 7.4, migrates on SQLite and passes its 2/2 tests.

**2026-11-02: HTTPS through a Devid CA, and two PHP versions side by side** (`spike/multi/setup.sh` and `test.sh`, state in the gitignored `spike/runtime/multi/`). No sudo and no system trust; `*.test` is pinned to 127.0.0.1 with `--resolve`, because `/etc/resolver` is not set up yet.

- **Setup:**
  - An EC P-256 root CA signs one leaf certificate per site (SAN, `serverAuth`, 825 days).
  - Caddy serves `https://blog.test:8443` through php-fpm **8.5** and `https://api.test:8443` through php-fpm **8.4**, each master with `pm = ondemand`.
- **8/8 checks pass:**
  - Both sites return 200 over HTTP/2, and `X-Powered-By` shows the expected PHP version for each site.
  - curl without the Devid CA is rejected (exit 60), so verification is real.
  - PHP 8.5 calls `api.test` over HTTPS and gets 200 from PHP 8.4, using only `curl.cainfo` from the Devid php.ini (system bundle plus the Devid CA). With `curl.cainfo` cleared the same call fails with error 60. HTTPS to public hosts still works.
  - Risk 3 is passed for PHP; browser trust needs the helper (Phase 2).
- **Idle cost with two versions:** php-fpm 8.5 master 15.4 MB, php-fpm 8.4 master 14.8 MB, Caddy 32.7 MB, about 63 MB in total with 0 workers. Each extra PHP version costs about 15 MB idle. The daemon could start a version's master only when a site on that version gets its first request; this is a Phase 2 design option, not decided.

**2026-11-02: erdbpro-saas served through Caddy, nginx and Apache** (`spike/webservers/`, state in the gitignored `spike/webservers/run/`). Risks 4, 5 and 6 are answered.

- **Setup:** one php-fpm 8.4 pool (`pm = ondemand`) behind all three servers, each with a Devid-CA certificate for `erdbpro-saas.test`: Caddy on :9443, nginx on :9444, Apache on :9445. GET requests only.
- **Identical results on all three:** `/` returns 200 (23,360 B), `/login` returns 200 (20,764 B) with the Inertia `data-page`, the Vite JS asset and `favicon.ico` return 200, an unknown path returns Laravel's 404, and every response is HTTP/2. `/.env` is never served (nginx 403 by a deny rule; Caddy and Apache return Laravel's 404).
- **nginx (risk 4):**
  - nginx 1.30.6 (stable) builds in about 2 min, statically linked with our OpenSSL 3.6.5, zlib and PCRE2. The result is 6.0 MB and links only `libSystem`.
  - Two pitfalls were found and fixed in `build-nginx.sh`:
    - When `-lssl` is not found, nginx's configure falls back to a hard-coded `-L/usr/local/lib`, which pulled in Homebrew's OpenSSL 4 even with `SDKROOT` set. The fix passes `-L<buildroot>/lib -lz` (our libcrypto needs zlib), and the script now fails the build on any `/usr/local` link.
    - The Laravel adapter needs `fastcgi_buffer_size 32k; fastcgi_buffers 16 16k;`. Without them, erdbpro's response headers (cookies, Vite preload links) give a 502 "upstream sent too big header".
- **Apache (risk 4):** macOS ships Apache 2.4.67 at `/usr/sbin/httpd` (Apple-signed) with `mod_proxy_fcgi`, `mod_ssl`, `mod_http2` and `mod_rewrite`. It runs as the user with a Devid-owned config (never `/etc/apache2`), and Laravel's `.htaccess` works. No Apache build is needed. Unknown: whether future macOS releases keep `httpd`.
- **Idle RAM of the web server alone:** Caddy 33.9 MB, nginx 7.8 MB (master + 1 worker), Apache 8.2 MB (2 processes). nginx and Apache are about 4x lighter than Caddy at idle.
- **CI (risk 6):** the GitHub-hosted image list (actions/runner-images README, 2026-11-02) offers x64 macOS as `macos-15-intel` and `macos-26-intel`; macOS 14 images are deprecated. Unverified: whether these labels count as standard (free-quota) runners or as paid larger runners.

**Phase 1 is done when** all of these work on the owner's Mac:
- ✅ (2026-11-02) `php -m` shows the full extension list for 8.3, 8.4 and 8.5.
- `xdebug.so` loads.
- ✅ (2026-11-02) HTTPS calls from PHP succeed, including site-to-site through the Devid CA.
- ✅ (2026-11-02) erdbpro-saas loads through the static php-fpm on Caddy, nginx and Apache.

## Machine state

**2026-11-02: `.test` switched from Valet to Devid** (owner ran `devid helper install`).

- Devid's daemon (home `~/.devid`) serves `~/Projects` (parked) on nginx with static PHP 8.5, the same version Valet used for every site.
- `/etc/resolver/test` points to Devid's DNS (127.0.0.1:53535). Valet's file is kept as `/etc/resolver/test.devid-backup`.
- The pf anchor `com.apple/devid` redirects 127.0.0.1:80/443 to 8088/8443, and the LaunchDaemon `dev.devid.pf` reloads it at boot.
- The Devid CA is trusted (SSL only, name-constrained to `.test`) in the System keychain. Brave shows `https://erdbpro-saas.test` as secure, issued by "Devid Local CA".
- Only `erdbpro-saas` and `phpmyadmin` still exist among Valet's 9 secured sites. The other 7 were leftover configs for deleted folders.
- The cloudflared tunnel targets `127.0.0.1:8000` and ports 3xxx, so the switch does not touch it; `:8000` was verified 200 afterwards.
- **Since M6 (2026-11-02, the owner ran `devid setup`):**
  - launchd runs the daemon from `~/.devid/bin/devid` (LaunchAgent `dev.devid.daemon`; starts at login, restarts on a crash);
  - `~/.zshrc` line 153 puts `~/.devid/bin` before Homebrew's `php@8.4`, so new terminals get Devid's `php` (8.5.12 static) and `composer` (2.11.1);
  - `devid doctor` is all ok except that Valet still runs.
- **Since Phase 5 (2026-11-02):** everything Devid runs comes from releases:
  - PHP 8.4.27 / 8.5.12 and nginx 1.30.6 from `devid-binaries` release `2026.11.02-3`, installed with `devid php install` / `devid server install`;
  - `devid` 0.1.0 itself, installed with `curl … install.sh | sh`.

  Updates are `devid self-update` and `devid update`. The local `spike/` builds are no longer used at runtime.
- Rollback: `devid uninstall` (restores Valet's resolver), then `valet start`.
- Valet (root nginx, dnsmasq, php-fpm) still runs but no longer receives traffic; the owner stops it with `valet stop` when ready.

### Before the switch (2026-11-01)

- Laravel Valet serves 9 `.test` sites. It uses Homebrew `php@8.4`, `nginx` and `dnsmasq`, which run as root LaunchDaemons. Devid conflicts with it on ports 80/443 until Valet is stopped.
- The cloudflared tunnel runs as the system daemon `com.cloudflare.cloudflared` from `/usr/local/bin/cloudflared`, which is a symlink into Homebrew. Before Homebrew is removed, replace it with the official binary at the same path. The tunnel must keep working.
- The Homebrew agent `sh.brew.cloudflared` runs no tunnel and can be removed.
- Node comes from nvm. PostgreSQL comes from DBngin (PG 17 on port 5432).
- Homebrew removal has not started. First run `brew bundle dump --file=~/Brewfile-backup`.

## Working rules

- Reply to the owner in Bahasa Indonesia.
- Commit and push only on request. Do not add a "Generated with Claude Code" line to PR or issue text.
- Ask before any installer or system-level change: sudo, LaunchDaemons, `/etc/resolver`, CA trust.
