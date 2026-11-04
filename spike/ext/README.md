# Extension spike (Phase 6, E1)

Can a PHP extension built separately as a shared `.so` load into Devid's static PHP? This answers it per extension before any catalog or product code exists (D-021, `docs/roadmap/phase-6.md`).

## Run

```sh
./build.sh apcu                       # PECL stable version
./build.sh mongodb 2.5.5 --with-mongodb-ssl=openssl ...   # pinned version, extra configure args
./build.sh ldap php-src --with-ldap="$BR" "LIBS=$(pkg-config --static --libs ldap)"   # ext/ldap of the build root's php-src ($BR = spike/php/buildroot)
```

- An extension that needs a C library gets it from spc first, built static into the same build root: `cd spike/php && ./spc download --for-libs=ldap && ./spc build:libs ldap` (5.5 min with gmp), or `imagemagick` (29 min). For SVG, ImageMagick was then reconfigured by hand in `spike/php/source/imagemagick` with spc's arguments (`./config.status --config`) and `--with-xml` instead of `--without-xml` (D-024). `imagemagick-svg-url-fill.patch` fixes SVG gradient and pattern fills (`patch -p1` in the ImageMagick source, then `make install`); `svg-cases.sh` compares two `magick` binaries on gradient, pattern and stroke cases. Static libraries do not bring their own dependencies, so pass them with `LIBS=$(pkg-config --static --libs <lib>)`; otherwise configure's link checks fail.
- The `.so` is a flat-namespace bundle with lazy binding. The summary lists undefined symbols that neither PHP's binary nor a system library the `.so` links provides (checked with `dlsym`). That count must be 0, or the extension fails to load or crashes when it reaches the missing symbol.
- It builds against the PHP build root of `spike/php` (headers, `phpize`, `php-config`, static OpenSSL and libxml2) and uses spc's `pkg-config`. Like `spike/php/build.sh`, it keeps Homebrew out (`PKG_CONFIG_LIBDIR`, `SDKROOT`).
- It loads the result into Devid's installed PHP (`~/.devid/php/<the build root's minor version>`, override with `PHP_HOME`) with `php -n -d extension=…`, in the CLI and in php-fpm (`-m`). Nothing is installed and no site changes.
- Functional checks (apcu store/fetch; mongodb BSON and TLS against a local `openssl s_server` with a throwaway CA; ldap bind over ldaps against a small Python TLS stub that answers one BindRequest) were run by hand; their output is appended to the summary.
- Downloads, sources and results go to `spike/ext/{downloads,source,results}` in the main checkout (gitignored). `results/<name>-<version>/summary.txt` holds the sha256, build time, size, linked libraries, symbol counts and the load checks.

Building needs the Xcode Command Line Tools and autoconf. That is fine here: in the product, CI builds the catalog and users compile nothing.

## Results

See PROJECT_CONTEXT ("Phase 6 E1 spike") and the table in `docs/roadmap/phase-6.md`.
