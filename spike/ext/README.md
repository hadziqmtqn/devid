# Extension spike (Phase 6, E1)

Can a PHP extension built separately as a shared `.so` load into Devid's static PHP? This answers it per extension before any catalog or product code exists (D-021, `docs/roadmap/phase-6.md`).

## Run

```sh
./build.sh apcu                       # PECL stable version
./build.sh mongodb 2.5.5 --with-mongodb-ssl=openssl ...   # pinned version, extra configure args
```

- It builds against the PHP build root of `spike/php` (headers, `phpize`, `php-config`, static OpenSSL and libxml2) and uses spc's `pkg-config`. Like `spike/php/build.sh`, it keeps Homebrew out (`PKG_CONFIG_LIBDIR`, `SDKROOT`).
- It loads the result into Devid's installed PHP (`~/.devid/php/8.5`, override with `PHP_HOME`) with `php -n -d extension=…`, in the CLI and in php-fpm (`-m`). Nothing is installed and no site changes.
- Functional checks (apcu store/fetch; mongodb BSON and TLS against a local `openssl s_server` with a throwaway CA) were run by hand; their output is appended to the summary.
- Downloads, sources and results go to `spike/ext/{downloads,source,results}` in the main checkout (gitignored). `results/<name>-<version>/summary.txt` holds the sha256, build time, size, linked libraries, symbol counts and the load checks.

Building needs the Xcode Command Line Tools and autoconf. That is fine here: in the product, CI builds the catalog and users compile nothing.

## Results

See PROJECT_CONTEXT ("Phase 6 E1 spike") and the table in `docs/roadmap/phase-6.md`.
