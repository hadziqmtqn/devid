# Devid

Devid is a local PHP development environment for macOS. Every folder you link or park is served at `https://<name>.test` with a trusted certificate. Devid runs PHP 8.3, 8.4 and 8.5 side by side, and nginx, Caddy or Apache as the web server. It needs no Homebrew, no Docker and no virtual machine. You use it from the command line or from a terminal UI (TUI).

> **Status:** v0.2, used daily on one Mac. It runs on macOS on Intel only. Apple Silicon builds come later (D-010). This repository holds the source and is private; the binaries are published in the public [`devid-binaries`](https://github.com/hadziqmtqn/devid-binaries) repository.

## Contents

- [Requirements](#requirements)
- [Install](#install)
- [First steps](#first-steps)
- [Sites](#sites)
- [PHP](#php)
- [PHP extensions](#php-extensions)
- [Web servers and services](#web-servers-and-services)
- [The TUI](#the-tui)
- [HTTPS](#https)
- [Updates](#updates)
- [Troubleshooting](#troubleshooting)
- [Uninstall](#uninstall)
- [Files](#files)
- [Roadmap](#roadmap)
- [Developing Devid](#developing-devid)

## Requirements

- macOS 12 or newer, on an Intel Mac.
- An administrator password, needed once during setup.
- Nothing else running on ports 80 and 443 for `127.0.0.1`. If Laravel Valet is installed, stop it first with `valet stop` and `valet stop dnsmasq`.

## Install

```sh
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
```

The script downloads `devid`, checks its sha256 and starts `devid setup`. The script itself never uses sudo. Setup lists every step as it runs it:

1. Copies `devid` to `~/.devid/bin`, and adds the `php` and `composer` commands there.
2. Puts `~/.devid/bin` first on your `PATH` in `~/.zshrc`.
3. Runs the Devid daemon as a LaunchAgent, so it starts at every login.
4. Downloads `composer.phar` and checks it.
5. Asks for your password **once** for the system setup:
   - `.test` DNS (`/etc/resolver/test`);
   - ports 80/443 redirected to Devid (a pf rule);
   - trust for the Devid certificate authority.
6. Downloads PHP 8.5 and nginx (both sha256-checked), unless PHP or the web server is already installed.

To see the steps without changing anything, run `devid setup --dry-run`. To install a specific version of Devid, run `DEVID_VERSION=0.4.1 sh install.sh`.

To choose PHP versions and the web server, pass options to setup. With the install script, put them after `sh -s --`:

```sh
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh -s -- --php 8.3 --server caddy
devid setup --php 8.3 --php 8.4     # several versions; the first becomes the default
devid setup --server apache         # nginx, caddy or apache
devid setup --no-php --no-server    # only the system parts; install PHP and a server later
```

You can add or switch PHP versions and web servers at any time later (see [PHP](#php) and [Web servers and services](#web-servers-and-services)).

Open a new terminal after setup so that `php` and `composer` come from Devid.

## First steps

```sh
devid park ~/Projects          # every subfolder becomes <folder>.test
devid doctor                   # check that everything works
```

Then open `https://<folder>.test` in your browser. If the folder has a `public/` subfolder (Laravel, Symfony), Devid serves that; otherwise it serves the folder itself.

## Sites

| Command | What it does |
|---|---|
| `devid park [path]` | Serves every subfolder of a folder (default: the current one) as `<subfolder>.test`. |
| `devid unpark [path]` | Stops serving a parked folder. |
| `devid link [name] [--path <dir>]` | Serves one folder (default: the current one) as `<name>.test` (default name: the folder name). |
| `devid unlink <name>` | Stops serving a linked site. |
| `devid ignore <name>` | Stops serving one site of a parked folder; its folder is not touched. `devid sites` lists ignored sites. |
| `devid unignore <name>` | Serves an ignored site again. |
| `devid sites` | Lists sites with their URL, folder and PHP version. |
| `devid isolate <version> [--site <name>]` | Pins a site (default: the one containing the current folder) to a PHP version. |
| `devid unisolate [--site <name>]` | Returns a site to the default PHP version. |

Every site gets HTTPS automatically; there is no separate `secure` step.

## PHP

```sh
devid php list                 # installed versions and whether php-fpm runs
devid php install 8.4          # add another version
devid php use 8.4              # make it the default
devid php ext 8.5 xdebug on    # turn Xdebug (or pcov) on/off for a version
devid php ext available 8.4    # extra extensions from Devid's catalog
devid php ext install 8.4 imagick mongodb   # one or more; php-fpm restarts once
devid php ext remove 8.4 imagick
devid php ext list 8.4         # bundled, Xdebug/pcov, and installed catalog extensions
```

- **The `php` command** follows the folder you are in. Inside an isolated site it uses that site's version; anywhere else it uses the default. `composer` works the same way.
- **Extensions:**
  - Every version includes:
    - bcmath, ctype, curl, dom, exif, fileinfo, gd, iconv, intl, mbstring, opcache, openssl, pcntl;
    - PDO with MySQL, PostgreSQL and SQLite, plus mysqli, pgsql and sqlite3;
    - readline (for `php -a` and `artisan tinker`), redis, soap, sockets, sodium, zip, and the XML extensions.
  - PHP installed before 2026-11-03 lacks mysqli, readline and soap. `devid update` does not offer the rebuild because the version number is the same, so run `devid php install <version>` (your php.ini is kept).
  - Xdebug and pcov are included but off by default. Xdebug starts in trigger mode on port 9003.
- **Extra extensions:** see [PHP extensions](#php-extensions).
- **php.ini:** each version has `~/.devid/php/<version>/etc/php.ini`. Devid writes it once and never overwrites it. After editing it, run `devid restart php-8.5`. The defaults are `memory_limit = 512M`, uploads up to 64 MB, errors displayed, and JIT off.
- **Processes:** php-fpm uses `pm = ondemand`, so idle versions use almost no memory.

## PHP extensions

```sh
devid php ext available 8.4                 # what the catalog offers for PHP 8.4
devid php ext install 8.4 imagick igbinary # one or more; php-fpm restarts once
devid php ext list 8.4                      # bundled, Xdebug/pcov, and installed catalog extensions
devid php ext remove 8.4 imagick
```

In the TUI: PHP page, `enter` on a version, then **Extensions…**: `space` ticks, `enter` on the Install button installs, `r` removes.

**The catalog** (PHP 8.3, 8.4 and 8.5): apcu, igbinary, imagick, ldap, memcached, mongodb, msgpack and swoole.
- Each comes only from Devid's releases. It is built in CI from its official source for that exact PHP release, and its checksum is verified before install. Nothing is compiled on your Mac.
- `install` checks that the extension loads in `php` and `php-fpm` before it restarts php-fpm. If it does not load, nothing changes and the sites keep running. Several extensions install in one go, with a single php-fpm restart.
- `devid update` updates them together with PHP. `devid doctor` reports one that no longer loads.

| Extension | Notes |
|---|---|
| apcu | Also on in the CLI (`apc.enable_cli=1`, since v0.4.1). An apcu installed with v0.3 or v0.4.0 gets the setting when installed again. |
| igbinary | Laravel's Redis and cache serializer option. PHP 8.5 gets 3.2.17RC1, the only igbinary release that supports it; 8.3 and 8.4 get the stable 3.2.16. |
| imagick | JPEG, PNG, GIF, WebP, AVIF, JPEG XL, TIFF and SVG; HEIC can be read but not written. PDF cannot be read (that needs Ghostscript). SVG gradients in `objectBoundingBox` units, SVG's default, are not drawn correctly (ImageMagick/ImageMagick#9093 and D-024). Text uses macOS fonts. |
| ldap | From PHP's own source, with OpenLDAP built in. |
| memcached | libmemcached built in. Serializers: PHP's and JSON, plus the session handler. The igbinary and msgpack serializers are left out, so memcached does not depend on loading those first. |
| mongodb | Carries its own OpenSSL. |
| msgpack | Redis serializer option. |
| swoole | For the CLI (`php artisan octane:start`). TLS, HTTP/2, brotli, async DNS and the coroutine curl hook. It does not change how php-fpm serves `.test` sites. |

TLS in mongodb and ldap trusts the macOS system certificates. For a server with a Devid `.test` certificate, pass `tlsCAFile` (mongodb) or `LDAP_OPT_X_TLS_CACERTFILE` (ldap) pointing at `~/.devid/ca/cacert.pem`.

### Which extensions can be added

| Kind | Examples | Can be added? |
|---|---|---|
| Bundled in Devid's PHP | bcmath, curl, gd, intl, mbstring, mysqli, pdo_pgsql, redis, soap, sodium, zip | Nothing to install (`devid php ext list`). |
| Shipped with Devid's PHP, off by default | Xdebug, pcov | `devid php ext <version> xdebug on` |
| Open source, no extra library | apcu, igbinary, msgpack, ds, uuid | Yes |
| Open source, with an open-source C library | imagick, memcached, mongodb, swoole, amqp, rdkafka, yaml | Yes. The library is built into the extension, so nothing else is installed. |
| In PHP's source but not in Devid's build | ldap, xsl, gmp, tidy | Yes, built from the same PHP source as the release. |

**Cannot be added, and why:**

| Reason | Examples |
|---|---|
| Needs a proprietary client library | oci8 / pdo_oci (Oracle Instant Client), sqlsrv / pdo_sqlsrv (Microsoft ODBC Driver) |
| Needs thread-safe PHP; Devid's PHP is non-thread-safe, as php-fpm uses it | parallel |
| The vendor no longer builds it for Intel Macs | ionCube Loader: its Intel loaders stop at PHP 8.3 (D-025) |
| Closed source without a license to redistribute | vendor loaders whose license allows neither mirroring nor downloads from the vendor |
| Windows only, or no support for PHP 8.3 and newer | com_dotnet; Zend Guard Loader, pthreads |
| A closed-source agent with its own background service | New Relic, Datadog, Blackfire (not planned) |

**Asking for another extension:** open an issue on `devid-binaries`. Each catalog extension is a reviewed recipe (`extensions/<name>.sh`): the official source pinned by checksum, its build options and a smoke test. The pull request builds and tests it for every PHP version, and a merge publishes it in a new release, with no Devid update needed. A weekly check lists newer releases of catalog extensions and popular PIE extensions for review; it never adds one by itself.

## Web servers and services

One web server runs at a time. nginx is the default.

```sh
devid server list              # nginx, caddy, apache: installed, active, version
devid server install caddy     # nginx/caddy come from Devid's releases; apache uses macOS's /usr/sbin/httpd
devid server use caddy         # switch; the old server stops, the new one starts
```

Services are the web server and one php-fpm per PHP version, named `nginx`, `caddy`, `apache`, `php-8.4` and so on.

```sh
devid status                   # daemon, web server, PHP versions, ports
devid restart nginx
devid stop php-8.4             # stays stopped until `devid start php-8.4`
devid stop all                 # stop everything (a daemon restart, e.g. at login, brings it back)
devid start all
devid logs                     # list logs
devid logs php85-fpm.log -n 50 --follow
devid logs nginx-error.log --clear
```

## The TUI

Run `devid` with no arguments, or run `devid tui`. The TUI has five pages: Sites, PHP, Server, Logs and Settings. Every action in it is also a CLI command.

| Where | Keys |
|---|---|
| Everywhere | `↑ ↓` or `j k` move · `tab` switch sidebar/page · `1`–`5` jump to a page · `r` refresh · `?` help · `q` quit |
| Sites | `enter` actions (open in browser, open in Terminal, copy URL, PHP version, unlink) · `a` link a folder · `p` park a folder · `U` unpark |
| PHP | `enter` actions (extensions, make default, Xdebug, pcov, edit php.ini, restart, install) · `u` check for updates · `U` update |
| PHP → Extensions… | `space` tick catalog extensions · `tab` to the Install button · `enter` install the ticked ones (after a confirmation, with an `installing 2/3 imagick… 52%` line) · `r` remove one (after a confirmation) · `x` Xdebug · `c` pcov |
| Server | `enter` actions (use, install, restart, stop/start, view config) · `S` stop all · `A` start all |
| Logs | `f` follow · `c` clear |
| Settings | `d` health check · `u` check for updates · `U` update PHP/nginx · `D` update devid · `s` setup · `X` uninstall |

Destructive actions ask for confirmation first. Colors come from your terminal theme, and every state also has a symbol and a word. Set `NO_COLOR=1` for a monochrome UI.

## HTTPS

HTTPS needs no manual steps:

- **On first start:** the daemon creates a local certificate authority, "Devid Local CA". It is name-constrained to `.test`, so it cannot be used for any other domain.
- **During setup:** `devid setup` adds the CA to the System keychain.
- **Per site:** each site gets a certificate when it is linked or parked. Devid renews it 30 days before it expires.
- **Browsers:** browsers that use the macOS keychain (Safari, Chrome, Brave) trust these certificates.
- **Devid's PHP and Composer:** they use `~/.devid/ca/cacert.pem` (the macOS bundle plus the Devid CA), so HTTP calls from one `.test` site to another work.

Some tools use their own certificate store:

| Tool | What to do |
|---|---|
| Firefox | Usually reads the macOS keychain (`security.enterprise_roots.enabled`). If it warns, import `~/.devid/ca/devid-ca.crt` under Settings → Privacy & Security → Certificates. |
| Node.js | `export NODE_EXTRA_CA_CERTS="$HOME/.devid/ca/devid-ca.crt"` |
| Python (requests) | `export REQUESTS_CA_BUNDLE="$HOME/.devid/ca/cacert.pem"` |

## Updates

```sh
devid self-update --check      # is there a newer devid?
devid self-update              # install it (restarts the daemon)
devid update --check           # newer PHP, nginx or Caddy?
devid update                   # install them
```

Each download is checked against its sha256 before Devid uses it. In the TUI, the same actions are on the Settings page.

## Troubleshooting

Start with `devid doctor`. It checks every part of the installation and prints a fix for each problem:

- `PATH`;
- the LaunchAgent;
- the daemon, DNS, the web server and PHP;
- the system setup;
- `.test` resolving;
- the CA;
- Valet.

| Symptom | Fix |
|---|---|
| `php` is still Homebrew's or another one | Open a new terminal (or `exec zsh`). `~/.devid/bin` must come first on `PATH`. |
| "daemon is not running" | Run `devid setup` again. It is safe to repeat. Logs: `~/.devid/log/daemon.log`. |
| `.test` does not resolve, or ports 80/443 do not answer | Run `devid helper status`, then `devid helper install`. |
| A site shows the wrong PHP version | Run `devid sites` to see each site's version, then `devid isolate <version>` inside the project. |
| The certificate is not trusted | Run `devid helper install` to trust the CA again, then restart the browser. |

## Uninstall

```sh
devid uninstall --dry-run      # show what would change
devid uninstall                # undo setup; keeps ~/.devid (PHP, config, CA, certificates)
devid uninstall --purge        # also delete ~/.devid
```

Uninstall undoes the system setup (with sudo), removes the LaunchAgent and the `PATH` line, and deletes `~/.devid/bin`.

## Files

Everything lives in `~/.devid`:

| Path | Contents |
|---|---|
| `bin/` | `devid`, plus `php` and `composer` (hard links to `devid`) |
| `config.toml` | Settings: `.test` domain, web server, default PHP, ports (8088/8443 behind pf, DNS 53535), parked folders, links and isolated sites. Prefer the commands to editing it. |
| `php/<version>/` | PHP binaries, `etc/php.ini` (yours to edit), `etc/conf.d/` (switches for Xdebug, pcov and catalog extensions) |
| `php-ext/<version>/` | Catalog extensions (`devid php ext install`), one folder each |
| `nginx/`, `caddy/`, `apache/` | Web server binaries and generated configs (rewritten by Devid) |
| `ca/`, `certs/` | The local CA, the CA bundle and per-site certificates |
| `log/` | Daemon, php-fpm and web server logs |
| `run/` | The daemon socket and pid files |

## Roadmap

The direction after v1 is D-019. Each item lists the decision it comes from. The decisions are in [`docs/structure-of-app/DECISIONS.md`](docs/structure-of-app/DECISIONS.md). Nothing below v1 has a date yet.

### v1: done (v0.2)

- Sites at `*.test`: park, link, and per-site PHP versions (D-011).
- PHP 8.3, 8.4 and 8.5 as static builds, with Xdebug and pcov switches (D-008, D-009).
- One web server: nginx (default), Caddy or Apache (D-015).
- Automatic HTTPS through a local CA.
- The CLI and the TUI (D-012).
- Install, self-update and binary updates from public releases (D-014, D-016).
- `devid setup` also installs PHP and a web server, chosen with `--php` and `--server` (v0.2.3).

### Next: small v1.x improvements

- A shorter install command on its own domain (deferred, `docs/roadmap/phase-5.md`).

### Now: more PHP extensions (Phase 6)

- **Done in v0.3.0:** `devid php ext install <version> <name>` adds an extension from Devid's catalog: apcu, mongodb, ldap and imagick (D-021, `docs/roadmap/phase-6.md`). See [PHP](#php).
- **Done in v0.4.0:** the TUI's PHP page installs several catalog extensions at once and removes them one at a time; the CLI takes several names too.
- **Done 2026-11-05:** igbinary, msgpack, memcached and swoole joined the catalog. A recipe pull request now builds and tests a new extension, and its merge publishes it. See [PHP extensions](#php-extensions).

### Then: desktop app (Phase 7)

- A menu-bar app with a window, based on the approved prototype. It starts after Phase 6 is released.
- Built with Tauri v2 and plain HTML ported from the prototype, with no Node. It is another client of the daemon's API, so the daemon needs no rework.
- **Done 2026-11-05:** the first step (G1) measured it on an Intel Mac. The menu-bar icon alone uses 21 MB and no CPU, and closing the window brings it back to that (D-027, `docs/roadmap/phase-7.md`).

### v2 and later: planned, not scheduled

- **More platforms:**
  - **Apple Silicon (arm64) macOS:** the build pipeline can already produce these (D-010).
  - **Linux:** a second platform implementation for CA trust, `.test` DNS and ports 80/443 (D-013, D-018).
  - **Windows:** a goal, and the largest effort, since it needs its own PHP builds and has no php-fpm (D-018, D-019).
- **Public release (D-010):**
  - a license;
  - signed and notarized macOS builds;
  - a clear notice about the system changes Devid makes.
- **Developer tools (D-011):** a mail catcher, a dump viewer, a full log viewer, and sharing a site through a tunnel.
- **Preview mode:** run more than one web server at the same time on different ports, to compare nginx, Caddy and Apache side by side.
- **TUI extras (Phase 4):** mouse support, themes, and changing the domain and ports from the TUI.

### Out of scope

- Databases. Use a separate tool such as DBngin (D-006).
- Docker or VM-based setups.

## Developing Devid

The source is a Rust workspace:

- `crates/daemon`: the daemon library;
- `crates/cli`: the `devid` binary and the TUI.

```sh
cargo build
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on every pull request. To publish a release, push a `v*` tag. The release workflow then publishes `devid` to `devid-binaries`.

The plans and decisions are in [`docs/`](docs/README.md). Start with the project context, then read the architecture and the decisions.
