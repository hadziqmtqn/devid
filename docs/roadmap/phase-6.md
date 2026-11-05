# Phase 6 Plan: extra PHP extensions from a trusted catalog

> Scope: letting users install PHP extensions that Devid does not bundle, only from verified sources (D-021, which supersedes D-020)
> Verified against: 2026-11-04
> Status: E1–E4 done (2026-11-05). Release `2026.11.04-6` of `devid-binaries` carries apcu, mongodb, ldap and imagick for PHP 8.3, 8.4 and 8.5. Devid v0.3.0 installs them from the CLI, v0.4.0 also from the TUI (several at once, with progress). E5 (docs and release) is next. ionCube is not offered (D-025).

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
| 1 | imagick | Intervention Image and Spatie Image (Laravel), CodeIgniter 4 Image (ImageMagick handler), WordPress; Sail; **E1 passed** (no HEIC writing, no PDF reading; SVG gradients only in user-space coordinates, D-024) | `imagick/imagick` |
| 1 | igbinary | Laravel Redis and cache serializer; Sail | `igbinary/igbinary` |
| 1 | msgpack | Redis serializer option; Sail | `msgpack/msgpack-php` |
| 1 | memcached | cache drivers in Laravel, CodeIgniter 4, Symfony; Sail | `php-memcached/php-memcached` |
| 1 | mongodb | `mongodb/laravel-mongodb`, Doctrine MongoDB ODM; Sail; **E1 passed** | `mongodb/mongodb-extension` |
| 1 | swoole | Laravel Octane (Swoole server); Sail | `swoole/swoole` |
| 1 | apcu | Symfony, Drupal, Yii, CakePHP caches; **E1 passed** | `apcu/apcu` |
| 1 | ldap | LdapRecord (Laravel), Symfony Ldap; Sail. In php-src, needs OpenLDAP built in; **E1 passed** | php-src |
| 2 | amqp | Symfony Messenger, Laravel RabbitMQ queues | `php-amqp/php-amqp` |
| 2 | grpc, protobuf | Google Cloud clients, OpenTelemetry | `pie-extensions/grpc`, `pie-extensions/protobuf` |
| 2 | imap | Sail installs it; removed from php-src in 8.4 and barely maintained | `sirlouen/imap` (unofficial) |
| 2 | excimer, event, zstd, brotli, rdkafka, yaml, uuid, ds, ast, mailparse | profiling, ReactPHP/Reverb event loops, compression, Kafka, others | on PIE |

E2 shipped the four that passed E1 (apcu, mongodb, ldap, imagick). igbinary, msgpack, memcached and swoole follow as recipe PRs in `devid-binaries`, which need no Devid release. Priority 2 and anything else waits for a request.

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
| Open source, needs an open-source C library | imagick (ImageMagick), memcached (libmemcached) | Yes. CI builds the library into the extension, so nothing else is installed. Confirmed with imagick 3.8.1 and ImageMagick 7.1.2 (E1): JPEG, PNG, GIF, WebP, AVIF, JPEG XL, TIFF and SVG (D-024; with a small ImageMagick patch, gradients work in user-space coordinates but not in `objectBoundingBox` units); HEIC can be read but not written (the HEVC encoder, x265, is GPL); reading PDF needs Ghostscript, which Devid does not ship. |
| In php-src but not in Devid's build | ldap, xsl, gmp, tidy (mysqli, readline and soap are bundled since 2026-11-03, D-022) | Yes, built from the same php-src as the PHP release (with the C library they need built in). Confirmed with ldap and OpenLDAP 2.7.2 (E1): binds over LDAPS and verifies the server certificate. |
| Swoole / OpenSwoole | swoole, openswoole | Yes, for CLI use (`php artisan octane:start`). They do not change how php-fpm serves `.test` sites. |
| Closed-source loaders | ionCube Loader, SourceGuardian | Depends on the vendor's license: from `devid-binaries` (mirroring allowed), directly from the vendor with a pinned SHA-256 (only vendor downloads allowed), or not available. See "Closed-source vendor loaders". ionCube (E1): its license allows mirroring, but ionCube's last Intel Mac loader (14.0.0) covers PHP up to 8.3; its loaders for 8.4 and 8.5 are for Apple silicon only. Devid does not offer it (D-025). |
| APM and profiler agents | New Relic, Datadog, Blackfire | Case by case: closed source, and most need their own background agent. Not planned for Phase 6. |

**Cannot be added, and why:**

| Reason | Examples |
|---|---|
| Needs a proprietary system library that Devid may not ship | oci8 / pdo_oci (Oracle Instant Client), sqlsrv / pdo_sqlsrv (Microsoft ODBC Driver) |
| Closed source and the license forbids redistribution | any vendor loader without a redistribution or official-download license |
| Needs thread-safe PHP (ZTS); Devid builds non-thread-safe PHP for php-fpm | parallel, pthreads |
| No support for PHP 8.3 or newer | Zend Guard Loader, pthreads |
| Windows only | com_dotnet |
| The vendor no longer builds it for Intel Macs | ionCube Loader: its Intel loaders stop at PHP 8.3 and are not maintained (D-025) |
| Clashes with a library already built into Devid's static PHP | none found in E1: imagick next to gd (each with its own libpng) and mongodb next to PHP's OpenSSL both work |

## Milestones

| # | Milestone | Done when |
|---|---|---|
| E1 | **Spike:** build apcu (no deps), mongodb (bundles libraries, uses OpenSSL), ldap (php-src, needs OpenLDAP) and imagick (external library) as shared extensions against Devid's static PHP 8.4 and 8.5; load each in the CLI and php-fpm, with Xdebug on and off. Check one closed-source loader's license and loading. Record results in PROJECT_CONTEXT and correct the table above. | each case is shown to work or the reason it cannot is known |
| E2 | **Catalog in `devid-binaries`:** CI builds the first extensions per PHP version, publishes them with checksums and adds the `extensions` section to `manifest.json`. | the manifest lists apcu (and the other E1 successes) for 8.3, 8.4 and 8.5. **Done 2026-11-04** (release `2026.11.04-6`; record in `docs/implementation-timeline/extensions/`) |
| E3 | **Daemon and CLI:** `devid php ext available / install / remove / list` with verify, load check and rollback; `devid update` and `doctor` cover catalog extensions. Vendor entries (license prompt, `source: vendor` downloads) wait until a vendor loader is accepted into the catalog; none is (D-025). | `devid php ext install 8.4 apcu` works on the owner's Mac; a failing extension is rolled back and PHP keeps running. **Done 2026-11-04** (v0.3.0; record in `docs/implementation-timeline/extensions/`) |
| E4 | **TUI:** the PHP page's Extensions section shows catalog extensions with install and remove (with confirmation). | the same from the TUI. **Done 2026-11-05** (v0.4.0; see "E4 design") |
| E5 | **Docs and release:** the README section "PHP extensions" with the tables above, a release. | `devid self-update` brings it to the owner's Mac |
| E6 | **Adding catalog entries later (GitHub Actions, `devid-binaries`):** a workflow that takes a new recipe (a PR or a manual run with the package name and version), builds it for every PHP version, runs the load and smoke tests, and publishes it with a new `manifest.json`. A scheduled job lists PIE candidates (darwin + NTS) and new releases of catalog entries for review; it never adds an entry by itself. | a new extension reaches the catalog through one reviewed recipe PR, without a Devid release |

### E3 design (accepted 2026-11-04)

What the daemon and CLI do with the catalog that E2 publishes (`manifest.json`, `extensions.<php minor>.<name>`; see the `devid-binaries` README).

**Commands.** The CLI's `devid php ext` keeps `devid php ext 8.5 xdebug on|off`. A first word of `available`, `install`, `remove` or `list` selects the new commands instead:

| Command | Request | What it does |
|---|---|---|
| `devid php ext available <ver>` | `PhpExtCatalog` | the catalog entries for that PHP version, marked installed or not |
| `devid php ext install <ver> <name>` | `PhpExtInstall` | see "Install" below |
| `devid php ext remove <ver> <name>` | `PhpExtRemove` | removes the ini, restarts php-fpm, then deletes the files |
| `devid php ext list <ver>` | `PhpExtList` | bundled extensions (`php -m`), Xdebug and pcov on or off, installed catalog extensions |

**Files.**
- The archive unpacks to `~/.devid/php-ext/<ver>/<name>/` (`<name>.so`, `ext.json`, `LICENSES/`, `etc/`), outside `~/.devid/php/<ver>/`. `install_php` replaces that folder on every PHP install or update and keeps only `etc/`, so files inside it would be deleted.
- Devid records the archive's SHA-256 next to it (`.sha256`). That lets `devid update` find a rebuild of the same version, for example one with a new OpenSSL.
- Turning an extension on writes `~/.devid/php/<ver>/etc/conf.d/devid-<name>.ini` (`extension=<absolute path>`), the same way Xdebug and pcov work.

**Environment** (imagick: `MAGICK_CONFIGURE_PATH`).
- php-fpm starts workers with a cleared environment (`clear_env = yes`), so the variable goes into the pool as `env[MAGICK_CONFIGURE_PATH] = …` in the generated `php-fpm.conf`.
- The `php` shim sets it too. It reads the `env` of each installed extension from its `ext.json`: a few small files, so the shim stays fast.

**Install.**
1. Read the manifest. Refuse when the entry is missing for that PHP version, or when its `php_api` differs from the installed PHP's ("run `devid update` first").
2. Download and verify with the code `devid update` uses (`releases::download`: the trusted prefix and SHA-256).
3. Unpack into a temporary folder, then rename it into place.
4. Write the ini, then a **preflight before touching the running PHP**: `php -m` and `php-fpm -m` with the real `php.ini` and `conf.d` must list the extension and print no warning or error.
   - If the preflight fails, Devid deletes the ini and the files and shows PHP's output. The running php-fpm was never restarted, so the sites never notice.
5. Only then regenerate `php-fpm.conf` (for `env[]`) and restart that version's php-fpm.

**`devid update`.** After it installs a new PHP build, it reinstalls every installed catalog extension of that version from the same manifest. With `--check`, it also lists extensions whose manifest SHA-256 differs from the recorded one. An extension that the new release no longer offers is turned off and reported, never left half-loaded.

**`devid doctor`.** For each installed PHP version, every installed catalog extension must load (`php -m`); otherwise the fix is `devid php ext install <ver> <name>` again or `remove`.

**Not in E3:**
- vendor entries (license prompt, `source: vendor`), because the catalog has none (D-025);
- the TUI, which is E4.

**Tests:**
- unit tests for reading catalog entries, the `php_api` check, the ini and the `env[]` lines;
- an integration test with a local `file://` release (as `tests/releases.rs` does), whose archives hold fake `.so` files and whose fake `php`/`php-fpm` list or refuse them. This covers the preflight, the rollback and the reinstall after `update`.

### E4 design (accepted 2026-11-05)

The owner chose these on 2026-11-05:
- **Several at once, removal one at a time.** The PHP menu's "Extensions…" lists the catalog for that version. `space` ticks extensions that are not installed. The Install button sits under a rule, and `enter` on it asks for confirmation, naming each extension and the download size. An installed row has no checkbox: `r` removes it after its own confirmation. `x` and `c` switch Xdebug and pcov from the same view.
- **A progress line, not a bar:** `installing 2/3 imagick… 52%`, then `checking …` and `restarting php-fpm …`. The percentage is real, not animated:
  - downloads count for 0–90 %, by bytes: the archive's size on disk against its size in the manifest;
  - each finished extension and the final restart share the last 10 %.
- **The daemon:**
  - `PhpExtInstall` takes several names and installs them one after another, each verified and preflighted on its own;
  - php-fpm restarts once at the end, and one failure does not undo the others; the reply has a result per name;
  - `PhpExtProgress` reports the running install; a second install is refused while one runs.
- **The CLI** takes several names too (`devid php ext install 8.5 apcu mongodb`, D-012). It prints the same progress line when stdout is a terminal, the summary, and each failure's full message.
- **Network use:** the catalog is fetched only when the Extensions view opens, never on the 2 s refresh. Nothing is downloaded until the owner confirms an install.

### E1 results (2026-11-03, 2026-11-04)

Script and method: `spike/ext/` (README). Built against the PHP build root of `spike/php` (8.5.12, then 8.4.27), loaded into the installed PHP of the same version from the Devid release (CLI and php-fpm, `-n -d extension=…`). Nothing was installed.

| | apcu 5.1.28 | mongodb 2.5.5 | ldap (php-src) | imagick 3.8.1 |
|---|---|---|---|---|
| Kind | no library | bundles its libraries | php-src, OpenLDAP 2.7.2 | ImageMagick 7.1.2-35 |
| Build time of the extension (2 cores) | 6–8 s | 3 min 53 s (8.5), 4 min 39 s (8.4) | 3 s | 10–16 s |
| Build time of its C library (spc `build:libs`, once per release) | none | none | 5.5 min (with gmp) | 29 min (libaom 9.3, libjxl 7.1, libheif 4.7, ImageMagick 4.1) |
| Size | 91 KB | 8.7 MB | 388 KB | 21 MB (20 MB stripped); 24 MB with SVG |
| Linked libraries (all in `/usr/lib`) | `libSystem` | `libSystem`, `libresolv` | `libSystem` | `libSystem`, `libresolv`, `libc++` |
| Loads in `php -m` / `php-fpm -m`, 8.5.12 and 8.4.27 | yes | yes | yes | yes |
| Works | store, fetch, inc | BSON; TLS verifies certificates and host names | LDAPS bind; refuses an unknown CA and a wrong host name | JPEG, PNG, GIF, WebP, AVIF, JPEG XL, TIFF: write, read, thumbnail; HEIC: read only; SVG (gradients: see below) |
| Together with Xdebug, the other three and PHP's own OpenSSL and gd | yes | yes | yes | yes |

Findings that shape E2 (the catalog build):
- **Symbols:** shared extensions use `-undefined dynamic_lookup` and take PHP's symbols from the static binary (146 for apcu, 358 for mongodb, 350 for ldap, 469 for imagick). This works, as it does for Xdebug. The `.so` files are flat-namespace bundles with lazy binding, so a missing symbol may fail only when the code reaches it. The spike checks that every undefined symbol is exported by `php` and `php-fpm` or by a system library the `.so` links: 0 missing for all eight builds, while the same check against a binary without PHP's exports finds 80–227. The catalog build must run this check.
- **Static libraries:** an extension built against a static library (libldap, libMagickWand) gets that library's own dependencies only through `LIBS=$(pkg-config --static --libs …)`; without it configure's link checks fail.
- **Which OpenSSL:** it depends on the extension's build system, not on Devid. mongodb links its own static OpenSSL into the `.so`. ldap links only libldap and liblber, so OpenSSL and zlib bind to PHP's copies at load time (113 OpenSSL symbols). Both work. Either way an extension only matches the PHP build of the same release, so every PHP update rebuilds the catalog, and an OpenSSL security fix means a new release of PHP and its extensions together.
- **CA trust:** mongodb's OpenSSL reads `/etc/ssl/cert.pem` (macOS system bundle), not `openssl.cafile`, and public TLS works. libldap uses PHP's OpenSSL, whose default bundle is the same file (a public LDAPS server was not tested). A server with a Devid certificate needs `tlsCAFile` (mongodb) or `LDAP_OPT_X_TLS_CACERTFILE` (ldap); document both.
- **Build paths leak:** mongodb.so and imagick.so have an `LC_RPATH` to the build root; OpenSSL's `ENGINESDIR`/`MODULESDIR` and ImageMagick's `CONFIGURE_PATH`, `CODER_PATH` and `SHARE_PATH` point there too. The catalog build must drop the rpath (`install_name_tool -delete_rpath`) and set neutral paths. For ImageMagick that matters: without its config files there is no `policy.xml` (security policy) and no `type.xml`, so text needs an explicit font file (`ImagickDraw::setFont('/System/Library/Fonts/Supplemental/Arial.ttf')` works; the default font fails).
- **imagick formats:** spc's recipe builds ImageMagick `--without-xml --without-zip --without-lzma --without-zstd --without-jbig --disable-openmp`, so SVG cannot be read. Rebuilt with `--with-xml` (libxml2 from the build root, 3.8 min), SVG works through ImageMagick's internal renderer (MSVG): shapes, paths, strokes, opacity, `viewBox` and density scaling. SVG `<text>` draws nothing without a `type.xml`. **Gradients (linear and radial) rendered black**, also as MVG and with ImageMagick's own `magick` command. See "SVG gradients" below for the cause and the patch. HEIC writing needs x265, which is GPL, so it stays out. Reading PDF needs Ghostscript (`gs`, AGPL), which Devid does not ship; writing PDF works.
- **imagick cost:** `php -r ''` takes 0.21–0.25 s warm with imagick loaded, against 0.06 s without it and 0.11 s with mongodb (cold: 0.61 s); RSS grows by 2 MB. php-fpm loads extensions once in the master, so requests to `.test` sites do not pay this, but every `php artisan` run does. AVIF, JPEG XL and HEIC (libaom, libjxl, libheif) are most of the size and the build time.
- **Test method:** public "bad certificate" hosts (badssl.com) gave random "connection closed" results, so TLS checks use a local server with a throwaway CA, which is deterministic: `openssl s_server` for mongodb, and for ldap a small Python TLS stub that answers one LDAP BindRequest.
- PHP 8.3 was not part of the four builds above; E2 builds it in CI.

#### SVG gradients (2026-11-04)

Two separate problems in ImageMagick's internal SVG renderer (MSVG), found with `spike/ext/svg-cases.sh`:

1. **Every `fill="url(#id)"` and `stroke="url(#id)"` drew black: fixed with a patch.**
   - Cause: before drawing, ImageMagick scans the MVG for named blocks. Any `push <kind> "name"` with a quoted name becomes a "macro", and the SVG coder writes every gradient and pattern that way (`push gradient "id" linear …`). `fill` and `stroke` look the macro up first and pass the macro's text, not its name, to `DrawPatternPath`, which finds no pattern by that name. The fill stays black.
   - The same MVG with an unquoted name draws the gradient, and so does a plain `push pattern`, which confirms the cause.
   - The code is the same from 7.1.1-30 to `main` (checked 2026-11-04). Reported upstream as ImageMagick/ImageMagick#9093 (2026-11-04), with the reproduction and the patch.
   - Fix: `spike/ext/imagemagick-svg-url-fill.patch` (two hunks in `MagickCore/draw.c`) looks for the pattern by name first and falls back to the macro. Rebuilding ImageMagick with it takes seconds.
   - Checked with `magick` and with imagick in PHP 8.4. Now correct: gradients with `gradientUnits="userSpaceOnUse"` or percent coordinates, `<pattern>` fills, gradient strokes. Plain fills are unchanged.
2. **Gradients in `objectBoundingBox` units are not implemented: a documented limit.**
   - `objectBoundingBox` is SVG's default unit for gradients: no `x1`/`x2`, a default radial gradient, or fractions from 0 to 1.
   - The SVG coder reads `gradientUnits` and writes `gradient-units`, but the drawing code ignores it (`draw.c`, keyword `gradient-units`). Such gradients come out as one colour or in the wrong direction.
   - In a sample of 545 SVG files on the owner's Mac, 13 have gradients: 3,154 gradient elements use `userSpaceOnUse` and 87 use `objectBoundingBox`. Still, 10 of those 13 files have at least one.
   - Options weighed on 2026-11-04: implement it in ImageMagick ourselves (hours to a day, then a patch to carry each release), or librsvg (full SVG, but Rust, cairo, pango, glib and more built static, with no spc recipe). The owner has no project that needs SVG now, so neither is done (D-024).

The catalog build applies the patch until upstream fixes the macro lookup.

#### Vendor loader: ionCube (2026-11-04)

- **License: case 1, mirroring allowed.** The Loader EULA (`ioncube.com/legal/loaders/eula`, the same text as `LICENSE.txt` in the package, last revised 2015-04-23):
  - 2.1: "The Loader may be freely distributed to third parties alone or as part of a distribution containing other items provided that this license is also included."
  - 2.2: it "may under no circumstances be branded as another product".
  - 3.1: no reverse engineering, and no modification of its behaviour.
  So `devid-binaries` may republish it unmodified with `LICENSE.txt`, under the name "ionCube Loader".
- **Intel Macs: PHP 8.3 only.** ionCube's download page lists the macOS x86-64 package as a "legacy" package that is no longer maintained. Its newest loader is 14.0.0 (2024-10-14), for PHP 4.4 to 8.3. Loader 15.5.1, with PHP 8.4 and 8.5, is built for macOS arm64 only. Devid's PHP 8.4 and 8.5 on the owner's Mac cannot use ionCube.
- **The vendor repacks the archive.** `ioncube_loaders_mac_x86-64.tar.gz` is dated 2026-09-18, while the loaders inside are from 2024-10-14. A pinned SHA-256 of the vendor's archive (case 2) would break on a repack that changes nothing. A mirrored copy (case 1) keeps its own checksum.
- **Loads** into the released PHP 8.3.36 (`devid-binaries` 2026.11.03-4, checksum matches `manifest.json`), in the CLI and php-fpm (`-v`: "with the ionCube PHP Loader v14.0.0"). It also loads next to OPcache. Startup and memory cost are negligible (0.04–0.06 s, +0.7 MB). It links only `libSystem`, and 0 of its 479 undefined symbols are missing.
- **Load order:** ionCube must be the first Zend extension. With Xdebug before it, PHP stops: "The Loader must appear as the first entry in the php.ini file". Ordered ionCube then Xdebug, both load. A vendor entry for it would need its ini to sort before Xdebug's.
- **Version check:** the 8.3 loader in PHP 8.4 is refused with a clear message (API420230831 against API420240924), so a wrong file cannot load silently.
- **Not tested:** running an encoded file, which needs the ionCube Encoder. Also whether loader 14.0 runs files from the newer V15 encoder. The download page only says it runs files from the V14 encoder.

Decided (D-025, 2026-11-04): ionCube is not offered. The owner does not use it, and on Intel it covers PHP 8.3 only with an unmaintained loader.

Decided for E2 (D-024, 2026-11-04):
- imagick keeps spc's full format set and adds SVG (`--with-xml`), with the gradient patch below. Gradients in `objectBoundingBox` units stay a documented limit; no own implementation, no librsvg.
- The catalog entry ships ImageMagick's default `policy.xml` and a `type.xml` with fonts that come with macOS. With `MAGICK_CONFIGURE_PATH` pointing at them, text with the default font and SVG `<text>` work (checked in the spike). ImageMagick's own `type-apple.xml` is out of date: 60 of its 193 font paths exist on macOS 14.

Each milestone is one worktree and one PR. After E6, new catalog entries are reviewed recipe PRs in `devid-binaries` only.

## Out of scope

- Installing a user-supplied `.so`, or compiling extensions on the user's Mac (D-021).
- Extensions on macOS arm64, Linux or Windows; they follow the platform work in D-019.
