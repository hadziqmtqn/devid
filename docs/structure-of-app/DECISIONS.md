# Decisions

> Scope: decisions that constrain Devid's design
> Verified against: 2026-11-02
> Status: current

Never delete an entry. When a decision is replaced, mark it `superseded by D-xxx`.

| ID | Date | Status | Decision |
|---|---|---|---|
| D-001 | 2026-11-01 | accepted | Build our own native app: no Homebrew, no VM |
| D-002 | 2026-11-01 | accepted | Rust workspace: `daemon`, `cli`, `helper`, `gui` |
| D-003 | 2026-11-01 | accepted | GUI: Tauri v2 + React + Tailwind + shadcn/ui, menu bar first |
| D-004 | 2026-11-01 | accepted | PHP 8.1 to latest from static-php-cli builds, php-fpm with `pm = ondemand` |
| D-005 | 2026-11-01 | accepted | One global web server: Caddy (default), nginx, Apache |
| D-006 | 2026-11-01 | accepted | Databases are out of scope (DBngin is used) |
| D-007 | 2026-11-01 | accepted | UI follows macOS System Settings, per the approved prototype |
| D-008 | 2026-11-02 | proposed | How to ship static PHP binaries (details below) |

## D-001: native app, no Homebrew, no VM

- **Context:** a 2-core Intel Mac with 16 GB RAM; idle resources matter.
- **Rejected:** Lerd + Podman (VM uses 3–4 GB), Yerd and EnvKit (Apple Silicon only), Herd (owner preference), zerobrew, php.new.
- **Consequence:** we own PHP builds, web server binaries, DNS and the CA.

## D-004: static PHP, `pm = ondemand`

- **Context:** keep idle RAM near zero and avoid Homebrew dylib chains.
- **Consequence:** we must rebuild for every PHP patch release and every dependency CVE. 8.1 is marked EOL in the UI.

## D-005: one global web server

- **Consequence:** there is no per-site server choice. The nginx static build and the Apache source are still open risks (see PROJECT_CONTEXT).

## D-008: shipping static PHP binaries (proposed 2026-11-02, awaiting Phase 1)

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

**Accept when:** Phase 1 confirms risks 1–3 in PROJECT_CONTEXT.

**Evidence so far (2026-11-02):** on the owner's Mac, PHP 8.5.12 builds static, loads shared `xdebug.so` and `pcov.so`, serves Laravel through Caddy, and passes erdbpro-saas's 532 tests. The fallback is likely unnecessary. Still open:
- builds for 8.1 and 8.4;
- HTTPS to `.test` sites through the Devid CA;
- pinning a stable Xdebug.
