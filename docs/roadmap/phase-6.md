# Phase 6 Plan: user-installed PHP extensions

> Scope: letting users add PHP extensions that Devid does not bundle, starting with the ionCube Loader (D-020)
> Verified against: 2026-11-03
> Status: proposed, not started (after v0.2.3)

Devid's PHP is a static build with a fixed extension set (D-008). Xdebug and pcov already load as separate `.so` files, which shows that the static php-fpm can load shared extensions. Phase 6 opens that path to extensions the user brings.

## Why ionCube first

- ionCube is closed source; it cannot be built from PECL. It ships as prebuilt loaders, one file per PHP version and platform (for example `ioncube_loader_dar_8.4.so`).
- So the first feature is **adding a prebuilt `.so`**, which needs no compiler. PECL builds come after it and reuse the same install, check and rollback steps.

## Risks to settle in the spike

| Risk | Why it matters | How to check |
|---|---|---|
| ionCube rejects or crashes with a static PHP | the loader expects symbols from a normal PHP build | load it with `php -n -d zend_extension=… -v` and through php-fpm on an encoded test file |
| Build mismatch | the loader must match the PHP API and NTS; Devid builds NTS | compare `php -i` "PHP Extension Build" with the loader's |
| Load order | ionCube must load before other Zend extensions such as Xdebug | name its ini so it sorts first in `conf.d`; test with Xdebug on |
| PHP 8.5 support | ionCube supports new PHP versions some time after release | check which of 8.3/8.4/8.5 have loaders; Devid lists unsupported versions clearly |
| macOS code signing and quarantine | a downloaded `.so` carries `com.apple.quarantine`; library validation can block it | load a freshly downloaded loader; clear quarantine only on the copy Devid installs |
| License | whether Devid may download or mirror the loader | read the ionCube loader license before any automatic download |

## Milestones

| # | Milestone | Done when |
|---|---|---|
| E1 | **Spike:** load ionCube into Devid's PHP 8.3 and 8.4 (and 8.5 if a loader exists) on the owner's Mac, CLI and php-fpm, with Xdebug on and off. Record the result in PROJECT_CONTEXT. | an ionCube-encoded file runs on a `.test` site; D-020 accepted or revised |
| E2 | **`devid php ext add <ver> <file.so>` / `remove` / `list`:** daemon API and CLI. Copy into `~/.devid/php/<ver>/modules/`, write `conf.d/<order>-<name>.ini` (`zend_extension` or `extension`, detected), restart, verify with `php -m`, roll back on failure. `list` shows bundled, optional and user extensions. | ionCube added and removed from the CLI; a broken `.so` is rolled back and PHP keeps running |
| E3 | **TUI:** the PHP page's Extensions section adds and removes user extensions (path input, confirmation). | the same from the TUI |
| E4 | **Updates:** after `devid update` changes a PHP version, re-check user extensions; disable any that no longer load and say why. `doctor` reports them. | a mismatched `.so` after an update is disabled, not a broken php-fpm |
| E5 | **PECL builds:** a per-version dev kit (headers, `phpize`, `php-config` with paths rewritten) in `devid-binaries` releases; `devid php ext install <ver> <name|tgz>`; `doctor` checks the Xcode Command Line Tools. Spike apcu (no deps), mongodb (bundled deps) and imagick (external lib) first. | apcu builds and loads from PECL on the owner's Mac |

Each milestone is one worktree and one PR. E5 can move to a later phase if E1–E4 cover the real need.

## Out of scope

- Devid building or updating user extensions automatically.
- Providing C libraries for extensions (ImageMagick and similar).
- Extensions on Linux or Windows (D-019 covers other OSes later).
