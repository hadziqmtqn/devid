# Fix: duplicate OPcache in `devid php ext list`; apcu on in the CLI (v0.4.1)

> Date: 2026-11-05 · Branch: `fix/ext-list-apcu-cli` · Ships in v0.4.1

## Problems

The owner checked the extensions they had installed on PHP 8.5 (apcu, ldap and mongodb):
- **Duplicate entry:** `devid php ext list` and the TUI's Bundled line showed `Zend OPcache` twice. `php -m` lists a Zend extension under both "[PHP Modules]" and "[Zend Modules]".
- **apcu in the CLI:** apcu was off there (PHP's default `apc.enable_cli=0`). `php artisan` and tests could not use the cache API that the sites use.

## Fix

- **`phpext::module_names`:** reads `php -m` and keeps each name once, in order. `PhpExtList` uses it.
- **apcu's ini:** the generated `conf.d/devid-apcu.ini` also sets `apc.enable_cli=1`. An apcu installed before v0.4.1 keeps its old ini until it is installed again (`devid php ext install <ver> apcu`). Devid does not rewrite installed ini files on its own.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. New unit tests: `module_names_are_listed_once`, and `ini_points_at_the_installed_file`, which now checks that only apcu gets `apc.enable_cli=1`. `tests/ext.rs` checks the setting in the installed ini.
- With the owner's PHP 8.5.12 and installed `apcu.so`, using a temporary scan dir so the owner's files were untouched:
  - with the setting, `apcu_store`/`apcu_fetch` work in the CLI; without it, nothing is stored;
  - `php -r ''` takes 0.04 s either way, and RSS is 13.2 MB.
