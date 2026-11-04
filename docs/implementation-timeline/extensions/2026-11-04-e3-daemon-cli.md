# Phase 6 E3: catalog extensions in the daemon and CLI

> Date: 2026-11-04 · Branch: `feat/php-ext-catalog` · Ships in v0.3.0 · Plan: [Phase 6, "E3 design"](../../roadmap/phase-6.md)

## What shipped

- **Commands:** `devid php ext available / install / remove / list <ver> [name]`, as the requests `PhpExtCatalog`, `PhpExtInstall`, `PhpExtRemove` and `PhpExtList`. `devid php ext <ver> xdebug|pcov on|off` is unchanged.
- **`crates/daemon/src/phpext.rs`:**
  - reads the manifest's `extensions` section;
  - accepts only plain names (`[a-z0-9_]`), since names become paths and ini files;
  - checks `php_api` against the installed PHP's, read from `PHP_EXTENSION_DIR` (`php -i` is too large for `probe`);
  - downloads through `releases::download` (trusted prefix, SHA-256).
- **Files:**
  - the archive unpacks to `~/.devid/php-ext/<ver>/<name>/`, with the archive's SHA-256 in `.sha256`;
  - `conf.d/devid-<name>.ini` turns it on.
- **Preflight and rollback:**
  - with the new ini in place, `php -m` and `php-fpm -m` (real `php.ini` and `conf.d`) must list the extension and print no startup warning;
  - otherwise the previous folder and ini come back, and php-fpm is never restarted.
- **Environment:**
  - each installed extension's `env` (imagick: `MAGICK_CONFIGURE_PATH`) goes into the generated `php-fpm.conf` as `env[…]`, because workers start with a cleared environment;
  - it also goes into the php-fpm master's environment and the `php` shim.
- **`devid update`:**
  - reinstalls an extension whose archive SHA-256 changed, which covers a new version or a rebuild of the same version;
  - when PHP itself was updated, removes an extension that the release no longer offers;
  - `--check` lists these changes.
- **`devid doctor`:** each PHP version's installed catalog extensions must load; otherwise the check shows PHP's own message.
- **README:** a "PHP" section on the catalog, a `php-ext/` row in "Files", and the Phase 6 roadmap status.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- **`tests/ext.rs`:** a local `file://` release with fake `php` and `php-fpm`. They list the conf.d extensions and refuse a broken `.so` the way PHP does. The test covers:
  - install, and the ini, files and `.sha256` it leaves;
  - `env[]` in the pool and in the shim;
  - a broken `.so` rolled back without a php-fpm restart (same pid);
  - a wrong API, an unknown name and an illegal name;
  - a same-version rebuild found and reinstalled by `update`;
  - a PHP update that keeps the extensions and drops one the release lost;
  - remove, and the old xdebug syntax.
- **Real release on the owner's Mac:** a separate daemon from this branch, with its own `DEVID_HOME` and ports, so the owner's Devid, its sites and the cloudflared tunnel were untouched.
  - `devid php install 8.4` (8.4.27 from `2026.11.04-6`), then `devid php ext install 8.4 apcu|mongodb|ldap|imagick` took 1.8 s, 2.7 s, 1.9 s and 4.4 s.
  - **In a php-fpm worker** (a real FastCGI request to the socket) and **through the `php` shim:** all four load, `MAGICK_CONFIGURE_PATH` is set, text with the default font draws, and an SVG gradient renders (left pixel `#f90006`). The same holds with Xdebug on.
  - **`doctor`:** reports `ok PHP 8.4 loads apcu, imagick, ldap, mongodb`. With `apcu.so` overwritten by junk, it reports PROBLEM with PHP's own "Unable to load dynamic library … not a mach-o file", and ok again once the file is restored.
  - **`remove imagick`:** drops the `env[]` line, php-fpm restarts and runs, and the shim no longer sets the variable.

## Notes

- apcu is off in the CLI unless `apc.enable_cli=1` (PHP's default). Devid does not change that yet.
- The TUI does not show catalog extensions yet (E4).
