# Decisions

> Scope: decisions that constrain Devid's design
> Verified against: 2026-11-23
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
| D-016 | 2026-11-02 | accepted ("the code stays private" superseded by D-043) | Binaries are built and released from a separate public repo, `devid-binaries`; the code stays private |
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
| D-032 | 2026-11-06 | accepted ("stays private" superseded by D-043; the downloads part stands) | The `devid` source stays private; downloads, the desktop app included, come from `devid-binaries`, whose README and release notes carry D-010's notice |
| D-033 | 2026-11-06 | accepted (the guide's home moved by D-034) | The user guide is public, in `devid-binaries` |
| D-034 | 2026-11-06 | accepted | A website, `devid-site`, with the landing page and the docs |
| D-035 | 2026-11-08 | accepted (Phase 8) | Each site has a serving mode (`php`, `static`, `proxy`), filled in by a detected preset (Laravel, CodeIgniter, Node, Vite …); the user can change both |
| D-036 | 2026-11-08 | accepted (Phase 8) | A site can have named user commands that the daemon starts and stops, one by one or all at once; never at login, never by themselves |
| D-037 | 2026-11-08 | accepted (Phase 8; schedules D-011's deferred share/tunnel) | A site can also be served on a local port for tunnels; "Share" runs a Cloudflare quick tunnel on demand |
| D-057 | 2026-11-23 | accepted 2026-11-23 (Phase 12; narrows D-044); L1 details 2026-11-23; L2 runtime 2026-11-23 | Linux comes as Ubuntu 24.04 and 26.04 LTS on x86_64, with the CLI and the TUI: systemd-resolved for `.test`, an nftables redirect for ports 80/443, the system CA store plus NSS, a systemd user unit, and glibc PHP builds so `.so` extensions load; tested on GitHub's Ubuntu runners and, for the desktop, on the owner's Xubuntu machine over SSH |
| D-056 | 2026-11-22 | accepted 2026-11-22 (Phase 11) | Vite over HTTPS for Laravel sites: a PHP site's commands get `VITE_DEV_SERVER_KEY` and `VITE_DEV_SERVER_CERT` with the site's Devid certificate when the project's `.env` has `APP_URL` on the site's host and sets neither; otherwise the clients say how to fix `APP_URL` |
| D-055 | 2026-11-22 | accepted 2026-11-22 (the owner's request) | The app's site detail puts explanations in tooltips: an (i) after each title, the notes under the cards in their headings' tooltips; what needs action stays on the page, and a status is said once |
| D-054 | 2026-11-21 | accepted 2026-11-21 (Phase 10; amends D-036, D-040); amended 2026-11-21 (O4) and 2026-11-22 (O5) | Dev servers on demand: a proxy site can name one saved command that its first request starts (the request waits for it) and that stops after the site has been idle for a set time (10 min by default); off by default, on nginx, Caddy and Apache |
| D-053 | 2026-11-20 | accepted 2026-11-20 (amends D-014) | Devid's commands on PATH for every shell: `php8.3`–`php8.5`, `nginx`, `caddy` and `mailpit` join `php` and `composer` as shims, the PATH line goes into `~/.zshenv` and `~/.zshrc`, and Settings › Terminal, `devid path on/off` and the TUI add or remove it |
| D-052 | 2026-11-20 | accepted 2026-11-20 | The target machine is the owner's Apple M1 (8 cores, 8 GB RAM, macOS 26), not the Intel Mac; RAM is the tightest budget; Intel stays supported |
| D-051 | 2026-11-16 | accepted 2026-11-17 (amends D-049) | The app's inbox shows an email's HTML through its own `mailview:` scheme: its own CSP without scripts, a sandboxed frame, and links that open in the browser |
| D-050 | 2026-11-16 | accepted (amends D-049) | New-mail notifications use the legacy NSUserNotification API, since UserNotifications refuses an ad-hoc signed app; they show the sender and the subject |
| D-049 | 2026-11-16 | accepted (Phase 9; schedules D-011's deferred mail catcher) | Mail: Devid ships the official Mailpit build and runs it when Mail is on (off by default); 0.15.0 opens Mailpit's inbox in the browser, a later 0.15.x shows the inbox in the app (both ship in 0.15.0: D-051) |
| D-048 | 2026-11-16 | accepted (amends D-027, D-039) | The desktop app restarts through LaunchServices (`open -n`), not by replacing itself with `exec` |
| D-047 | 2026-11-15 | accepted (amends D-014) | Ports 80/443 are checked live, since another app can switch pf off on loopback; `devid helper install` undoes that |
| D-046 | 2026-11-14 | accepted (completes D-044, amends D-010) | Apple silicon: every build is native on its own GitHub runner, and a release is smoke-tested with the real PHP and nginx on both arches before it is published |
| D-045 | 2026-11-14 | accepted (amends D-043) | Issues move from `devid-binaries` to `devid`, next to the code; `devid` gains a contributor guide and a security policy |
| D-044 | 2026-11-14 | accepted (amends D-019) | Platforms come one at a time: Apple silicon, then Debian-based Linux, then Windows |
| D-043 | 2026-11-13 | accepted (supersedes part of D-032) | The `devid` source is public under the GNU AGPL v3; binaries and releases stay in `devid-binaries` |
| D-042 | 2026-11-12 | accepted (amends D-035) | A new proxy site whose default dev server port another site uses gets the next free port, saved |
| D-041 | 2026-11-12 | accepted (amends D-036) | A proxy site's commands get its dev server port ("Forward to") as `PORT` |
| D-040 | 2026-11-09 | accepted (amends D-036) | Commands that a Devid-made restart (an update, `devid setup`) interrupts start again in the new daemon; never at login or after a crash |
| D-039 | 2026-11-09 | accepted | The desktop app updates devid in the background and reopens as the new version; Terminal only for a step that needs sudo |
| D-038 | 2026-11-08 | accepted (amends D-016's release layout) | The newest stable devid is GitHub's "Latest" release in `devid-binaries`; the PHP/nginx builds move to a rolling `runtime-latest` release; tags with a suffix are pre-releases |

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
- **Amended by D-038 (2026-11-08):** devid releases are now GitHub's "Latest" in `devid-binaries`, and the PHP/nginx builds are read from `runtime-latest`.
- **Amended by D-043 (2026-11-13):** the `devid` source is public under the AGPL. The binaries and releases stay in `devid-binaries`.

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
  - Firefox has its own store; on macOS it usually reads the keychain (`security.enterprise_roots.enabled`), not yet tested; *(2026-11-22: Firefox 160 on macOS 26 trusts the Devid CA with default settings, checked with a fresh profile in Phase 11.)*
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
- **Not decided yet:** order and dates, the GUI technology (measure idle RAM first), and the public-release items of D-010 (license, signing/notarization, notice). *(2026-11-04: D-023 sets the desktop app's technology, Tauri v2, and its timing, Phase 7 after Phase 6.)* *(2026-11-14: D-044 sets the platform order: Apple silicon, then Linux, then Windows.)*
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
- **Superseded in part by D-043 (2026-11-13):** the source is public under the AGPL. Downloads still come from `devid-binaries`, with D-010's notice.

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

## D-038: devid is GitHub's "Latest" release; PHP/nginx builds move to `runtime-latest` (accepted 2026-11-08)

- **Context:**
  - Devid is shared in public from v0.10.1. In `devid-binaries`, GitHub's "Latest" label sat on the PHP/nginx build release (`2026.11.05-7`), because the daemon and `check-upstream.yml` read `releases/latest/download/manifest.json`. The devid releases were published with `--latest=false`, and updates were found through the rolling `devid-latest` release.
  - A visitor saw a PHP build as the newest release, and there was no way to publish a test build: every `v*` tag replaced `devid-latest` at once.
- **Decision:**
  - **devid:** a stable `devid-v<version>` is created with `--latest`. `devid self-update`, the daemon's automatic check, the TUI and the app read `releases/latest/download/devid-version.txt` (through the API's `/releases/latest` when the download link fails).
  - **Pre-releases:** a tag with a suffix (`v0.11.0-rc.1`, with the same version in `Cargo.toml`) is published with `--prerelease`. It never becomes Latest and never touches `devid-latest`; it is installed by hand. A release counts as newer than its own pre-release.
  - **PHP, nginx and Caddy:** their release is created with `--latest=false` and copied to a rolling `runtime-latest` release, whose `manifest.json` the daemon and `check-upstream.yml` read. Artifact URLs still point at the dated release, inside the trusted prefix.
  - **Compatibility:**
    - `devid-latest` stays and is updated by every stable release: `install.sh`'s public link uses it, and devid 0.10.0 and older look for updates there.
    - Every stable devid release also carries `runtime-latest`'s `manifest.json`, and a new PHP/nginx release uploads its manifest to the current Latest devid release. devid 0.10.0 and older read the manifest from `releases/latest`.
- **Order of the switch:** `runtime-latest` exists before the first devid release with `--latest` (0.10.1), because that release copies its manifest.
- **Amends:** D-016's release layout.
- **Accepted:** by the owner on 2026-11-08.

## D-039: the desktop app updates devid in the background (accepted 2026-11-09)

- **Context:** "Update Devid…" opened Terminal and ran `devid self-update` there, because `setup` may ask for the sudo password. On the owner's Mac (2026-11-09, 0.10.1 → 0.12.0) the Terminal window stayed open at "[Process completed]", and the app window stayed on the old version until it was closed by hand. The owner asked for an update like other desktop apps.
- **Decision:**
  - The app runs `devid self-update --no-sudo` itself, with no Terminal, and the button shows "Updating Devid…".
  - When it is done, the app restarts as the new binary (same pid) with its window on Settings.
  - `--no-sudo` (also on `devid setup`) skips a system setup step that would need the password and reports it (exit code 3). Everything else installs.
  - The app then offers Terminal for `devid setup`. That is the only case left for Terminal, besides `setup` and `uninstall` started on purpose.
- **Why it is safe:** an update needs sudo only when the system setup (resolver, CA trust, port forwarding) is missing or changes. The download is still sha256-checked, and the new binary installs itself as before.
- **Accepted:** by the owner on 2026-11-09.

## D-040: commands run again after Devid restarts itself (accepted 2026-11-09, amends D-036)

- **Context:** D-036 says commands never start by themselves, and a daemon stop stops them all. An update restarts the daemon (`self-update` runs the new `devid setup`, which reloads the LaunchAgent). After updating to 0.12.2 from the app, the owner found erd-builder-pro's `start` stopped, and asked whether Devid should start again what was running.
- **Decision:**
  - Right before `devid setup` reloads the daemon, it sends `remember_running`. The running daemon writes its running commands (not shares) to `~/.devid/run/resume.json`, with the time.
  - The next daemon, after applying its config, starts those commands again in the background. Each log is headed "started again after Devid restarted for an update".
  - Then it removes the note. A note older than 10 minutes, a command that was removed and a site that is gone are skipped.
- **What stays as D-036 says:**
  - At login, after a crash, after `devid daemon stop` and at logout, no note is written, so nothing starts.
  - Shares are not started again, because their link would change without notice.
- **Why:** an update is an interruption Devid makes itself, minutes after the owner chose what to run. Login and crashes give no such signal, and on a 2-core Mac a dev server costs 0.5–1 GB.
- **Limit:** the old daemon must know `remember_running`, so this works from the update after 0.12.3 is installed.
- **Accepted:** by the owner on 2026-11-09.

## D-041: a proxy site's commands get its dev server port as PORT (accepted 2026-11-12, amends D-036)

- **Context:**
  - A proxy site forwards to its dev server port ("Forward to"), but the dev server picks its own port. The two were set in two places and could disagree.
  - Two Node sites of the same type both default to its port (Next.js and Node.js: 3000). Vite, and Next.js without a port, then move to the next free port, and the `.test` address shows the other project.
  - The owner asked how a project can avoid a port another one uses, then asked for `PORT` to come from "Forward to" automatically.
- **Decision:**
  - A command of a proxy site starts with `PORT=<Forward to>` in its environment: started by hand, with Start all, or again after an update (D-040).
  - A `PORT=` in the command line (`PORT=4000 npm run api`) still wins, because the shell sets it last.
  - Shares, and the commands of PHP and static sites, get no `PORT`.
- **What follows it:** dev servers that read `PORT`: Next.js, Express, Fastify and most Node servers. Vite ignores it (use `--port <n> --strictPort`), and a `-p` in the project's script wins over it.
- **Trade-off:** a site that runs two servers which both read `PORT` now gives both the same port; the second one sets its own in its command line.
  - The owner's commands were checked on 2026-11-12. erd-builder-pro reads `PORT || 3000` and forwards to 3000. erd-selfhost-local's `.env` has `PORT=3001`, and it forwards to 3001. The Tauri servers set `PORT=3100` themselves.
- **Limit:** a running command keeps the `PORT` it started with. After "Forward to" changes, restart it.
- **Accepted:** by the owner on 2026-11-12.

## D-042: a new proxy site gets a free dev server port (accepted 2026-11-12, amends D-035)

- **Context:**
  - Every site of a type starts with that type's port (Next.js and Node.js: 3000, Vite: 5173).
  - A second project of the same type therefore forwards to the first one's dev server, and its own fails with EADDRINUSE.
  - D-041 made commands follow "Forward to", so a different "Forward to" is now enough to keep them apart. The owner asked for Devid to pick it.
- **Decision:**
  - **Which sites:** a proxy site that Devid has nothing saved for (no `[apps.<site>]` before the change) and that has no `PORT` in `.env`.
  - **When it moves:** when another site already forwards to its default port, it gets the next port that no site uses (as upstream or local port) and no program listens on. The port is saved as its upstream, so it never moves again.
  - **Who keeps the default:** of two such sites, the older folder (by creation time, then name) keeps it, so a new project never moves an existing one.
  - **When it runs:** on every change made through Devid (park, link, `app set`, `cmd add`…) and when the daemon starts. A first `cmd add` on a new site still counts as new.
  - **Not touched:** sites with settings or commands, a port the owner typed, and `PORT` from `.env`. Two sites the owner set to the same port stay as they are.
- **Limits:**
  - A folder created while the daemon runs gets its port at the next change in Devid. Until then it is not in the web server's config either.
  - Vite ignores `PORT`. Its command still needs `--port <n> --strictPort`.
  - The owner's sites on 2026-11-12 all have settings, so none of them moves.
- **Accepted:** by the owner on 2026-11-12.

## D-043: the `devid` source is public under the GNU AGPL v3 (accepted 2026-11-13, supersedes part of D-032)

- **Context:**
  - The release workflow builds on `macos-15-intel`. In a private repository, macOS minutes count 10x against the 2,000 free monthly minutes. The 22 release runs from 2026-11-02 to 2026-11-12 took about 192 runner minutes, so about 1,900 quota minutes, and the free quota is not enough.
  - Standard GitHub-hosted runners are free in a public repository.
  - Before the switch, the history was searched for private keys and tokens; none were found. Two hostnames of the owner's own Cloudflare tunnel were in the docs. They are removed, and older commits still have them.
- **Decision:**
  - `hadziqmtqn/devid` becomes public.
  - **License:** GNU AGPL v3 only (`AGPL-3.0-only`), in `LICENSE`. Whoever distributes Devid, changed or not, must offer its source under the same license; a changed version offered to users over a network must offer its source too.
  - **Downloads do not move:** `install.sh`, the binaries, the releases and the update checks stay in `devid-binaries` (D-016, D-038). Installed copies of Devid change nothing.
  - **Workflows:** Claude Code Review skips pull requests from forks, which get no secrets. `@claude` answers only users with write access, the action's default.
- **What becomes public:** the source and its whole history (the commit author's email included), the docs, past pull requests and issues, and old Actions logs (GitHub masks secrets in them). Actions secrets stay secret.
- **Cannot be undone:** clones and forks made while the repository is public stay public.
- **Still to do from D-010:** the notice at first launch, a signing approach, and an arm64 test plan.
- **Accepted:** by the owner on 2026-11-13.

## D-044: platforms come one at a time: Apple silicon, then Linux, then Windows (accepted 2026-11-14, amends D-019)

- **Context:**
  - D-019 lists macOS arm64 first, then Linux, then Windows, but left the order open. The landing page and the README then listed Apple silicon last, after Linux and Windows (devid-site #7, devid #77).
  - Apple silicon runs the same macOS code. It needs arm64 builds of Devid, PHP, the extensions and the web servers, and tests, but no new platform layer. Today every release job runs on `macos-15-intel` and builds x86_64 only. GitHub's Apple silicon runners are free now that the repository is public (D-043).
  - Linux needs its own DNS, CA trust and port forwarding in place of `/etc/resolver`, the System keychain and pf. Windows also needs its own PHP builds and has no php-fpm.
  - The owner has an Intel Mac only, so arm64 is tested on CI runners, not by hand.
- **Decision:**
  - Apple silicon is next, in the roadmap's "Next" next to macOS 15 and later on Intel.
  - Then Linux (Debian-based), then Windows, one platform at a time. Each one starts after the previous one ships.
- **Unchanged:** no dates; OS-specific code stays in the `platform`/`helper` layer (D-019).
- **Still to do:** the arm64 test plan (D-043).
- **Where it shows:** the README "Roadmap", and the landing page's Roadmap and Requirements in `devid-site`.
- **Accepted:** by the owner on 2026-11-14.

## D-045: issues live in `devid`, next to the code (accepted 2026-11-14, amends D-043)

- **Context:**
  - Since D-043 the source is public, but bug reports and feature requests still went to `devid-binaries`, which holds only build scripts and releases. A pull request in `devid` cannot close an issue there with `Fixes #n`.
  - Both repositories had no issues yet, so nothing has to move.
- **Decision:**
  - Bug reports, feature requests and extension requests go to `hadziqmtqn/devid`. Its bug and feature forms are the ones `devid-binaries` had. Blank issues are off; the chooser also links the security form and the guide.
  - `devid-binaries` keeps its Issues tab, with blank issues off and its forms replaced by links to `devid`.
  - `devid` gets `CONTRIBUTING.md`, `SECURITY.md` and a pull request template.
  - **Security reports:** GitHub's private vulnerability reporting, turned on in `devid` on 2026-11-14. No email address is published.
  - **Contributions:** licensed as `AGPL-3.0-only`, the same as the project (inbound = outbound), with no separate agreement.
- **Not now:** a code of conduct; one line in `CONTRIBUTING.md` asks for kindness.
- **Where it shows:** the `devid` README, the `devid-binaries` README, and the guide's troubleshooting and extensions pages and the landing page footer in `devid-site`.
- **Accepted:** by the owner on 2026-11-14.

## D-046: Apple silicon builds and their test plan (accepted 2026-11-14, completes D-044, amends D-010)

- **Context:**
  - D-044 made Apple silicon the next platform and left its test plan open (D-010, D-043). The owner has an Intel Mac only, so nobody runs the arm64 builds by hand.
  - The code was mostly ready: the manifest has an `arch` slot per asset, and the daemon, `self-update` and `install.sh` already pick `arm64` on Apple silicon. The builds and the release workflows were x86_64 only, and `devid setup --gui` always fetched the Intel app.
- **Decision:**
  - **Native builds:** every asset is built on a runner of its own arch, `macos-15-intel` for x86_64 and `macos-15` for arm64, with no cross-compiling. The build checks then run each binary natively: `php -m`, Xdebug, every extension in `php` and `php-fpm` with its smoke test, `nginx -V`.
  - **devid-binaries:** `release.yml` and `catalog.yml` build each PHP version, its extensions and nginx for both arches; the Caddy mirror takes both official builds of one version. One dated release and one manifest hold both arches.
  - **devid:** `release.yml` builds and tests `devid` and Devid.app on both runners. Before anything is published, `scripts/smoke.sh` runs on each: a daemon in a throwaway `DEVID_HOME` on spare ports installs PHP 8.5 and nginx from `runtime-latest`, serves a linked site, and checks that PHP answers with the runner's own arch (`php_uname("m")`, which Rosetta would report as x86_64). A failure on either arch publishes nothing. `workflow_dispatch` with `dry_run` runs all of it without publishing.
  - **Assets:** `devid-darwin-arm64` and `Devid-darwin-arm64.zip` next to the x86_64 ones, in the same release and the same `checksums.txt`.
  - **Order:** the runtime release with arm64 comes first, since the smoke test needs it; then a devid release.
  - **Honesty:** the release notes, the README and the landing page say Apple silicon is tested on GitHub's runners only, until someone confirms it on a real Mac.
- **Not now:** a universal binary (it doubles every download), and an arm64 job in CI on every pull request (the release builds and smoke-tests both arches).
- **Accepted:** by the owner on 2026-11-14.

## D-047: ports 80/443 are checked live; `helper install` undoes another app's `skip on lo0` (accepted 2026-11-15, amends D-014)

- **Context:**
  - Devid sends 127.0.0.1:80/443 to the web server's high ports with `rdr pass on lo0` rules in the pf anchor `com.apple/devid`. The LaunchDaemon `dev.devid.pf` loads them once, at boot.
  - Yerd works the same way and reported that its redirect dies whenever another pf client sets `skip on lo0`. macOS Internet Sharing does that when OrbStack, Docker or UTM adds a VM network; on one machine it happened 16 times in 45 minutes (https://github.com/forjedio/yerd/issues/253). It came up in the competitor research (`docs/research/competitors-2026-11.md`).
  - Confirmed on the owner's Mac on 2026-11-15 (macOS 14.8.9), changing only pf options:
    - with `skip on lo0`, 127.0.0.1:80 and :443 refuse connections at once, while nginx on 8088/8443 runs;
    - reloading Devid's anchor, which `devid setup` and the LaunchDaemon do, does not help;
    - `pfctl -O -f /etc/pf.conf` (options only) clears the skip, and 80/443 answer again.
  - Nothing noticed it:
    - `doctor`, `setup` and `standard_ports` only compared the files with what Devid writes;
    - `setup` skipped its sudo step when the files matched;
    - Share, which goes through :80 since 0.12.1, failed the same way.
- **Decision:**
  - **Live check:** `webserver::answers_on(80)` asks 127.0.0.1:80 for an unknown host and expects the catch-all reply of Devid's web servers, "No Devid site for this host." Apache now sends that text too. Another server on port 80 does not pass.
  - **Where it shows:**
    - The status reply gains `standard_ports_ok`. It is null without the redirect or a running web server, and is cached for 15 s.
    - `devid status`, the TUI header and Server page, and a banner on every page of the app name the problem and the fix.
    - The menu-bar icon turns red, and its menu starts with the problem.
    - `devid doctor` runs the check every time.
  - **Repair:**
    - `devid helper install`, the fix shown everywhere, also runs `pfctl -O -f /etc/pf.conf`, but only when `lo0` is skipped.
    - `devid setup` checks before it restarts the daemon, and runs its sudo step when port 80 does not answer.
- **Not now:**
  - **Repairing on its own:** a root job woken by macOS's network-change notification could clear the skip at no idle cost. Whether that notification comes every time needs a spike on a Mac with OrbStack or Docker.
  - **Dropping pf:**
    - nginx could listen on `*:80`, which a user process may bind on macOS 14 (unlike `127.0.0.1:80`; checked with port 81), but it would then also face the LAN and the firewall prompt;
    - handing sockets over from a root helper is a larger change.

    Either would also end the iCloud Private Relay conflict that Yerd reports for any pf `rdr` rule (https://github.com/forjedio/yerd/issues/247), which is not yet checked for Devid.
- **Accepted:** by the owner on 2026-11-15.

## D-048: the desktop app restarts through LaunchServices, not with exec (accepted 2026-11-16, amends D-027 and D-039)

- **Context:**
  - The app replaces itself in place with `exec` (same pid) in four cases:
    - its window closes (D-027);
    - after an update from the app, with the window on Settings (D-039);
    - the menu-bar icon finds a new binary at the path (v0.10.1);
    - the window opens after an update (v0.10.1).
  - On 2026-11-16 the owner updated 0.13.0 to 0.14.0 from the app. The app did not come back: no window and no menu-bar icon.
    - The process ran the new binary with the same pid. It sat in AppKit's event loop with two threads, and Tauri's setup never ran because `applicationDidFinishLaunching` never came.
    - Its log showed the check-in with the process manager and LaunchServices registering the swapped bundle again ("Failed to get unit … from store"), then nothing.
    - `open ~/Applications/Devid.app` sent it an event from LaunchServices, and the window and the icon appeared at once.
  - The same `exec` did not hang with a test copy of the app. The tests covered the window open, the icon only, and another version swapped in. So the hang depends on the state of LaunchServices, and `exec` gives no guarantee.
- **Decision:**
  - Every restart goes through `tauri::process::restart`: `open -n` on the bundle with the same arguments, then the old process exits. The new process is an ordinary launch by LaunchServices.
  - Unchanged: when the app restarts, with which arguments, and that the daemon and the sites are not touched. D-027's memory gain stays, since the new process starts fresh.
  - Changed: the pid. For a moment two instances run while the old one quits.
- **Not now:** keeping `exec` and sending the app an event from LaunchServices after the restart. That would be a workaround for one symptom of a behavior Apple does not document.
- **Limit:** the fix ships in the app that an update replaces. The update from 0.14.0 still runs 0.14.0's `exec`. If the app does not come back after it, opening Devid again from Spotlight or `~/Applications` brings it back.
- **Accepted:** by the owner, who merged #89 on 2026-11-16.

## D-049: Mail: Devid runs Mailpit to catch the mail that apps send (accepted 2026-11-16, schedules D-011's deferred mail catcher)

- **Context:**
  - D-011 deferred the mail catcher until v1 was in daily use, which it now is (v0.14.1). D-037 kept it deferred because the owner already runs Mailpit; on 2026-11-16 the owner asked for it in Devid.
  - Herd Pro ($99 a year), ServBay Pro and MAMP PRO charge for mail capture. Sail, DDEV, EnvKit and MAMP 7.5 ship Mailpit (`docs/research/competitors-2026-11.md`).
  - The owner already runs Mailpit 1.33.0 by hand: `~/.local/bin/mailpit`, started by a LaunchAgent, on 127.0.0.1:1025 (SMTP) and 8025 (web). After 34 hours it used 8.4 MB RSS and 2.25 s of CPU in total.
  - A catcher of our own in the daemon would save a process. But SMTP, MIME parsing and an inbox page are far more work than Mailpit already does, for less.
- **Decision:**
  - Devid ships Mailpit's official macOS builds (MIT license) in its runtime release, mirrored without a rebuild like Caddy, and the daemon runs Mailpit under its supervisor while Mail is on.
  - Mail is off by default. Turning it on downloads Mailpit (11 MB); nothing runs before that.
  - Mailpit listens on 127.0.0.1 only, with an allowlist for the Host header, no update check and no relay. SMTP is on 2525 by default (what Laravel's `.env.example` uses), and the inbox and its API on the next port.
  - While Mail runs, PHP's `mail()` lands in it too: Devid's PHP gets a `sendmail_path` that pipes to Mailpit (a setting, on by default).
  - Two stages, as designed on the canvas page "0.15.x", which the owner approved on 2026-11-16:
    - Stage 1, 0.15.0: Mail in the daemon, the CLI (`devid mail`), the TUI and the app, with on/off, status, settings, the `.env` lines and the quick menu. "Open inbox" opens Mailpit's own page in the browser.
    - Stage 2, a later 0.15.x: the inbox inside the app, over Mailpit's API.
- **Consequences:**
  - One more component to mirror and update. Mailpit releases often, so Devid pins the version it ships, as it does for Caddy.
  - With Mail on: about 16 MB RSS at rest and 25 MB with 200 messages, with no idle CPU measured (Phase 9, M1). With Mail off, nothing changes.
  - The owner's own Mailpit can keep its ports. Removing it is the owner's choice once Devid's Mail is in use.
- **Not now:** a dump viewer (the rest of D-011's deferral), an inbox per site (Mailpit stores each message's SMTP user name, so a site that sets `MAIL_USERNAME` could be told apart), and relaying mail to a real server.
- **Accepted:** by the owner on 2026-11-16, with the two stages and the canvas prototype.
- **Amended 2026-11-17 (D-051):** stage 2 was done before 0.15.0 could ship, since 0.15.0 waits for a runtime release with Mailpit. Both stages ship in 0.15.0.

## D-050: notifications for new mail use NSUserNotification (accepted 2026-11-16, amends D-049)

- **Context:**
  - The Mail page's "Notify on new mail" (D-049, the canvas board) needs macOS notifications from Devid.app, which is ad-hoc signed (D-010).
  - Tested on the owner's Mac (macOS 14.8) with ad-hoc signed test bundles:
    - **UserNotifications** (`UNUserNotificationCenter`): `requestAuthorization` failed at once with UNErrorDomain 1, "Notifications are not allowed for this application". No prompt was shown.
    - **`NSUserNotificationCenter`**, deprecated since macOS 11: the first notification made macOS show its permission banner ("Notifications may include alerts, sounds and icon badges", Options › Allow). After Allow, banners showed with their title, subtitle, text and sound.
- **Decision:**
  - Devid.app posts with `NSUserNotificationCenter` through objc2-foundation. Both objc2 crates are already in Tauri's tree; only their NSUserNotification feature is new.
  - **A new message:** the sender as the title, as in Mail.app, and the subject below it. No snippet: three lines were too much text for the owner.
  - **Turning notifications on** posts "Devid Mail · Notifications are on" at once, so macOS asks while the user expects it.
  - **Show** opens the Mail page. Banners show while the Devid window is in front, too.
  - **Checking:** the daemon's `status` carries the newest message (`mail.latest`: when Mailpit received it, the sender and the subject). The menu-bar icon checks every 5 s while Mail runs with notifications on (15 s otherwise). It notifies when the newest message is newer than the one it last saw, and stays quiet at its first look, so old mail does not notify when the app starts.
  - Outside an app bundle (a debug build run from a terminal), nothing is posted.
- **Consequences:**
  - A deprecated API: if a later macOS removes it, notifications stop and the switch should be hidden. A Developer ID signature, which D-010 does not plan, would allow UserNotifications.
  - Allowing notifications is up to the user. After Don't Allow, they stay quiet until allowed in System Settings › Notifications › Devid.
- **Not chosen:** `osascript` with `display notification`, which shows as Script Editor and needs its own permission, and no notifications at all.
- **Accepted:** by the owner on 2026-11-16, after the test notifications and with the two-line layout.

## D-051: the app's inbox shows email HTML through its own `mailview:` scheme (proposed 2026-11-16, accepted 2026-11-17, amends D-049)

- **Context:**
  - Stage 2 of D-049 puts the inbox inside Devid.app, as on the boards "Mail · Inbox" and "Mail · Inbox, empty".
  - Whoever sent an email wrote its HTML. Mailpit's `/view/<id>.html` serves it with its `<script>`, and the app's page can call the app's commands.
  - Laravel's mails need their inline styles and `<style>` blocks, and some embed images by `cid:`. A `srcdoc` frame would inherit the window's CSP (`style-src 'self'`) and lose them.
- **Decision:**
  - **Data through the daemon (D-012):** the page uses the inbox requests of M6a (`mail_list`, `mail_message`, `mail_headers`, `mail_raw`, `mail_read`, `mail_delete`, `mail_save`) and never calls Mailpit's API. New mail shows with the window's 2 s refresh, not over Mailpit's WebSocket.
  - **Email HTML through `mailview://localhost/<id>`,** a URI scheme the app registers:
    - Its reply carries its own CSP: `default-src 'none'; img-src * data:; style-src * 'unsafe-inline'; font-src * data:; form-action 'none'`. No scripts, frames, forms or plugins; styles, images and fonts load.
    - The page shows it in an iframe with `sandbox="allow-popups"`: an opaque origin, no scripts, no navigation of the window.
    - Before serving, `cid:` images point to Mailpit's part URL on 127.0.0.1, every `http-equiv` is renamed so no `<meta http-equiv="refresh">` works, and `<base target="_blank">` is appended.
    - Every `color-scheme` is renamed too, so each email shows its light design: its own backgrounds, and white where it sets none, as in Gmail and Mailpit. In dark mode, an email's dark rules otherwise put light text on that white (seen on 2026-11-16).
    - The HTML comes from the `mail_message` reply the page just got, kept in the app. Asking Mailpit again would mark the message read again after "Mark as unread".
  - **Links:** each link in an email opens as a new window. The window's navigation handler opens http, https and mailto URLs in the default browser, and allows nothing else outside the app's own pages and `mailview:`.
    - WebKit asks that handler before it opens a new window.
    - A link that would navigate the frame itself to http:// is blocked by WebKit as mixed content before the handler is asked; hence the `<base>`.
  - **Opening the inbox:** "Open inbox" in the menu-bar menu and in the quick menu opens the Mail page. Mailpit's own page stays one click away: ⋯ › "Open inbox in Mailpit", and "Open in Mailpit" for one message.
- **Consequences:**
  - Remote images in an email load, as in most mail apps. The mail comes from the user's own apps.
  - An email's dark-mode design does not show. The words `http-equiv` and `color-scheme` in an email's visible text show renamed.
  - A link with an explicit `target="_self"` to an http:// URL does nothing. Laravel's buttons use `target="_blank"`, and its plain links get the `<base>`.
  - Saving an attachment writes to ~/Downloads from the daemon. macOS may ask the first time whether `devid` may use the Downloads folder, and again after an update, since the signature is ad hoc (D-010). Not seen yet: the test daemon ran where Downloads is not reachable.
- **Not chosen:** a `srcdoc` frame (the window's CSP strips the email's styles) and Mailpit's `/view/<id>.html` in a frame (its scripts run).
- **Accepted:** by the owner, who merged #98 on 2026-11-16 after testing a debug build, and chose on 2026-11-17 to ship the inbox in 0.15.0 with stage 1.

## D-052: the target machine is the owner's Apple M1 with 8 GB RAM (proposed and accepted 2026-11-20)

- **Context:**
  - Since the start, the owner's Intel Mac (i5-8210Y, 2 cores/4 threads, 16 GB RAM, macOS 14.8) set the budget for every design choice. CLAUDE.md, PROJECT_CONTEXT and CONTRIBUTING named it.
  - On 2026-11-20 the owner moved Devid work to a new Mac: Apple M1, 8 cores, 8 GB RAM, macOS 26.6.1.
  - Devid 0.15.0 runs on it: `devid doctor` reports all good. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass natively on arm64.
- **Decision:**
  - The target machine is the M1 with 8 GB RAM.
  - Idle RAM, CPU and battery cost still matter in every design choice. RAM is now the tightest, at half of the old Mac's, and macOS, the browser and the owner's dev servers share it.
  - Intel Macs stay a supported platform (D-044 unchanged). Work that is light on 8 cores but heavy on 2 cores still needs a reason.
- **Consequences:**
  - Apple silicon builds are now tested by hand on the owner's Mac, as well as on GitHub's runners (D-046). On the owner's request, the README says Devid supports Macs with Apple silicon (M series) and is tested on an M1.
  - Intel builds keep CI on `macos-15-intel`. Whether they still get a hand test is open.
  - Measurements taken on the Intel Mac (Phase 7, Phase 8, PROJECT_CONTEXT) stay as recorded, with that machine named. New measurements name the M1.
- **Accepted:** by the owner, who merged #102 on 2026-11-20.

## D-053: Devid's commands on PATH for every shell, with a button to add them (proposed and accepted 2026-11-20, amends D-014)

- **Context:**
  - Setup (D-014) puts `~/.devid/bin` on PATH with one line in `~/.zshrc`, and that folder holds `devid`, `php` and `composer`. Only a terminal reads `~/.zshrc`; the shells that editors, scripts and tools such as Claude Code start do not, so `php` and `devid` are missing there.
  - The PHP versions, nginx, Caddy and Mailpit that Devid installs have no command of their own. Running one meant typing a path inside `~/.devid` and the flags Devid passes, such as nginx's prefix and config.
  - Nothing in the app showed whether a terminal finds Devid's commands, or put them back after the line was removed.
  - On 2026-11-20 the owner asked for an "Add to PATH" button, so that Devid's services are easy to call from a terminal, and chose the commands below, Settings plus the CLI, and `~/.zshenv`.
- **Decision:**
  - **More shims** in `~/.devid/bin`, hard links to `devid` like `php` (D-014): `php8.3`, `php8.4` and `php8.5` (that version anywhere, with Devid's ini), `nginx` (Devid's nginx on Devid's config, for `-t`, `-T`, `-s`, `-v`, `-V`, `-h` only, since the daemon starts it), `caddy` (with the daemon's XDG folders) and `mailpit` (`mailpit sendmail` delivers to Devid's SMTP port). A missing program says how to install it.
  - **Two startup files for zsh:** the marked PATH line goes into `~/.zshenv`, which every zsh reads, and stays in `~/.zshrc`. In a terminal, macOS's `/etc/zprofile` runs `path_helper` after `~/.zshenv` and moves its entries behind `/usr/local/bin`, so without the `~/.zshrc` line Homebrew's `php` would win again. bash keeps `~/.bash_profile`. The shell comes from the user database, since the daemon's launchd environment need not have `SHELL`.
  - **One switch in three places:** Settings › Terminal in the app (Add to PATH / Remove from PATH, with the command list) through the daemon's `shell_path`; `devid path [on|off]`, which works while the daemon is down; and `p` on the TUI's Settings page. `status` reports `terminal`, and `devid doctor` checks that every startup file has the line.
- **Consequences:**
  - A terminal opened before the change keeps its PATH until it is reopened; every message says so.
  - `nginx`, `caddy` and `mailpit` on PATH are Devid's. Devid's folder comes first on PATH, so it hides a Homebrew nginx, Caddy or Mailpit too. Removing the line from PATH, or calling the other copy by its full path, reaches those.
  - Existing installs get the shims and the `~/.zshenv` line with the next `devid self-update`, which runs setup.
- **Not chosen:** symlinks for the new commands (PHP resolves them for `PHP_BINARY`, and nginx would run without Devid's config); a PATH line in `~/.zshenv` only (Homebrew's `php` would win in a terminal); adding any folder the user picks, such as `~/.local/bin` (outside Devid's job).
- **Accepted:** by the owner, who merged #103 on 2026-11-20.

## D-054: dev servers on demand (proposed and accepted 2026-11-21, Phase 10, amends D-036 and D-040)

- **Context:**
  - A proxy site's dev server runs from Start to Stop (D-036). Nothing starts at login, so after a login or a crash every proxy site shows "Nothing answers" until its command is started by hand.
  - Once started, a dev server keeps its RAM all day. On the owner's M1 with 8 GB (D-052), three idle proxy sites held about 1.1 GB on 2026-11-21 (devid-site's `next dev` alone 777 MB), with 2.6 GB of swap in use.
  - A warm Next.js dev server answers its first page about 2 s after it starts.
  - Herd 2 starts dev servers on the first request and stops them after 5 minutes idle, behind Herd Studio.
  - On 2026-11-21 the owner picked this as the next feature.
- **Proposal:**
  - **Opt-in:** a proxy site can name one of its saved commands as `on_demand`, with `idle_stop` in minutes (10 by default, 0 for never).
  - **Waking:** when the dev server refuses a request, the web server asks the daemon over a unix socket (`~/.devid/run/wake.sock`). The daemon starts the command once and waits until its port accepts, then the web server sends the same request again (method and body included).
  - **Slow starts:** a page load still starting after 5 s gets a "Starting…" page that refreshes itself. Other requests wait up to 60 s. A command that exits shows its exit code and log.
  - **Idle:** while an on-demand command runs, the daemon checks every 30 s for requests (the site's access log, written only for on-demand sites) and open connections (the command's own sockets). An open tab keeps its HMR WebSocket and so counts as use. After `idle_stop` minutes of neither, the command is stopped and shown as sleeping.
  - **nginx first.** Caddy and Apache follow after a spike, or show on-demand as unavailable.
- **What it changes:**
  - **D-036:** a request, not only the user, can start the one command the user marked. It still never starts at login or by itself, and a repository cannot mark a command.
  - **Stop:** on an on-demand command, Stop means sleep until the next request.
  - **D-040:** the update resume skips on-demand commands.
- **Found on the way:**
  - nginx's implicit upstream for `localhost:<port>` skips a refused address for 10 s. In 0.16 a dev server that has just come up keeps showing "Nothing answers" for up to 10.6 s.
  - The fix is Phase 10's O2, which can ship on its own: a failed request tries once more through an `upstream` group with `max_fails=0`, but only if it never reached the dev server. The main path keeps the skipping group, since `max_fails=0` there would log a refusal on every other request.
- **Not chosen:**
  - `X-Accel-Redirect` back to the dev server, which nginx follows as a GET.
  - A full HTTP proxy in the daemon.
  - Holding every request until the server answers, which shows a blank tab during a long build.
- **Plan:** `docs/roadmap/phase-10.md`.
- **Status:** accepted by the owner on 2026-11-21 ("Ya, mulai O3"). O2 shipped in 0.16.1; O3 (nginx) merged on 2026-11-21 (#110); O4 (Caddy and Apache) merged on 2026-11-21 (#111); O5 (the app and the TUI) merged on 2026-11-22 (#113). They ship in 0.17.0.
- **Amended 2026-11-21 (O4, Caddy and Apache):** both wake on-demand sites, so on-demand is never "unavailable".
  - **Apache sends the client back.** Apache reaches the daemon through `ErrorDocument 503`, which loses the method and cannot send the request again itself. So the daemon answers `307` to the same URL, and the browser sends the request again, method and body included.
  - **No idle connections to an on-demand dev server.** Caddy and Apache would keep idle connections to the dev server open for reuse. The idle check counts an open connection as an open tab, so one held open by a dev server that never closes it kept the site awake. They now close each connection after its request (`keepalive off`, `disablereuse=On`), as nginx does.
  - **Apache keeps long connections, for all proxy sites.** An open tab's HMR WebSocket must survive, as it does on nginx.
    - Its proxy line gets `timeout=3600`, nginx's `proxy_read_timeout 1h`. Apache's `Timeout` of 60 s had cut slow responses and idle WebSockets.
    - `ServerLimit` goes from 1 to 16, so a graceful reload can start its new process while the old one finishes open connections. Before, every request after a config change waited for an open WebSocket to close: 89 s in the check.
    - Not chosen: a hard restart (SIGHUP), as Caddy does. Apache then needs about 10 s to kill a process with an open WebSocket, and every request waits for that.
- **Amended 2026-11-22 (O5, the app and the TUI):** the owner approved the board "Devid — Dev servers on demand (Phase 10 O5)" on 2026-11-22 ("Sudah, bangun app"). Two points differ from the plan's wording:
  - **One On demand card per site.** The card holds the switch, the command and the idle time, instead of a switch on a command row: only one command per site can be on demand.
  - **TUI key `z`.** `o` already opens the site.

## D-055: explanations in tooltips on the site detail (accepted 2026-11-22)

- **Context:** the site detail had grown with Phase 8 and Phase 10. Every row carried a line of explanation, three cards had a note under them, and an on-demand command's state showed in its row and again in the On demand card. On 2026-11-22 the owner asked: "pada halaman detail site untuk teks-teks keterangan dipindahkan ke tooltip agar lebih ringkas dan bersih, dan kurangi narasi perulangan".
- **Decision:**
  - **Explanations move into tooltips.** Each row's title has an (i) that shows its explanation on hover or keyboard focus.
  - **Notes move to their headings.** The notes under App, Commands, On demand and Share are now the tooltips of those headings.
  - **What needs action stays on the page:** a PHP version that is not installed, a busy local port, a cloudflared that stopped, and why the on-demand switch is disabled.
  - **A status is said once.** The On demand card no longer repeats the command's state, which its row shows. That row says "Sleeping", and its chip's tooltip says when the command starts and stops.
- **How the tooltip works:**
  - It is fixed on the page body, as the menus are, so no card or scroll area clips it.
  - The (i) can be focused, and its label is the same text for screen readers.
  - Esc closes it. A redraw from polling waits until it closes.
- **Scope:** the site detail. Other pages keep their texts until there is a reason to change them.

## D-056: Vite over HTTPS for Laravel sites (proposed and accepted 2026-11-22, Phase 11)

- **Context:**
  - laravel-vite-plugin writes Vite's address to `public/hot` (for example `http://[::1]:5173`), and Laravel's `@vite` tags load a page's scripts and CSS from it.
  - The plugin makes Vite serve HTTPS only with Herd's or Valet's certificates, found in their config folders, or with `VITE_DEV_SERVER_KEY` and `VITE_DEV_SERVER_CERT` plus `APP_URL`'s host.
  - With Devid, Vite stays on plain HTTP. In the Phase 11 spike on 2026-11-22, Chrome 158 loaded it from `https://<site>.test`, since it trusts loopback. Safari 26.6 blocked all of it, so the page stayed blank.
  - With the two variables set to the site's Devid certificate, both browsers worked, HMR included.
  - With an `APP_URL` on another host, Vite did not start at all (`ENOTFOUND localhost:8000`).
- **Proposal:**
  - When a PHP site's command starts, the daemon reads the project's `.env` for the host of `APP_URL` and for `VITE_DEV_SERVER_KEY`/`VITE_DEV_SERVER_CERT`.
  - If `APP_URL`'s host is the site's host, with no port, and the project sets neither variable, the command gets both. They point at the site's key and certificate in `~/.devid/certs`. Vite's `loadEnv` lets the process environment win over `.env`.
  - If the project uses laravel-vite-plugin and `APP_URL` names another host, nothing is set. `devid cmd`, the TUI and the app show one note that says how to fix `APP_URL`; it stays on the page, since it needs action (D-055).
  - Vite started in the user's own terminal is covered by the guide: two `.env` lines with absolute paths.
- **What it changes:** a PHP site's command now reads its project's `.env`, but only for `APP_URL` and those two names. Before, `.env` was read only for proxy sites, and only for `PORT`.
- **Not chosen:**
  - **Setting `APP_URL` on the command too.** Laravel keeps an environment `APP_URL` over `.env`, so queue workers, and Vite plugins that call `artisan`, would see another `APP_URL` than the app.
  - **Certificates in `~/.config/valet`.** Other tools read that folder as "Valet is installed".
  - **Proxying Vite through the site's web server.** Vite's module paths and CSS URLs still point at Vite's own address, so the project's `base` would have to change.
- **Plan:** `docs/roadmap/phase-11.md`.
- **Status:** accepted by the owner on 2026-11-22 ("Ya, mulai V2"). V2 (the daemon and the clients) merged on 2026-11-22 (#117) and shipped in 0.18.0, released on 2026-11-22.

## D-057: Linux starts with Ubuntu LTS, the CLI and the TUI (proposed and accepted 2026-11-23, Phase 12)

- **Context:**
  - D-044 puts Debian-based Linux after Apple silicon, which shipped in 0.14.0. On 2026-11-23 the owner chose Linux, not Windows, as the next platform.
  - The daemon's core, the CLI and the TUI already build and pass every test on CI's Ubuntu runners. DNS, ports 80/443, CA trust, start at login, the runtime builds, install and self-update are macOS-only (`docs/roadmap/phase-12.md`, "Where things stand").
  - A fully static musl PHP cannot load `.so` files, and Xdebug, pcov and the extension catalog are `.so` files (D-008, D-021).
  - On Debian, systemd-resolved is a separate package, and installing it takes over `/etc/resolv.conf`. Ubuntu uses it by default.
- **Proposal:**
  - **Target:** Ubuntu 24.04 and 26.04 LTS on x86_64, with the CLI and the TUI. Web servers: nginx and Caddy.
  - **One root step** (`sudo devid helper apply`, undone by `devid uninstall`):
    - systemd-resolved sends only `.test` to Devid's DNS port;
    - an nftables table, loaded at boot by a system unit, redirects `127.0.0.1` and `[::1]` ports 80/443 to the web server's high ports;
    - the Devid CA goes into the system store with `update-ca-certificates`, and browsers get it through NSS.
  - **As the user:** a systemd user unit for the daemon, the PATH line in `~/.bashrc` for bash, and `~/.devid`.
  - **Runtime:** PHP from static-php-cli's glibc targets, built for glibc 2.36 (Debian 12) and later, so `.so` extensions load. nginx and Devid itself as static musl builds.
  - **Testing:**
    - GitHub's Ubuntu runners, which allow `sudo`, for CI and most of the spike.
    - The owner's old Intel Mac running Xubuntu, reached over SSH, for real browsers and reboots. Claude runs these checks.
    - Nothing runs on the owner's M1, and there is no VM on it (D-013).
  - The spike (L1) confirms each part before any code lands.
- **Not chosen:**
  - **`net.ipv4.ip_unprivileged_port_start=80`:** every user's process could take a low port.
  - **`setcap` on the web server:** lost at every update of the binary.
  - **Debian first:** DNS without systemd-resolved needs its own answer. Debian follows Ubuntu.
- **Not in Phase 12:** the app on Linux, arm64 Linux, Debian without systemd-resolved, Fedora, Arch, Apache on Linux, and Windows (D-044).
- **Owner's answers (2026-11-23):**
  - Ubuntu first, on Xubuntu; the owner sets up the machine.
  - Claude runs the desktop checks on the owner's old Intel Mac running Xubuntu, over SSH on the local network.
  - The CLI and the TUI first; the app on Linux after them.
- **L1 (2026-11-23), on GitHub's Ubuntu 24.04 and 26.04 runners:** every part holds, with these details:
  - **PHP:** `SPC_TARGET=x86_64-linux-gnu.2.17`, not `native-…`, which builds for the build machine's CPU and died with SIGILL on older runners. Its newest glibc symbol is 2.17. Xdebug, pcov and apcu load, and it runs on Ubuntu 22.04 to 26.04 and Debian 12 and 13.
  - **DNS:** the global drop-in `/etc/systemd/resolved.conf.d/devid.conf` (`DNS=127.0.0.1:53535`, `Domains=~test`). No query outside `.test` reached Devid's port.
  - **Ports:** an `inet devid` nftables table with an `output` nat hook, loaded by a oneshot system unit. It works next to Docker and ufw.
  - **CA:** the system store serves curl and Firefox 160. Chrome 157 ignores it and needs `~/.pki/nssdb` (`certutil`) or its `CACertificates` policy; L3 picks one.
  - **The rest:** a systemd user unit, the `/proc` connection check, nginx and Devid as static musl builds (Devid's tests pass on musl), and about 19 MB idle for the daemon, nginx and one php-fpm master.
- **L2 (2026-11-23), runtime release `2026.11.23-19` (hadziqmtqn/devid-binaries#19):**
  - **What ships:** the Linux x86_64 runtime sits next to macOS's:
    - PHP 8.3 to 8.5 for glibc 2.17, with the eight catalog extensions;
    - nginx as a static musl build;
    - the official Caddy and Mailpit.
  - **Manifest:** Linux builds go in the same entries under the arch key `linux-x86_64`. Devid on macOS looks up only `x86_64` or `arm64`, so 0.18.0 reads the release unchanged.
  - **Extensions:**
    - linked with `-z nodelete`, so their libraries' exit handlers survive PHP's `dlclose`;
    - linked with `-Bsymbolic`, so their own OpenSSL and libc++ never bind to PHP's;
    - in CI, loaded with `LD_BIND_NOW` and smoke-tested on QEMU's Nehalem CPU too.
  - **imagick's fonts:** a Linux `type.xml` names Debian's and Ubuntu's DejaVu and Liberation fonts, since the static build has no fontconfig.
- **Plan:** `docs/roadmap/phase-12.md`.
- **Status:** accepted by the owner on 2026-11-23, answering the plan's three questions ("ubuntu dulu, berbasis Xubuntu"; "sesuai usulan kamu, dahulukan pondasi CLI dan TUI dulu baru ke desktop app linux").
