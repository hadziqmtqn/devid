# Decisions

> Scope: decisions that constrain Devid's design
> Verified against: 2026-11-02
> Status: current

Never delete an entry. When a decision is replaced, mark it `superseded by D-xxx`.

| ID | Date | Status | Decision |
|---|---|---|---|
| D-001 | 2026-11-01 | accepted | Build our own native app: no Homebrew, no VM |
| D-002 | 2026-11-01 | accepted (`gui` crate dropped by D-012) | Rust workspace: `daemon`, `cli`, `helper`, `gui` |
| D-003 | 2026-11-01 | superseded by D-012 | GUI: Tauri v2 + React + Tailwind + shadcn/ui, menu bar first |
| D-004 | 2026-11-01 | accepted (version range superseded by D-009) | PHP 8.1 to latest from static-php-cli builds, php-fpm with `pm = ondemand` |
| D-005 | 2026-11-01 | accepted (default changed to nginx by D-015) | One global web server: Caddy (default), nginx, Apache |
| D-006 | 2026-11-01 | accepted | Databases are out of scope (DBngin is used) |
| D-007 | 2026-11-01 | accepted (applies to the TUI page structure, D-012) | UI follows macOS System Settings, per the approved prototype |
| D-008 | 2026-11-02 | accepted | How to ship static PHP binaries (details below) |
| D-009 | 2026-11-02 | accepted | Supported PHP versions: 8.3, 8.4, 8.5; no 8.1 or 8.2 |
| D-010 | 2026-11-02 | accepted | Personal use first; a public release needs a prominent notice |
| D-011 | 2026-11-02 | accepted | v1 scope: sites, PHP versions, HTTPS, parked folders |
| D-012 | 2026-11-02 | accepted | v1 is CLI + TUI; GUI deferred; all logic in the daemon |
| D-013 | 2026-11-02 | accepted | v1 is macOS only; OS-specific code isolated in a `platform` module |
| D-014 | 2026-11-02 | accepted | Install with `install.sh` plus `devid setup`/`doctor`/`uninstall`; no Node |
| D-015 | 2026-11-02 | accepted | nginx is the default web server; Caddy and Apache stay selectable |

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
  - `devid setup` is idempotent. It downloads Caddy and PHP (sha256-checked), installs the daemon as a user LaunchAgent, then lists the system changes and asks for sudo once for the helper (`/etc/resolver/test`, CA trust, ports 80/443).
  - `devid doctor` reports each component with a fix hint.
  - `devid uninstall` reverses every change made by `setup`.
- **Release contract:** every release publishes `devid-darwin-x86_64` (and `devid-darwin-arm64` once D-010 allows it) plus a `checksums.txt` in `shasum -a 256` format.
- **Status:** `install.sh` exists. It fails cleanly until the first release exists. `setup`, `doctor` and `uninstall` arrive with Phase 2.

## D-013: macOS only for v1 (accepted 2026-11-02)

- **Context:** the CLI, TUI, Caddy and static-php-cli are cross-platform. DNS, CA trust, privileged ports, background services and php-fpm differ per OS; for example, Windows has no php-fpm.
- **Decision:** v1 targets macOS only. All OS-specific code (resolver, CA trust, port binding, service install, shims) lives in a `platform` module and in the `helper` crate. The rest of the daemon must not call macOS APIs directly.
- **Later:** Linux means a second `platform` implementation. Test it on GitHub Actions Ubuntu runners and a small VPS (1 vCPU / 1 GB is enough to run Devid, but not to build PHP), never in a local VM on the owner's laptop. Windows is a much larger effort and is not planned.

## D-015: nginx as the default web server (accepted 2026-11-02)

- **Context:** the web server spike (PROJECT_CONTEXT, 2026-11-02) served erdbpro-saas identically on all three servers. At idle the web server alone uses Caddy 33.9 MB, nginx 7.8 MB and Apache 8.2 MB. Caddy's main advantage, automatic HTTPS, is unused because Devid manages its own CA and certificates.
- **Decision:**
  - `devid setup` activates **nginx** by default. It is our own static build, about 6 MB and about 2 min to build (`spike/webservers/build-nginx.sh`).
  - **Caddy** (official binary) and **Apache** (macOS `/usr/sbin/httpd`) remain selectable at any time, for example `devid server use caddy` or the TUI Server page.
  - Only one is active at a time (D-005), with the same site configuration rendered for each.
- **Consequences:**
  - The nginx adapter must include the Laravel FastCGI buffer settings (`fastcgi_buffer_size 32k; fastcgi_buffers 16 16k;`).
  - The nginx build joins the CI pipeline and is subject to the same no-`/usr/local` link check.
