# Decisions

> Scope: decisions that constrain Devid's design
> Verified against: 2026-11-08
> Status: current

Never delete an entry. When a decision is replaced, mark it `superseded by D-xxx`.

| ID | Date | Status | Decision |
|---|---|---|---|
| D-001 | 2026-11-01 | accepted | Build our own native app: no Homebrew, no VM |
| D-002 | 2026-11-01 | accepted (`gui` crate dropped by D-012) | Rust workspace: `daemon`, `cli`, `helper`, `gui` |
| D-003 | 2026-11-01 | superseded by D-012 | GUI: Tauri v2 + React + Tailwind + shadcn/ui, menu bar first |
| D-004 | 2026-11-01 | accepted (version range superseded by D-009) | PHP 8.1 to latest from static-php-cli builds, php-fpm with `pm = ondemand` |
| D-005 | 2026-11-01 | accepted (default changed to nginx by D-015; all three implemented in M7) | One global web server: Caddy (default), nginx, Apache |
| D-006 | 2026-11-01 | accepted | Databases are out of scope (DBngin is used) |
| D-007 | 2026-11-01 | accepted (applies to the TUI page structure, D-012) | UI follows macOS System Settings, per the approved prototype |
| D-008 | 2026-11-02 | accepted | How to ship static PHP binaries (details below) |
| D-009 | 2026-11-02 | accepted | Supported PHP versions: 8.3, 8.4, 8.5; no 8.1 or 8.2 |
| D-010 | 2026-11-02 | accepted | Personal use first; a public release needs a prominent notice |
| D-011 | 2026-11-02 | accepted | v1 scope: sites, PHP versions, HTTPS, parked folders |
| D-012 | 2026-11-02 | accepted | v1 is CLI + TUI; GUI deferred; all logic in the daemon |
| D-013 | 2026-11-02 | accepted (Windows "not planned" superseded by D-019) | v1 is macOS only; OS-specific code isolated in a `platform` module |
| D-014 | 2026-11-02 | accepted | Install with `install.sh` plus `devid setup`/`doctor`/`uninstall`; no Node |
| D-015 | 2026-11-02 | accepted | nginx is the default web server; Caddy and Apache stay selectable |
| D-016 | 2026-11-02 | accepted | Binaries are built and released from a separate public repo, `devid-binaries`; the code stays private |
| D-017 | 2026-11-02 | accepted | TUI colors: semantic, from the terminal's own 16-color palette; not the prototype's exact colors |
| D-018 | 2026-11-03 | proposed | Local HTTPS and the system pieces on other operating systems |
| D-019 | 2026-11-03 | accepted (direction, not scheduled; desktop app technology and timing set by D-023) | After v1: a desktop app and support for every OS (macOS arm64, Linux, Windows) |
| D-020 | 2026-11-03 | superseded by D-021 | Users add PHP extensions that are not bundled: prebuilt loaders such as ionCube first, PECL builds second |
| D-021 | 2026-11-03 | accepted (direction; the Phase 6 spike confirms feasibility) | Extra PHP extensions come only from a Devid catalog built from trusted sources; no user-supplied `.so`, no compiling on the user's Mac |
| D-022 | 2026-11-03 | accepted | Add mysqli, readline and soap to the bundled PHP build (CodeIgniter 4 and WordPress need mysqli) |
| D-023 | 2026-11-04 | accepted (Phase 7, after Phase 6; the G1 spike confirms idle RAM; reference and frontend source amended by D-026) | The desktop app is a Tauri v2 menu-bar app with the prototype's plain HTML, no Node; another daemon client |
| D-024 | 2026-11-04 | accepted | imagick in the catalog keeps every format and adds SVG; Devid ships its ImageMagick config |
| D-025 | 2026-11-04 | accepted | No ionCube Loader in the catalog |
| D-026 | 2026-11-05 | accepted (Phase 7) | The revised prototype is the desktop app's reference, ported to plain HTML; English UI, a 1080×680 window, the padlock-and-prompt logo |
| D-027 | 2026-11-05 | accepted (Phase 7, from the G1 spike) | When its window closes, the desktop app restarts itself in place as icon-only, so the window's memory goes back to the system |
| D-028 | 2026-11-06 | accepted (built in G3) | The daemon checks for updates every 20 minutes (configurable); the menu-bar icon and Settings show a dot when there are any |
| D-029 | 2026-11-06 | accepted | A parked site can be removed (ignored) and restored; its folder is never touched |
| D-030 | 2026-11-06 | accepted (amends D-023's "no Dock icon") | While its window is open, the desktop app is in the Dock and Cmd+Tab with the padlock-and-prompt icon; at idle it is in the menu bar only |
| D-031 | 2026-11-06 | accepted | Two login settings: "Serve sites at login" (nginx and PHP, on by default) and "Open Devid at login" (the app, off by default) |
| D-032 | 2026-11-06 | accepted | The `devid` source stays private; downloads, the desktop app included, come from `devid-binaries`, whose README and release notes carry D-010's notice |
| D-033 | 2026-11-06 | accepted (the guide's home moved by D-034) | The user guide is public, in `devid-binaries` |
| D-034 | 2026-11-06 | accepted | A website, `devid-site`, with the landing page and the docs |
| D-035 | 2026-11-08 | accepted (Phase 8) | Each site has a serving mode (`php`, `static`, `proxy`), filled in by a detected preset (Laravel, CodeIgniter, Node, Vite …); the user can change both |
| D-036 | 2026-11-08 | accepted (Phase 8) | A site can have named user commands that the daemon starts and stops, one by one or all at once; never at login, never by themselves |
| D-037 | 2026-11-08 | accepted (Phase 8; schedules D-011's deferred share/tunnel) | A site can also be served on a local port for tunnels; "Share" runs a Cloudflare quick tunnel on demand |

## D-001: native app, no Homebrew, no VM

- **Context:** a 2-core Intel Mac with 16 GB RAM; idle resources matter.
- **Rejected:** Lerd + Podman (VM uses 3–4 GB), Yerd and EnvKit (Apple Silicon only), Herd (owner preference), zerobrew, php.new.
- **Consequence:** we own PHP builds, web server binaries, DNS and the CA.

## D-004: static PHP, `pm = ondemand`

- **Context:** keep idle RAM near zero and avoid Homebrew dylib chains.
- **Consequence:** we must rebuild for every PHP patch release and every dependency CVE. 8.1 is marked EOL in the UI.

## D-005: one global web server

- **Consequence:** there is no per-site server choice. The nginx static build and the Apache source are still open risks (see PROJECT_CONTEXT).

## D-008: shipping static PHP binaries (proposed 2026-11-02, accepted 2026-11-02)

**Context:** static binaries cause seven problems:
1. Extensions are fixed at build time.
2. Xdebug needs to be loaded as a shared extension.
3. Ini paths are compiled in.
4. Statically linked OpenSSL ignores the macOS Keychain.
5. We become responsible for security rebuilds.
6. Codesigning and the hardened runtime block JIT.
7. Binaries are large (ICU data alone is about 30 MB).

**Proposed direction:**
- A fixed extension set; no pecl in v1.
- Xdebug and pcov as separate `.so` files, off by default.
- `PHPRC` and `PHP_INI_SCAN_DIR` always set by the daemon and the shims.
- A Devid `cacert.pem` (Mozilla bundle plus the Devid CA).
- A weekly CI job that rebuilds on new php.net releases and publishes `manifest.json`.
- JIT off by default.
- Per-version on-demand download with a sha256 check.

See ARCHITECTURE, "PHP runtime".

**Fallback:** if static php-fpm cannot load `xdebug.so`, switch to our own dynamic builds. The libraries are bundled in each version's folder and relinked to `@rpath`. This also enables `phpize` and pecl. Estimated cost: 1–2 extra weeks on the build pipeline.

**Accept when:** Phase 1 confirms risks 1–3 in PROJECT_CONTEXT. Met on 2026-11-02; the owner accepted the risks and the direction above. The fallback is not needed.

**Evidence so far (2026-11-02):** on the owner's Mac, PHP 8.5.12 builds static, loads shared `xdebug.so` and `pcov.so`, serves Laravel through Caddy, and passes erdbpro-saas's 532 tests. The fallback is likely unnecessary. Still open:
- ~~builds for 8.3 and 8.4~~ done 2026-11-02;
- ~~HTTPS to `.test` sites through the Devid CA~~ done 2026-11-02 for PHP clients (browser trust is a Phase 2 helper task);
- ~~pinning a stable Xdebug~~ done 2026-11-02 (3.5.3).

## D-009: supported PHP versions (accepted 2026-11-02)

- **Context:** all of the owner's Laravel projects run on PHP 8.3 or newer. PHP 8.1 is EOL. PHP 8.2 receives security fixes only until the end of 2026. Each supported version costs about 20 minutes of CI per architecture on every release.
- **Decision:** support PHP **8.3, 8.4 and 8.5**. Build and test the fully supported versions (8.5, then 8.4) first; 8.3 is security-only but still required. New sites default to the newest version. 8.1 and 8.2 are not supported.
- **Going forward:** add a new PHP minor version once static-php-cli builds it. When a version reaches end of life, mark it EOL in the UI and stop building it. This replaces the "8.1 to latest" range in D-004.

## D-010: personal use first (accepted 2026-11-02)

- **Context:** Devid is built for the owner's own Intel Mac. An Apple Developer Program membership costs USD 99/year and is only needed to notarize builds for other machines.
- **Decision:** treat Devid as a personal tool for now. There is no Apple Developer membership and no notarization; builds are ad-hoc signed.
- **Consequences:**
  - Phase 5 shrinks to ad-hoc signing and a simple self-update.
  - arm64 builds are deferred until there is a public release. The pipeline stays able to produce them.
- **If Devid goes public:** add a prominent notice in the README, on the download page and on first launch. It must say:
  - this is a personal project provided as-is, without warranty;
  - which builds are unsigned or not notarized, and how to open them;
  - the tested platforms (Intel macOS 14 at the time of writing);
  - that the helper makes system changes (`/etc/resolver`, CA trust, ports 80/443) and how to undo them.

  Choosing a license, a signing approach and an arm64 test plan are separate decisions to make at that point.

## D-011: v1 feature scope (accepted 2026-11-02)

- **In v1:** sites served at `*.test`, multiple PHP versions with a per-site version, HTTPS through the local CA, and parked folders (every subfolder of a parked folder becomes `<name>.test`, as in Valet).
- **Deferred:** mail catcher, dump viewer, log viewer, and share/tunnel. Revisit after v1 is in daily use.
- **Out of scope:** databases (D-006).

## D-012: CLI and TUI instead of a GUI for v1 (accepted 2026-11-02, supersedes D-003)

- **Context:**
  - The owner opens a tool like this briefly, well under an hour a day. A menu-bar GUI would stay resident all day; its WebView cost is estimated at 50–100 MB (not measured), more than the whole PHP + Caddy stack (about 45 MB, PROJECT_CONTEXT).
  - Devid is for personal use first (D-010).
- **Decision:**
  - v1 ships one Rust binary, `devid`. With no arguments it opens a ratatui TUI; subcommands cover every action for scripting.
  - The TUI pages follow the approved prototype: Sites, PHP, Server, Logs, Settings.
  - Logs is a simple tail of the php-fpm and Caddy error logs. A full log viewer stays deferred (D-011).
  - The `gui` crate is dropped; Tauri, React and shadcn are not used.
- **Consequences:**
  - Nothing runs in the background except the daemon and the services it supervises.
  - There is no menu-bar status icon; `devid status` and daemon-sent macOS notifications replace it.
- **GUI later:** possible without reworking the daemon, as long as all behaviour stays behind the daemon's socket API (ARCHITECTURE, "Client rule"). A GUI would then be another client. Choose the technology (Tauri, or a small native Swift menu-bar app) at that time, and measure its idle RAM first.

## D-014: installer (accepted 2026-11-02)

- **Context:** Devid is a single Rust binary. A Node-based installer would depend on the owner's nvm-managed Node, and a tool that manages the environment should not depend on it.
- **Decision:**
  - `install.sh` (repo root, POSIX `sh`, no sudo) detects OS and CPU and downloads the release asset `devid-<os>-<arch>` with `checksums.txt`. It uses `gh` while the repo is private and plain `curl` once it is public. It verifies SHA-256, installs to `~/.devid/bin/devid`, and runs `devid setup`.
  - `devid setup` is idempotent. It downloads Caddy and PHP (sha256-checked), installs the daemon as a user LaunchAgent, then lists the system changes and asks for sudo once for the helper (`/etc/resolver/test`, CA trust, ports 80/443). *(Updated 2026-11-03, v0.2.3: as built, setup installs the binary, PATH, LaunchAgent, composer and the helper first, then downloads PHP 8.5 and nginx (nginx replaced Caddy as the default in D-015). `--php <ver>` (repeatable), `--server <name>`, `--no-php` and `--no-server` change that choice; anything already installed is skipped.)*
  - `devid doctor` reports each component with a fix hint.
  - `devid uninstall` reverses every change made by `setup`.
- **Release contract:** every release publishes `devid-darwin-x86_64` (and `devid-darwin-arm64` once D-010 allows it) plus a `checksums.txt` in `shasum -a 256` format.
- **Status:** `devid setup`, `doctor` and `uninstall` are implemented (M6, 2026-11-02). Phase 5 R4 points `install.sh` at the public `devid-binaries` repo (D-016) and the rolling `devid-latest` release: plain `curl`, SHA-256-checked.

## D-013: macOS only for v1 (accepted 2026-11-02)

- **Context:** the CLI, TUI, Caddy and static-php-cli are cross-platform. DNS, CA trust, privileged ports, background services and php-fpm differ per OS; for example, Windows has no php-fpm.
- **Decision:** v1 targets macOS only. All OS-specific code (resolver, CA trust, port binding, service install, shims) lives in a `platform` module and in the `helper` crate. The rest of the daemon must not call macOS APIs directly.
- **Later:** Linux means a second `platform` implementation. Test it on GitHub Actions Ubuntu runners and a small VPS (1 vCPU / 1 GB is enough to run Devid, but not to build PHP), never in a local VM on the owner's laptop. Windows is a much larger effort and is not planned. *(Superseded by D-019: Windows is now a long-term goal.)*

## D-015: nginx as the default web server (accepted 2026-11-02)

- **Context:** the web server spike (PROJECT_CONTEXT, 2026-11-02) served erdbpro-saas identically on all three servers. At idle the web server alone uses Caddy 33.9 MB, nginx 7.8 MB and Apache 8.2 MB. Caddy's main advantage, automatic HTTPS, is unused because Devid manages its own CA and certificates.
- **Decision:**
  - `devid setup` activates **nginx** by default. It is our own static build, about 6 MB and about 2 min to build (`spike/webservers/build-nginx.sh`).
  - **Caddy** (official binary) and **Apache** (macOS `/usr/sbin/httpd`) remain selectable at any time, for example `devid server use caddy` or the TUI Server page.
  - Only one is active at a time (D-005), with the same site configuration rendered for each.
- **Consequences:**
  - The nginx adapter must include the Laravel FastCGI buffer settings (`fastcgi_buffer_size 32k; fastcgi_buffers 16 16k;`).
  - The nginx build joins the CI pipeline and is subject to the same no-`/usr/local` link check.

## D-016: binaries in a separate public repository (accepted 2026-11-02)

- **Context:**
  - Phase 5 needs CI builds and downloadable releases.
  - The `devid` repo is private: GitHub-hosted macOS minutes count 10x against a private quota (a full PHP build is roughly 500–800 quota minutes), and assets in a private release need a GitHub token to download, which the daemon should not need.
- **Options considered:**
  - make `devid` public: needs the D-010 notice and a license now;
  - keep everything private with frugal CI: quota risk, and the daemon needs a `gh` token;
  - build on the owner's Mac: heavy on the 2-core laptop, and the release step is manual.
- **Decision:** a public repository, `hadziqmtqn/devid-binaries`, holds the PHP and nginx build scripts, their CI and all releases. The private `devid` repo builds only the `devid` binary and uploads it there with a fine-grained token limited to that repository.
- **Consequences:**
  - CI for the heavy builds is free.
  - Installed Devid downloads without a token.
  - `install.sh` can use plain `curl`.
  - Anyone can see the build scripts and download the binaries, which are open-source software plus the compiled `devid`; the Devid source stays private.
  - `install.sh`'s `REPO` changes to `devid-binaries` in R4.
- **Plan:** `docs/roadmap/phase-5.md`.

## D-017: TUI colors (accepted 2026-11-02)

- **Context:** the approved prototype is a macOS System Settings–style GUI with its own colors. The v1 interface is a TUI (D-012), and terminals use many themes (dark, light, custom).
- **Options considered:**
  - monochrome: always readable, but running/failed are hard to scan;
  - the prototype's exact truecolor palette: clashes with terminal themes, low-contrast greys on dark backgrounds, and not every terminal supports truecolor;
  - **semantic colors from the terminal's palette**.
- **Decision:** use the terminal's own 16 ANSI colors, and only for meaning:
  - green = running / ok;
  - red = failed / missing;
  - yellow = warning;
  - an accent or inverse for the selected row.
  The TUI takes the prototype's **structure and flows** (sidebar with Sites, PHP, Server, Logs and Settings), not its colors.
- **Rules:**
  - every state also has a symbol and text (`● running`, `✗ failed`), so it reads without color and for color-blind users;
  - `NO_COLOR` (no-color.org) gives a monochrome UI;
  - no truecolor;
  - important text never relies on dim colors, so contrast holds on dark and light themes.

## D-018: local HTTPS and the system pieces on other operating systems (proposed 2026-11-03)

- **Context:** v1 is macOS only. The owner asked how certificates and the system setup would work for a new install and on other OSes.
- **Today (macOS, implemented):**
  - the daemon creates the local CA on first start (name-constrained to `.test`);
  - leaf certificates are issued per site and renewed 30 days before expiry (397-day lifetime);
  - `devid setup` asks for the password once, and the helper trusts the CA in the System keychain;
  - PHP and Composer use Devid's `cacert.pem` (system bundle plus the Devid CA).
  The user does nothing by hand.
- **Known gaps on macOS (to document for users):**
  - Firefox has its own store; on macOS it usually reads the keychain (`security.enterprise_roots.enabled`), not yet tested;
  - Node and Python use their own bundles and need `NODE_EXTRA_CA_CERTS` (or the Python equivalent) pointing at the Devid CA.
- **Proposal for other OSes:** the CA and certificate code (rcgen) is portable and stays as is. The helper becomes one interface with one implementation per OS, the way mkcert does it:

  | | macOS (now) | Linux | Windows |
  |---|---|---|---|
  | Trust the CA | System keychain (sudo) | `update-ca-certificates` (Debian) or `update-ca-trust` (Fedora), sudo; Chrome/Firefox use NSS (`~/.pki/nssdb`) via `certutil` | `certutil -addstore Root`; the user store only needs a confirmation dialog, no admin |
  | `.test` DNS | `/etc/resolver/test` | systemd-resolved per-domain routing; distros without resolved are the hard case | NRPT, or a hosts entry per site |
  | Ports 80/443 | pf redirect | `setcap` or `net.ipv4.ip_unprivileged_port_start` | no privilege needed |

- **Assessment:**
  - Linux is the most work: many distro variants, plus NSS.
  - Windows is easier for HTTPS, but needs its own static PHP builds and a different process model.
- **Status:** proposed, not scheduled. v1 stays macOS only.

## D-019: direction after v1 (accepted 2026-11-03, not scheduled)

- **Context:** v1 (CLI + TUI on Intel macOS) is done and in daily use (v0.2). The owner wants the longer-term plan written down, including the README roadmap.
- **Decision:** after v1, Devid aims for:
  - **a desktop app** as another client of the daemon API (D-012, "GUI later");
  - **every major OS:**
    - macOS arm64 first (D-010);
    - then Linux (D-013, D-018);
    - then Windows.
- **Supersedes:** D-013's "Windows is … not planned". Windows is a goal now, still the largest effort: it needs its own PHP builds and has no php-fpm.
- **Not decided yet:** order and dates, the GUI technology (measure idle RAM first), and the public-release items of D-010 (license, signing/notarization, notice). *(2026-11-04: D-023 sets the desktop app's technology, Tauri v2, and its timing, Phase 7 after Phase 6.)*
- **Rule that keeps this possible:** all behaviour stays in the daemon behind its socket API, and OS-specific code stays in the `platform`/`helper` layer.
- **Where it is tracked:** the README "Roadmap" section.

## D-020: user-installed PHP extensions (proposed 2026-11-03, Phase 6)

*(Superseded by D-021 on 2026-11-03: no user-supplied `.so` files and no local PECL builds. ionCube was only an example.)*

- **Context:** D-008 ships a fixed extension set with "no pecl in v1"; only Xdebug and pcov are separate `.so` files. The owner needs extensions that are not bundled, first of all the **ionCube Loader**, which is closed source and only ships as prebuilt `.so` files per PHP version and platform.
- **Proposal:**
  1. **Prebuilt extensions first:** `devid php ext add <ver> <file.so>` installs a `.so` the user downloaded (ionCube, or any vendor loader). Devid checks it loads (`php -m`), writes `conf.d`, restarts that php-fpm, and rolls back if PHP fails to start.
  2. **PECL builds second:** `devid php ext install <ver> <name>` builds from PECL with a per-version dev kit (headers, `phpize`, `php-config`) published next to each PHP build. It needs the Xcode Command Line Tools; external C libraries are the user's responsibility.
- **Supersedes, if accepted:** D-008's "no pecl in v1" for user-added extensions. The bundled set stays fixed and is still the only thing Devid builds and updates.
- **Not decided yet:** whether Devid downloads ionCube itself (license and redistribution terms) or the user supplies the file; whether a small catalog of prebuilt optional extensions is worth the CI cost.
- **Accept when:** the Phase 6 spike loads ionCube into Devid's static PHP on the owner's Mac (see `docs/roadmap/phase-6.md`).

## D-021: extra PHP extensions from a trusted catalog (accepted 2026-11-03, Phase 6)

- **Context:** users need PHP extensions that Devid does not bundle (D-008). D-020 proposed installing a user-supplied `.so` and building from PECL on the user's Mac. The owner wants every extension to come from a safe, verified source instead, and ionCube was only an example.
- **Decision:**
  - `devid php ext install <ver> <name>` installs only extensions listed in Devid's **extension catalog**: an `extensions` section in `manifest.json` (D-016) with the name, PHP version, URL, SHA-256, `extension` or `zend_extension`, source and license of each `.so`.
  - CI in `devid-binaries` builds each catalog extension from its official source (PECL, php-src, or the project's own release), pinned by checksum, for every PHP version it supports, and publishes it with the PHP builds. A vendor's closed-source loader can be listed only if its license allows Devid to mirror it or to download it from the vendor's official URL with a pinned checksum.
  - Devid verifies the SHA-256, checks that the extension loads, and rolls back if php-fpm fails to start. A PHP update brings matching extension builds in the same release.
  - There is no `ext add <file.so>` and no local compiling, so users need no Xcode Command Line Tools. An extension that is not in the catalog is requested and added to the catalog.
- **Where users download from (clarified 2026-11-03):**
  - **Open-source extensions:** CI downloads the source from the official provider; users download the built `.so` only from the `devid-binaries` release. Devid never contacts PECL, php-src or a project's site itself.
  - **Closed-source vendor loaders** (ionCube, SourceGuardian and similar): the vendor builds the binary, so the vendor's license decides, per vendor, in this order:
    1. **Mirroring allowed:** CI downloads the vendor's official package, checks that it loads into each PHP build, and republishes it in `devid-binaries` with the vendor's license text. Users download from `devid-binaries`, as for any other extension.
    2. **Only downloads from the vendor allowed:** the manifest entry says `source: vendor` and holds the vendor's official HTTPS URL, a pinned SHA-256 of that file and the path of the right `.so` inside it per PHP version. Devid downloads **directly from the vendor**, only when the user runs `install` or `update`, never in the background. If the file no longer matches the pinned SHA-256 (the vendor replaced it), Devid refuses to install it until the catalog is updated. CI's scheduled upstream check notices a changed vendor file, tests the new one and updates the pin.
    3. **Neither allowed:** the loader is not in the catalog; the README lists it with the reason.
  - **For every vendor entry:** the manifest records the vendor, the license URL, which of the cases above applies and the date the license was checked. Before the first install, Devid shows the vendor and the license link and asks the user to accept (a prompt, or `--accept-license` in scripts). For case 2 it also says that the file comes from the vendor's server.
  - **Never:** a URL from the user or from anywhere but the manifest, a plain-HTTP download, or a file without a pinned SHA-256. The manifest itself comes only from `devid-binaries`.
- **What the catalog holds (clarified 2026-11-03):**
  - Curated, not every extension: Packagist lists 356 PIE extensions, many experimental. An entry needs a stable, maintained release for PHP 8.3–8.5, NTS and macOS, open-source dependencies that can be built in, passing CI load and smoke tests, and a real need.
  - **Priority:** what Laravel, CodeIgniter, Symfony and similar frameworks need, with Laravel Sail's runtime images as the reference set. First: imagick, igbinary, msgpack, memcached, mongodb, swoole, apcu, ldap. The full list is in `docs/roadmap/phase-6.md`.
  - **Source index:** PIE and Packagist first (PECL is deprecated), PECL only for extensions not on PIE yet, php-src for php-src extensions.
  - **Adding entries later** is a dedicated GitHub Actions task in `devid-binaries` (Phase 6, E6): a reviewed recipe goes in, CI builds and tests it for every PHP version and publishes it. Discovery can be automated; adding an entry never is.
- **Supersedes:** D-020. D-008's "fixed extension set" still holds for the bundled set; the catalog is a second, opt-in set.
- **User documentation:** the README lists which kinds of extensions can be added and, for those that cannot, why (closed source without a redistribution license, a proprietary system library, thread-safe PHP only, no PHP 8.3+ support, Windows only, and so on). The draft list is in `docs/roadmap/phase-6.md`.
- **Accept when:** done; the owner accepted the direction on 2026-11-03. The E1 spike decides which extensions are feasible as shared builds against Devid's static PHP.

## D-022: mysqli, readline and soap in the bundled PHP build (proposed and accepted 2026-11-03)

- **Context:** while ranking catalog extensions by framework needs (D-021), three php-src extensions turned out to be missing from Devid's PHP build itself:
  - **mysqli:** CodeIgniter 4's only MySQL driver and WordPress need it; `pdo_mysql` and mysqlnd are already built in.
  - **readline:** `php artisan tinker` (PsySH) and `php -a` lose history and line editing without it.
  - **soap:** used by SOAP clients (payment, government and banking APIs).
  Laravel Sail's PHP 8.4/8.5 images install all three.
- **Proposal:** add `mysqli`, `readline` and `soap` to the `EXT` list of the PHP build in `devid-binaries`. They need no new library (mysqlnd, libedit and libxml2 are already in the build), so the cost is a slightly larger binary.
- **Why bundled, not catalog:** they are small, common, and part of php-src; a catalog download for them would add steps for a basic need.
- **Amends:** D-008's fixed extension set (still fixed, three entries larger).
- **Accepted:** by the owner on 2026-11-03. Implemented in `devid-binaries` (`scripts/build-php.sh`: `EXT`, plus the three in the required-extension check).
- **Rollout:** `devid update` compares PHP version numbers, so a rebuild of the same PHP version is not offered as an update. Installed versions get the new build with `devid php install <ver>` (it re-downloads and keeps `php.ini`), or with the next PHP patch release. Offering same-version rebuilds through `devid update` needs a build marker in `manifest.json`; that is open work, also needed for catalog extensions (Phase 6).
- **Shipped:** 2026-11-03 in the `devid-binaries` release `2026.11.03-4` (PR #6), for PHP 8.3.36, 8.4.27 and 8.5.12. Each tarball matched its `manifest.json` checksum, listed mysqli, mysqlnd, readline and soap in `php -m` and `php-fpm -m`, and linked only system libraries. The README lists them (Devid PR #37). The owner reinstalled 8.4 and 8.5 with `devid php install` on 2026-11-04; `php.ini` was kept.

## D-023: the desktop app is a Tauri v2 menu-bar app with the prototype's HTML (accepted 2026-11-04, Phase 7)

- **Context:** D-019 plans a desktop app after v1 but leaves the technology open. The owner weighed three options on 2026-11-04:
  - **Swift native** (SwiftUI `MenuBarExtra`):
    - likely the leanest;
    - builds with the Command Line Tools alone (Swift 6.0.3 and SDK 15.2 on the owner's Mac, no Xcode);
    - macOS only, and the daemon protocol would have to be rewritten in Swift.
  - **A local web app served by the daemon** at `http://localhost` (pf already sends `127.0.0.1:80` to the web server):
    - costs nothing at idle and opens in any browser;
    - has no menu-bar icon;
    - every page the browser loads could send requests to it, so it needs Host and Origin checks and a login token.
  - **Tauri v2:** chosen.
- **Decision:**
  - **Where:** the desktop app is a Tauri v2 app in a new workspace crate, `crates/gui` (D-002's `gui` crate returns).
  - **Frontend:** the approved prototype's plain HTML, CSS and JS (`devid-prototype/index.html` is one file with no framework or CDN), wired to real data. There is no Node, npm, bundler, React, Tailwind or shadcn, unlike D-003.
  - **It is another daemon client (D-012 client rule):**
    - Its Rust side talks to `~/.devid/run/devid.sock` with the same `Request` types as the CLI.
    - The web view reaches the daemon only through a fixed set of Tauri commands, never the socket or the network directly.
    - Devid opens no new network port.
  - **Idle:** only the menu-bar icon stays resident. The window and its web view are created when the user opens them and destroyed when the window closes.
  - **Look:** the prototype's look (macOS System Settings, D-007). D-017's terminal palette applies to the TUI only.
- **Why Tauri:**
  - the same language and types as the daemon;
  - the prototype is reused as it is;
  - one code base for macOS, Linux and Windows (D-019);
  - the system web view (WKWebView on macOS) instead of a bundled Chromium (Electron stays rejected).
- **Risk:** idle RAM, D-012's reason for deferring a GUI, is not measured yet. Phase 7 starts with a spike (G1) that measures it against limits set in advance (`docs/roadmap/phase-7.md`).
  - If it fails, the fallback is no resident menu-bar icon: the app runs only while its window is open, like the TUI.
  - The fallback would be recorded as a new decision.
- **When:** Phase 7, after Phase 6 is released (E5). E6 (catalog workflow in `devid-binaries`) does not block it. Two reasons:
  - Phase 6 adds daemon requests (extensions) that the GUI's PHP page should show from the start.
  - Phase 6 meets a more immediate need: frameworks that need extensions Devid does not bundle.
- **Amends:**
  - D-012: a GUI now exists, along the "GUI later" path it describes.
  - D-019: the desktop app's technology and timing are decided.
  - D-003 stays superseded. Only its "Tauri v2, menu bar first, no Electron" returns.
- **Not in Phase 7:** arm64, Linux and Windows builds (D-010, D-019), and notarization.

## D-024: imagick in the catalog keeps every format and adds SVG; Devid ships its ImageMagick config (accepted 2026-11-04, Phase 6)

- **Context:** the E1 spike (`docs/roadmap/phase-6.md`, "E1 results") built imagick 3.8.1 against ImageMagick 7.1.2-35 from static-php-cli. spc's recipe builds ImageMagick `--without-xml`, so SVG could not be read. ImageMagick also looks for its config files (`policy.xml`, `type.xml`) in the build root, which does not exist on a user's Mac, so text without an explicit font file fails.
- **Decision:**
  - **Formats:** keep spc's full format set: JPEG, PNG, GIF, WebP, AVIF, JPEG XL, TIFF, and HEIC for reading. AVIF, JPEG XL and HEIC (libaom, libjxl, libheif) are most of the size and the library build time (21 MB and 29 min on the owner's Mac), but phones send HEIC and AVIF is common on the web.
  - **SVG:** build ImageMagick with libxml2 (`--with-xml`), which is already in the PHP build root, so SVG is read through ImageMagick's internal renderer (MSVG). The owner rates SVG as important. Cost: imagick.so grows from 21 MB to 24 MB.
  - **Config files:** the catalog entry ships ImageMagick's own default `policy.xml` (its "open" policy, the same as having none) and a `type.xml` that lists fonts that come with macOS (`/System/Library/Fonts`, `/System/Library/Fonts/Supplemental`), so text and SVG `<text>` work with the default font. ImageMagick's own `type-apple.xml` is out of date: only 60 of its 193 font paths exist on macOS 14.
- **Left out:** writing HEIC (needs the x265 encoder, which is GPL) and reading PDF (needs Ghostscript, AGPL); both stay unavailable.
- **Open for E2 and E3:**
  - **Gradients:** in the spike, SVG gradients (linear and radial) render black, also as MVG and with ImageMagick's own `magick` command, so it is not imagick. The draw log shows the gradient pattern is never drawn; the cause is not found. E2 must find it (another ImageMagick release, a fix, or an upstream report) before SVG counts as supported. If MSVG cannot do it, librsvg is the fallback, at the cost of cairo, pango and glib in the build.
    - **Update 2026-11-04:** the cause is found and patched (`spike/ext/imagemagick-svg-url-fill.patch`; details in `docs/roadmap/phase-6.md`, "SVG gradients"). Gradients in user-space or percent coordinates, patterns and gradient strokes now work. Gradients in `objectBoundingBox` units (SVG's default) are not implemented in ImageMagick's renderer and stay a documented limit. The owner chose this over implementing them or adding librsvg, since no current project needs SVG. Revisit when one does. Reported upstream as ImageMagick/ImageMagick#9093; the patch goes when a release fixes it.
  - **How ImageMagick finds the files:** proposal: Devid sets `MAGICK_CONFIGURE_PATH` for php-fpm and the CLI, as it sets `PHPRC` for `php.ini`. The spike confirmed that this variable makes ImageMagick read the shipped `type.xml`.
- **Accepted:** by the owner on 2026-11-04.

## D-025: no ionCube Loader in the catalog (accepted 2026-11-04, Phase 6)

- **Context:** the E1 spike checked one closed-source loader (`docs/roadmap/phase-6.md`, "Vendor loader: ionCube"). Its license allows mirroring (D-021 case 1) and loader 14.0.0 loads into Devid's PHP 8.3. But ionCube no longer maintains its Intel Mac loaders: they stop at PHP 8.3, and the 8.4 and 8.5 loaders exist for Apple silicon only. D-021 asks for a maintained release for PHP 8.3–8.5.
- **Decision:** the catalog does not offer the ionCube Loader. The owner has not used it on this Mac and expects to need it rarely, if ever.
- **Consequences:**
  - The README's "cannot be added" list names ionCube on Intel Macs, with the reason.
  - E3 builds no vendor-entry support (license prompt, `--accept-license`, `source: vendor` downloads) until a vendor loader is accepted. D-021's rules for vendor loaders stay as written for that case.
- **Revisit:** if a user asks for it, or when Devid supports Apple silicon (D-019), where ionCube still ships PHP 8.4 and 8.5 loaders.
- **Accepted:** by the owner on 2026-11-04.

## D-026: the revised prototype is the desktop app's reference, ported to plain HTML, with the padlock-and-prompt logo (accepted 2026-11-05, Phase 7)

- **Context:** D-023 names `devid-prototype/index.html` (2026-11-01) as the frontend to reuse. On 2026-11-05 the owner redesigned it in Claude Design and revised it over three review rounds: English text, a fixed window size, one confirmation pattern, and real Devid data (catalog sizes, versions, ports, paths). The owner also chose a logo on a separate canvas: concept C, a padlock whose body holds a `>_` prompt, for local HTTPS and the CLI.
- **Decision:**
  - **Reference:** the "Devid — macOS style" board of the Design artifact <https://claude.ai/code/artifact/d60fcdfc-9d87-4c41-9329-01ce1d0acccf> (version `1793875833-a8f0`). Its source files are copied to `devid-prototype/redesign/` (`Main.dc.html`, sha256 `cdd1f0e95d61…`).
    - Only the board's `mac` variant counts.
    - The canvas's "Prototype lama" page, the old `index.html` and the earlier export `Devid Redesign.html` are history.
  - **Frontend source:** the new prototype is a Claude Design file, not plain HTML. It uses `{{…}}` bindings, `<sc-if>` and `<sc-for>` tags and a `DCLogic` class, and Claude Design's own runtime renders it.
    - The app does not ship that runtime.
    - It ports the markup and the `variant-mac` CSS to plain HTML, CSS and JS, with small render functions in place of the template tags.
    - There is still no framework, Node or bundler (D-023).
  - **Language:** all UI text is English.
  - **Messages:** results and errors use the wording the CLI and TUI already print, in sentence case, and say only what the daemon reported. Where the prototype has its own text, the CLI's wording wins.
    - Example: restarting PHP 8.5's php-fpm shows "Restarted php-8.5", as `devid restart php-8.5` prints `restarted php-8.5`.
    - The prototype's "php-fpm 8.5 di-restart / Berjalan normal." is dropped, because the daemon does not report "running normally".
  - **Window:**
    - It opens at 1080×680, clamped to the screen's visible area (without the menu bar and Dock).
    - The minimum size is 760×520.
    - The sidebar is 200 wide, and the content column is at most 720 wide, centered.
    - There is no narrow or mobile layout.
  - **Confirmations:** a short title, one sentence on the effect, then Cancel and the action button. The action button is red when it deletes something.
    - Inside the Extensions dialog, the confirmation does not open a second dialog. It replaces the button row (install) or the extension's own row (remove).
  - **App icon:** logo C as drawn on the logo canvas's "Ukuran ikon macOS" board (<https://claude.ai/code/artifact/7f3d8221-223e-43b1-8e79-e6ab5935cf05>, version `1793874755-7971`, copied to `devid-prototype/logo/`). The plate is 824 of 1024, inset 100, corner radius 185.
    - `icon.icns` is built with macOS's own `iconutil` from one PNG per size, so the small sizes keep their own drawings:
      - 16 pt @1x: without the prompt;
      - 16 pt @2x and 32 pt @1x: thicker strokes;
      - 32 pt @2x and larger: the full icon.
    - `tauri icon` is not used, because it scales one image to every size. The canvas's `npx` note is not followed either (no Node).
  - **Menu-bar icon:** the prototype's outline padlock with the prompt, as a template image (black and transparent only), so macOS colors it for light, dark and highlighted menu bars.
    - The logo canvas's filled menu-bar version is not used. As a template its white prompt takes the body's color and disappears.
  - **macOS only:** no Windows or Linux icon files (D-023, "Not in Phase 7").
  - **Wordmark:** if the "devid" wordmark is used (the README, an About window), it is converted to SVG paths, so no font loads from the network. App text uses the system font.
- **Amends:** D-023. Its reference file changes, and "the prototype is reused as it is" becomes "ported from the prototype". Everything else in D-023 stands.
- **Accepted:** by the owner on 2026-11-05.

## D-027: the desktop app restarts itself as icon-only when its window closes (accepted 2026-11-05, Phase 7)

- **Context:** the G1 spike (`docs/roadmap/phase-7.md`, "G1 results") measured the app on the owner's Mac.
  - Icon only: 21 MB footprint and no CPU at idle, within the 30 MB limit.
  - After the window has been open once and is closed, the web view's processes exit, but the app process keeps about 20 MB more (41 to 46 MB). Most of it is graphics memory (IOKit 7.9 MB, IOAccelerator 4.0 MB, 2.0 MB other graphics), the rest malloc. It stayed for the two minutes measured. WebKit's Networking process (2.5 MB) also stays.
  - The limit was "within 5 MB of icon only". The total after closing missed it by about 22 to 27 MB.
- **Decision:**
  - When the last window closes, the app replaces itself with a fresh `devid-gui --background` (`exec`, so the pid stays the same).
  - Measured: 21 to 22 MB, 30 s and 120 s after the window closed, and no WebKit process left.
  - If the restart fails, the app keeps running as before, with that memory. Quit is not affected.
- **Safety:**
  - **Only on window close:** the restart runs only when the last window is destroyed. Tauri (tauri-runtime-wry 2.12.3) sends `ExitRequested` without an exit code only then. Quit Devid passes an exit code, and Cmd+Q, logout and shutdown end through AppKit's terminate. None of them restarts the app.
  - **The restart touches only `devid-gui`:** the daemon, php-fpm and the web server are separate processes, so sites keep running.
  - **Requests in flight:** a daemon request in flight when the window closes still completes, because the daemon runs each connection on its own thread. Only the reply is lost, and the next window shows the new state.
  - **Failure:** if `exec` fails, the app keeps running with the extra memory.
- **Cost:** the menu-bar icon disappears and comes back in about a second when the window closes. macOS should keep its position (status items are restored by order); G3 confirms this with `Devid.app`.
- **Rejected:**
  - **Keep the memory:** about 45 MB from the first time the window opens until logout.
  - **No resident icon** (the fallback in `phase-7.md`): the app runs only while its window is open. It is not needed, because icon-only passes its limit.
  - **A separate process for the window:** the same effect, with more code. The menu-bar process would have to start, track and focus a second process.
- **Amends:** the "30 s after the window closes" limit in `phase-7.md` is met by the restart, not by the process freeing memory.
- **Accepted:** by the owner on 2026-11-05, after seeing the icon blink for about a second, on the condition that the restart stays safe (see Safety).

## D-028: automatic update checks with an indicator in the menu bar (accepted 2026-11-06, Phase 7 G3)

- **Context:** updates are found only when the owner presses "Check for updates" (Settings, `devid update --check`, the TUI). The owner asked for a dot in the menu bar when Devid itself or PHP, a web server or an extension has an update.
- **Decision:**
  - **Who checks:** the daemon, every 20 minutes and a few minutes after it starts. It checks Devid's release (`devid-version.txt`) and the binaries manifest, as "Check for updates" does, and keeps the result.
    - `update_check_minutes` in `config.toml` sets the interval; 0 turns the checks off.
    - The owner chose 20 minutes and left the number open to change. Each check is two small HTTPS requests, so it adds nothing measurable at idle.
  - **Where it shows:** `status` carries the result, so the CLI, the TUI and the app read it from one place (D-012).
    - The menu-bar icon gets a dot. It stays a monochrome template image.
    - The menu starts with "Updates available (n)…", which opens Settings.
    - Settings shows a dot in the sidebar and the result in Updates without a click.
  - **Installing stays manual:** the Update button asks first, because php-fpm and the web server restart.
- **Not included:** macOS notifications. Phase 7 leaves notifications to the daemon, as a later feature.
- **When:** G3, with the menu-bar menu.
- **Accepted:** by the owner on 2026-11-06.

## D-029: removing a parked site from Devid, without touching its folder (accepted 2026-11-06)

- **Context:** a linked site can be unlinked, but a parked site is served for as long as its folder sits in a parked folder. The only way to stop serving one was to unpark the whole folder or move the project. The owner's updated prototype adds "Remove site…" for parked sites. The owner confirmed that it only removes the site from the daemon's list, never the project folder.
- **Decision:**
  - **Config:** `ignored` in `config.toml` lists the folders of removed parked sites. Site resolution skips them. A link of the same name still wins, as before.
  - **CLI:**
    - `devid ignore <name>` refuses a linked site and points to `devid unlink`.
    - `devid unignore <name>` serves a site again.
    - `devid sites` lists the ignored names.
    - The owner chose these names, after `link`/`unlink` and `park`/`unpark`.
  - **Daemon:** the `sites` reply lists the ignored sites (name and folder) for the TUI and the app.
  - **TUI:** "Remove site…" in a parked site's menu asks first. `I` picks an ignored site to serve again.
  - **App:**
    - "Remove site…" in a parked site's ⋯ menu asks with the prototype's wording, in red.
    - A filter on the Sites line switches between Served and Removed, without a section of its own; the owner chose this over a separate area. Removed sites have Restore.
  - **The folder is never touched:** nothing is deleted or moved on disk.
- **Accepted:** by the owner on 2026-11-06.

## D-030: the desktop app is in the Dock and Cmd+Tab while its window is open (accepted 2026-11-06, Phase 7)

- **Context:** D-023 and `phase-7.md` keep the app out of the Dock, so it lives in the menu bar only (an Accessory app). The window was then missing from Cmd+Tab and the Dock, and macOS did not bring it back to the front after a dialog (the folder-picker bug). The owner asked for the logo in the Dock and in Cmd+Tab, in the sizes of the logo canvas.
- **Decision:**
  - **With a window:** while the window is open, the app is a regular app (`ActivationPolicy::Regular`), in the Dock and in Cmd+Tab.
  - **At idle:** with no window, it is in the menu bar only, as before. Closing the window restarts it as icon-only (D-027), which takes it out of the Dock again.
  - **Icon:** `crates/gui/icons/icon.icns`, built with `iconutil` from one PNG per size as D-026 says:
    - 16 pt @1x without the prompt;
    - 16 pt @2x and 32 pt @1x with thicker strokes;
    - 32 pt @2x and up the full icon.
    `render.swift` draws them from the logo's geometry. Tauri sets it as the app icon for an unbundled run, and `Devid.app` (G4) carries it in its bundle.
- **Idle cost:** none. Icon-only is still an Accessory app with no window.
- **Amends:** D-023's "no Dock icon", now "no Dock icon at idle".
- **Accepted:** by the owner on 2026-11-06.

## D-031: two login settings, for the sites and for the app (accepted 2026-11-06, Phase 7 G3)

- **Context:** "launch at login" can mean two things in Devid:
  - **The sites:** nginx, php-fpm and DNS. The daemon's LaunchAgent has always started them at every login.
  - **The desktop app:** its menu-bar icon. The plan made this opt-in.

  The owner pointed out the ambiguity and asked for both to be clear.
- **Decision:** two settings, side by side under "Login" in the app's Settings:
  - **Serve sites at login:** "nginx and PHP start in the background when you log in. Off: they wait until you press Start all."
    - On by default: today's behaviour.
    - `config.toml` `autostart`; CLI `devid autostart on|off`; TUI `a` on Settings.
    - Off, the daemon still starts at login (about 2 MB, no CPU), so the CLI, TUI and app work at once. It starts with nginx and php-fpm stopped, as after `devid stop all`. This also holds after any daemon restart, such as a self-update.
  - **Open Devid at login:** "Shows the Devid icon in the menu bar. Your sites don't need it."
    - Off by default.
    - CLI `devid login-item on|off [--app <path>]`, which writes or removes the LaunchAgent `dev.devid.app`. It runs the app with `--background` in the login session.
    - The plist is written for the next login, never loaded at once, because loading it would start a second copy next to a running app.
    - The default app path is `~/Applications/Devid.app` (G4). The app's switch passes its own binary, and the page cannot name another program: the generic `daemon` command refuses `login_item`.
    - `devid uninstall` removes it.
- **Shown in:**
  - `devid status`: "at login: sites served, app does not open";
  - the TUI's Settings page;
  - the app's Settings.
- **Accepted:** by the owner on 2026-11-06, with the labels, the defaults and the CLI names.

## D-032: the source stays private; downloads come from devid-binaries (accepted 2026-11-06)

- **Context:**
  - The owner built a landing page and asked how it can link to the desktop app while `devid` is private.
  - The owner first planned to make the repository public under the MIT license, then chose to keep it private once it was clear the downloads do not need that.
- **Decision:**
  - **`devid` stays private**, as D-016 decided.
  - **The landing page links to the public `devid-binaries`:**
    - the install command (`install.sh` from `devid-latest`, then `devid setup --gui`);
    - optionally, `Devid-darwin-x86_64.zip` from `devid-latest` (G4).
    Neither needs a GitHub login.
  - **D-010's notice goes where users download:**
    - the `devid-binaries` README, which gains an "Install Devid" section and the system changes `devid setup` makes;
    - the release notes of `devid-v<version>` and `devid-latest` (`release.yml`);
    - the landing page (the owner's).
- **Not now:** the MIT license. The source is not published; a license can come with a later decision to publish it.
- **Cost:** macOS minutes in a private repository count 10x. With Devid.app, a release takes about 10–15 macOS minutes, or 100–150 of the 2,000 free monthly minutes.
- **Still to do from D-010:** the notice at first launch, a signing approach, and an arm64 test plan.
- **Accepted:** by the owner on 2026-11-06.

## D-033: the user guide is public, in devid-binaries (accepted 2026-11-06)

- **Context:**
  - Devid's usage docs (install options, sites, PHP, extensions, web servers, the desktop app, the TUI, HTTPS, updates, troubleshooting, uninstall, files) were in this repository's README, which is private (D-032).
  - The landing page's visitors and the `devid-binaries` README had only the install commands. The owner asked whether a separate docs page is needed.
- **Decision:**
  - **One user guide:** `GUIDE.md` in the public `devid-binaries`. It names no private files or decisions.
  - **No second copy:** this README keeps the overview, the install commands, the roadmap and development, and links to the guide.
  - **Links:** the `devid-binaries` README and the landing page ("Docs") point to it.
  - **Keeping it current:** a change users can see updates the guide in a `devid-binaries` pull request, merged when the release ships.
- **Not now:** a docs site (GitHub Pages or a generator). It is worth it once the guide outgrows one page, or users ask for search.
- **Accepted:** by the owner on 2026-11-06.

## D-034: a website with the landing page and the docs (accepted 2026-11-06, supersedes part of D-033)

- **Context:**
  - D-033 made `GUIDE.md` in `devid-binaries` the user guide. The owner then asked for a docs page on the website itself, in the same codebase as the landing page, with room for more features later.
  - The owner asked whether Zudoku fits. It is built for API references (OpenAPI, a playground, auth), and Devid has no public API, so its main features would go unused.
- **Decision:**
  - **Repository:** `devid-site`, public, separate from the release repositories.
  - **Stack:** Next.js with Fumadocs, exported as a static site. The search index is built at build time, so the site needs no server.
  - **Hosting:** Vercel, at `devid.bekenweb.com`.
  - **Pages:**
    - `/`: the landing page, ported from the "Devid Landing Page" design canvas, which stays the design reference;
    - `/docs`: the user docs, from `GUIDE.md` split into pages.
  - **One user guide, again:** `devid-site/content/docs` replaces `GUIDE.md` as the guide. Once the site is live, `GUIDE.md` keeps only a link to it, and the READMEs and the landing page link to `/docs`.
  - **Keeping it current:** as in D-033, but the pull request goes to `devid-site`.
- **Out of scope still:** databases (D-006). The docs promise nothing about them.
- **Accepted:** by the owner on 2026-11-06.

## D-035: app types: a serving mode per site, filled in by a detected preset (accepted 2026-11-08, Phase 8)

- **Context:**
  - Devid serves every site as PHP, with `public/` as the docroot when it exists.
  - The owner also works on Node and Vite apps (erd-builder-pro, apps on 3000, 3001 and 8080). They run outside Devid, and their folders are ignored in `~/Projects`.
  - The owner asked for detection like Vercel's framework presets (Laravel, CodeIgniter, Node.js, Vite and others) that the user can change.
- **Decision:**
  - **Mode:** each site has a serving mode: `php` (today's behaviour), `static` (files, optionally with an SPA fallback) or `proxy` (to `127.0.0.1:<upstream>`, WebSockets included).
  - **Preset:** a detected framework label that fills in the mode and its defaults. Detection only reads marker files, never runs anything, and is cached by their mtime.
  - **Override:** the user can change the preset or any field. Overrides live in `config.toml` under `[apps.<site>]`; a missing field falls back to detection.
  - **No match:** a site with no override and no match is served as today.
- **Scope change:** Devid stays a PHP-first app, but it also serves Node and static apps at `.test`. Node itself is the user's own (nvm); Devid ships no Node.
- **Plan:** `docs/roadmap/phase-8.md`.
- **Accepted:** by the owner on 2026-11-08, after the Phase 8 prototype on the Devid Redesign canvas.

## D-036: user commands run by the daemon (accepted 2026-11-08, Phase 8)

- **Context:**
  - The owner starts the same commands by hand in a terminal every day: `npm run dev:pg:local` and `npm run dev:tauri` in `~/Projects/erd-builder-pro`, and `php artisan serve` for erdbpro-saas.
  - The owner asked to enter commands in the desktop app, run several at once, and stop them.
- **Decision:**
  - **Commands:** a site can have any number of named commands. The daemon starts them in the site's folder with the user's login-shell environment, each in its own process group. Stop ends the whole group.
  - **Output** goes straight to `~/.devid/log/app-<site>-<name>.log`, so the Logs page shows it.
  - **Nothing by itself:** commands are never started at login and never restarted on a crash. Scripts found in `package.json` or `composer.json` are only suggestions.
  - **Lifetime:** a daemon stop stops them all. After a daemon crash, the next start stops the groups left behind.
- **Why not more:** automatic restart and start at login cost RAM and CPU on a 2-core Mac, and a broken build would loop. They can come later if the owner needs them.
- **Plan:** `docs/roadmap/phase-8.md`.
- **Accepted:** by the owner on 2026-11-08, after the Phase 8 prototype on the Devid Redesign canvas.

## D-037: a local port per site, and Share through a Cloudflare quick tunnel (accepted 2026-11-08, Phase 8)

- **Context:**
  - The owner's Cloudflare tunnel (`com.cloudflare.cloudflared`, routes set in the Cloudflare dashboard) targets `127.0.0.1:8000`, which is `php artisan serve`, and port 3000, which is a Node dev server.
  - D-011 deferred share/tunnel.
- **Decision:**
  - **Local port:** a site can also be served on `http://127.0.0.1:<port>`.
    - The default is no port; "Add port" picks the first free port from 8000 up, or the one the user enters.
    - A port that is busy is left out of the config and reported, so it never stops the web server.
    - With it, the tunnel reaches Devid directly, and `artisan serve` is no longer needed.
  - **Share:** `devid share <site>` runs `cloudflared tunnel --url … --http-host-header <name>.test` on demand and shows the random `trycloudflare.com` URL until it is stopped.
    - Devid uses an existing `cloudflared` binary and does not download it.
  - **Never:** Devid does not touch the user's own tunnel, its LaunchDaemon or its dashboard routes.
- **Amends:** D-011. Share/tunnel moves from deferred to Phase 8. The mail catcher stays deferred: the owner already runs Mailpit.
- **Plan:** `docs/roadmap/phase-8.md`.
- **Accepted:** by the owner on 2026-11-08, after the Phase 8 prototype on the Devid Redesign canvas.
