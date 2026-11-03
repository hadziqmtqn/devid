# Phase 6 Plan: extra PHP extensions from a trusted catalog

> Scope: letting users install PHP extensions that Devid does not bundle, only from verified sources (D-021, which supersedes D-020)
> Verified against: 2026-11-03
> Status: not started (after v0.2.3)

Devid's PHP is a static build with a fixed extension set (D-008). Xdebug and pcov already ship as separate `.so` files, which shows that the static php-fpm can load shared extensions. Phase 6 adds a second, opt-in set: the **extension catalog**.

## How it works for the user

```text
devid php ext available 8.4          # what the catalog offers for PHP 8.4
devid php ext install 8.4 mongodb    # download, verify, enable, restart, check
devid php ext remove 8.4 mongodb
devid php ext list 8.4               # bundled, catalog and their on/off state
```

- Extensions come **only** from the catalog. There is no `ext add <file.so>`, and nothing is compiled on the user's Mac, so no Xcode Command Line Tools are needed.
- `install` verifies the SHA-256, writes `conf.d/devid-<name>.ini` (`extension` or `zend_extension`, in the right load order), restarts that version's php-fpm and checks `php -m`. If php-fpm fails to start, Devid removes the ini again and says why.
- A PHP update brings the matching extension builds in the same release. `devid update` updates both together; `doctor` reports an extension that no longer loads.
- An extension that is not in the catalog is requested and, if it can be built (see below), added to the catalog.

## Where the catalog comes from

- `manifest.json` in `devid-binaries` (D-016) gets an `extensions` section. Each entry lists the name, version, PHP version, URL, SHA-256, `extension` or `zend_extension`, source and license.
- CI builds each extension from its official source, pinned by checksum: PECL (`pecl.php.net`), php-src (the same tarball as the PHP build) or the project's own release. It builds against the same static PHP and bundled libraries as the PHP release, so versions always match.
- Closed-source vendor loaders are listed only when their license allows Devid to mirror them, or to download them from the vendor's official URL with a pinned checksum.

## Which extensions can be added (draft for the user docs)

This table goes into the README when Phase 6 ships (E5). The E1 spike confirms or corrects it.

| Kind | Examples | Can be added? |
|---|---|---|
| Already bundled | redis, intl, gd, pdo_pgsql, Xdebug, pcov | Nothing to install. Turn Xdebug and pcov on with `devid php ext <ver> xdebug on`. |
| Open source on PECL, no extra library | apcu, igbinary, msgpack, ds, uuid | Yes. |
| Open source, bundles its own libraries | mongodb, grpc | Yes, if the E1 spike shows no clash with the libraries already inside PHP (OpenSSL, curl). |
| Open source, needs an open-source C library | imagick (ImageMagick), memcached (libmemcached) | Yes. CI builds the library into the extension, so nothing else is installed. |
| In php-src but not in Devid's build | soap, xsl, gmp, ldap, tidy | Yes, built from the same php-src as the PHP release (with the C library they need built in). |
| Swoole / OpenSwoole | swoole, openswoole | Yes, for CLI use (`php artisan octane:start`). They do not change how php-fpm serves `.test` sites. |
| Closed-source loaders | ionCube Loader, SourceGuardian | Only if the vendor's license allows mirroring or an official download; checked per vendor. Otherwise not available. |
| APM and profiler agents | New Relic, Datadog, Blackfire | Case by case: closed source, and most need their own background agent. Not planned for Phase 6. |

**Cannot be added, and why:**

| Reason | Examples |
|---|---|
| Needs a proprietary system library that Devid may not ship | oci8 / pdo_oci (Oracle Instant Client), sqlsrv / pdo_sqlsrv (Microsoft ODBC Driver) |
| Closed source and the license forbids redistribution | any vendor loader without a redistribution or official-download license |
| Needs thread-safe PHP (ZTS); Devid builds non-thread-safe PHP for php-fpm | parallel, pthreads |
| No support for PHP 8.3 or newer | Zend Guard Loader, pthreads |
| Windows only | com_dotnet |
| Clashes with a library already built into Devid's static PHP | to be found in the E1 spike |

## Milestones

| # | Milestone | Done when |
|---|---|---|
| E1 | **Spike:** build apcu (no deps), mongodb (bundles libraries, uses OpenSSL), soap (php-src) and imagick (external library) as shared extensions against Devid's static PHP 8.4 and 8.5; load each in the CLI and php-fpm, with Xdebug on and off. Check one closed-source loader's license and loading. Record results in PROJECT_CONTEXT and correct the table above. | each case is shown to work or the reason it cannot is known |
| E2 | **Catalog in `devid-binaries`:** CI builds the first extensions per PHP version, publishes them with checksums and adds the `extensions` section to `manifest.json`. | the manifest lists apcu (and the other E1 successes) for 8.3, 8.4 and 8.5 |
| E3 | **Daemon and CLI:** `devid php ext available / install / remove / list` with verify, load check and rollback; `devid update` and `doctor` cover catalog extensions. | `devid php ext install 8.4 apcu` works on the owner's Mac; a failing extension is rolled back and PHP keeps running |
| E4 | **TUI:** the PHP page's Extensions section shows catalog extensions with install and remove (with confirmation). | the same from the TUI |
| E5 | **Docs and release:** the README section "PHP extensions" with the tables above, a release. | `devid self-update` brings it to the owner's Mac |

Each milestone is one worktree and one PR. New catalog entries after Phase 6 are changes in `devid-binaries` only.

## Out of scope

- Installing a user-supplied `.so`, or compiling extensions on the user's Mac (D-021).
- Extensions on macOS arm64, Linux or Windows; they follow the platform work in D-019.
