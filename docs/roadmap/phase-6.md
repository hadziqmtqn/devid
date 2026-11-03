# Phase 6 Plan: extra PHP extensions from a trusted catalog

> Scope: letting users install PHP extensions that Devid does not bundle, only from verified sources (D-021, which supersedes D-020)
> Verified against: 2026-11-03
> Status: E1 in progress: apcu and mongodb pass (2026-11-03, PHP 8.5); ldap, imagick and a vendor loader not tested yet. D-022 (mysqli, readline and soap bundled) shipped in `devid-binaries` 2026.11.03-4

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
- Users download open-source extensions only from the `devid-binaries` release. Devid never fetches from PECL, php-src or a project's site; only CI does, to build.

### Closed-source vendor loaders (D-021)

The vendor builds these binaries, so its license decides how Devid may get them. Checked per vendor, in this order:

| Case | Where the user's Devid downloads from | What the catalog does |
|---|---|---|
| 1. The license allows mirroring | `devid-binaries`, like any extension | CI downloads the vendor's package, checks it loads into each PHP build, republishes it with the license text |
| 2. Only downloads from the vendor are allowed | **the vendor's official HTTPS URL**, directly | the manifest entry has `source: vendor`, the URL, a pinned SHA-256 and the `.so` path per PHP version; CI's upstream check re-pins after testing a changed file |
| 3. Neither | not available | the README lists it with the reason |

Rules for every vendor entry:
- The manifest records the vendor, the license URL, the case and the date the license was checked.
- Before the first install, Devid shows the vendor and the license link and asks the user to accept (a prompt, or `--accept-license` in scripts). In case 2 it also says the file comes from the vendor's server.
- Devid downloads only when the user runs `install` or `update`, never in the background.
- If the vendor's file no longer matches the pinned SHA-256, Devid refuses to install it ("the vendor changed this file; wait for a catalog update") instead of trusting it.
- Never a URL from the user or from outside the manifest, never plain HTTP, never a file without a pinned SHA-256.

## What goes into the catalog first

The catalog is curated, not a copy of every extension. Packagist lists 356 extensions that support PIE (353 `php-ext`, 3 `php-ext-zend`, checked 2026-11-03), many of them experiments, examples or one-person projects. Each catalog entry is third-party code that runs inside users' PHP, and costs one build per PHP version per release.

**Source index:** PIE and Packagist first; the PIE docs say PECL is deprecated. PECL only for an extension that is not on PIE yet, and php-src for php-src extensions. PIE metadata already filters candidates: `os-families` (must include `darwin`) and `support-nts`.

**An extension enters the catalog when:**
- it has a stable, maintained release that supports PHP 8.3–8.5, NTS and macOS;
- its C dependencies are open source and can be built in statically, with no proprietary library;
- it builds, loads and passes a smoke test in CI for every PHP version;
- PHP frameworks need it (below) or users ask for it.

**Priority: what Laravel, CodeIgniter, Symfony and similar frameworks need.** Reference set: the extensions in Laravel Sail's PHP 8.4/8.5 runtime images, plus the optional extensions in the framework docs. Most of them Devid already bundles (bcmath, curl, gd, intl, mbstring, pgsql, sqlite3, redis, xml, zip, pcov, Xdebug).

| Priority | Extension | Who needs it | Package (PIE) |
|---|---|---|---|
| 1 | imagick | Intervention Image and Spatie Image (Laravel), CodeIgniter 4 Image (ImageMagick handler), WordPress; Sail | `imagick/imagick` |
| 1 | igbinary | Laravel Redis and cache serializer; Sail | `igbinary/igbinary` |
| 1 | msgpack | Redis serializer option; Sail | `msgpack/msgpack-php` |
| 1 | memcached | cache drivers in Laravel, CodeIgniter 4, Symfony; Sail | `php-memcached/php-memcached` |
| 1 | mongodb | `mongodb/laravel-mongodb`, Doctrine MongoDB ODM; Sail; **E1 passed** | `mongodb/mongodb-extension` |
| 1 | swoole | Laravel Octane (Swoole server); Sail | `swoole/swoole` |
| 1 | apcu | Symfony, Drupal, Yii, CakePHP caches; **E1 passed** | `apcu/apcu` |
| 1 | ldap | LdapRecord (Laravel), Symfony Ldap; Sail. In php-src, needs OpenLDAP built in | php-src |
| 2 | amqp | Symfony Messenger, Laravel RabbitMQ queues | `php-amqp/php-amqp` |
| 2 | grpc, protobuf | Google Cloud clients, OpenTelemetry | `pie-extensions/grpc`, `pie-extensions/protobuf` |
| 2 | imap | Sail installs it; removed from php-src in 8.4 and barely maintained | `sirlouen/imap` (unofficial) |
| 2 | excimer, event, zstd, brotli, rdkafka, yaml, uuid, ds, ast, mailparse | profiling, ReactPHP/Reverb event loops, compression, Kafka, others | on PIE |

Priority 1 is built in E2. Priority 2 and anything else waits for a request.

### Gaps in the bundled build (D-022, shipped 2026-11-03)

Some php-src extensions that these frameworks need are missing from Devid's PHP build itself. They are small, need no new library or reuse ones already in the build, and are better bundled than offered as catalog downloads:

| Extension | Who needs it | Library |
|---|---|---|
| **mysqli** | CodeIgniter 4 (its only MySQL driver), WordPress; part of Sail's `php-mysql` | none (mysqlnd is already built in) |
| readline | `php artisan tinker` (PsySH) and `php -a` history and editing; Sail | libedit (already in the spc sources) |
| soap | SOAP clients (payment, government and banking APIs); Sail | libxml2 (already built in) |

Adding them changed the PHP build in `devid-binaries` (the `EXT` list), not Devid; they ship since release `2026.11.03-4`. ldap stays in the catalog because it brings OpenLDAP.

## Which extensions can be added (draft for the user docs)

This table goes into the README when Phase 6 ships (E5). The E1 spike confirms or corrects it.

| Kind | Examples | Can be added? |
|---|---|---|
| Already bundled | redis, intl, gd, pdo_pgsql, Xdebug, pcov | Nothing to install. Turn Xdebug and pcov on with `devid php ext <ver> xdebug on`. |
| Open source (PIE or PECL), no extra library | apcu, igbinary, msgpack, ds, uuid | Yes. Confirmed with apcu 5.1.28 (E1). |
| Open source, bundles its own libraries | mongodb, grpc | Yes. Confirmed with mongodb 2.5.5 (E1): it carries its own OpenSSL copy, with no clash with PHP's. TLS to a server with a Devid (`.test`) certificate needs `tlsCAFile=~/.devid/ca/cacert.pem`; public certificates work as is. |
| Open source, needs an open-source C library | imagick (ImageMagick), memcached (libmemcached) | Yes. CI builds the library into the extension, so nothing else is installed. |
| In php-src but not in Devid's build | ldap, xsl, gmp, tidy (mysqli, readline and soap are bundled since 2026-11-03, D-022) | Yes, built from the same php-src as the PHP release (with the C library they need built in). |
| Swoole / OpenSwoole | swoole, openswoole | Yes, for CLI use (`php artisan octane:start`). They do not change how php-fpm serves `.test` sites. |
| Closed-source loaders | ionCube Loader, SourceGuardian | Depends on the vendor's license: from `devid-binaries` (mirroring allowed), directly from the vendor with a pinned SHA-256 (only vendor downloads allowed), or not available. See "Closed-source vendor loaders". |
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
| E1 | **Spike:** build apcu (no deps), mongodb (bundles libraries, uses OpenSSL), ldap (php-src, needs OpenLDAP) and imagick (external library) as shared extensions against Devid's static PHP 8.4 and 8.5; load each in the CLI and php-fpm, with Xdebug on and off. Check one closed-source loader's license and loading. Record results in PROJECT_CONTEXT and correct the table above. | each case is shown to work or the reason it cannot is known |
| E2 | **Catalog in `devid-binaries`:** CI builds the first extensions per PHP version, publishes them with checksums and adds the `extensions` section to `manifest.json`. | the manifest lists apcu (and the other E1 successes) for 8.3, 8.4 and 8.5 |
| E3 | **Daemon and CLI:** `devid php ext available / install / remove / list` with verify, load check and rollback; `devid update` and `doctor` cover catalog extensions. Vendor entries: the license prompt (`--accept-license`) and, for `source: vendor`, the direct download with the pinned SHA-256 and the `.so` path inside the vendor's archive. | `devid php ext install 8.4 apcu` works on the owner's Mac; a failing extension is rolled back and PHP keeps running |
| E4 | **TUI:** the PHP page's Extensions section shows catalog extensions with install and remove (with confirmation). | the same from the TUI |
| E5 | **Docs and release:** the README section "PHP extensions" with the tables above, a release. | `devid self-update` brings it to the owner's Mac |
| E6 | **Adding catalog entries later (GitHub Actions, `devid-binaries`):** a workflow that takes a new recipe (a PR or a manual run with the package name and version), builds it for every PHP version, runs the load and smoke tests, and publishes it with a new `manifest.json`. A scheduled job lists PIE candidates (darwin + NTS) and new releases of catalog entries for review; it never adds an entry by itself. | a new extension reaches the catalog through one reviewed recipe PR, without a Devid release |

### E1 results so far (2026-11-03)

Script and method: `spike/ext/` (README). Built against the PHP 8.5.12 build root of `spike/php`, loaded into the installed PHP 8.5.12 from the Devid release (CLI and php-fpm, `-n -d extension=…`). Nothing was installed.

| | apcu 5.1.28 | mongodb 2.5.5 |
|---|---|---|
| Build time on the owner's Mac (2 cores) | 6 s | 3 min 53 s |
| Size | 91 KB | 8.7 MB |
| Linked libraries | `libSystem` only | `libSystem`, `libresolv` (both in `/usr/lib`) |
| Loads in `php -m` / `php-fpm -m` | yes / yes | yes / yes |
| Works | store, fetch, inc | BSON, TLS: verifies certificates and host names |
| Together with Xdebug, apcu, PHP's own OpenSSL (HTTPS, `.test` TLS) | yes | yes |

Findings that shape E2 (the catalog build):
- **Symbols:** shared extensions use `-undefined dynamic_lookup` and take PHP's symbols from the static binary (146 for apcu, 358 for mongodb). This works, as it does for Xdebug.
- **Second OpenSSL copy:** mongodb links the build root's static OpenSSL into the `.so` (no OpenSSL symbol left undefined). The two copies do not interfere. Consequence: an OpenSSL security fix means rebuilding such extensions together with PHP, in the same release.
- **CA trust:** mongodb's OpenSSL reads `/etc/ssl/cert.pem` (macOS system bundle) and ignores `openssl.cafile`. Public TLS works. A server with a Devid certificate needs `tlsCAFile` (document it).
- **Build paths leak:** mongodb.so has an `LC_RPATH` to the build root and OpenSSL's `ENGINESDIR`/`MODULESDIR` point there. Harmless for the default provider, but the catalog build must drop the rpath (`install_name_tool -delete_rpath`) and use neutral paths.
- **Test method:** public "bad certificate" hosts (badssl.com) gave random "connection closed" results, so TLS checks use a local `openssl s_server` with a throwaway CA, which is deterministic.
- **Still to test:** PHP 8.3 and 8.4 (the build root holds 8.5 only), ldap (php-src with OpenLDAP; soap moved to the bundled build, D-022), imagick (ImageMagick build), and one vendor loader.

Each milestone is one worktree and one PR. After E6, new catalog entries are reviewed recipe PRs in `devid-binaries` only.

## Out of scope

- Installing a user-supplied `.so`, or compiling extensions on the user's Mac (D-021).
- Extensions on macOS arm64, Linux or Windows; they follow the platform work in D-019.
