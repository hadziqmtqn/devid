#!/bin/bash
# Spike: measure the cost of a native static PHP build (cli + fpm) on this Mac.
# Usage: PHP_VER=8.4 ./build.sh   (default 8.5)
# Everything stays inside SPC_DIR; nothing is installed or put on PATH.
set -euo pipefail

PHP_VER="${PHP_VER:-8.5}"
# Build cache (spc, downloads, source, buildroot) is gitignored and lives in the main
# checkout, so a worktree can reuse it instead of rebuilding every library.
SPC_DIR="${SPC_DIR:-$(cd "$(dirname "$0")" && pwd)}"
cd "$SPC_DIR"

# Same list as devid-binaries scripts/build-php.sh (mysqli, readline, soap: D-022).
EXT="bcmath,ctype,curl,dom,exif,fileinfo,filter,gd,iconv,intl,mbstring,mysqli,opcache,openssl,pcntl,pdo,pdo_mysql,pdo_pgsql,pdo_sqlite,pgsql,phar,posix,readline,redis,session,simplexml,soap,sockets,sodium,sqlite3,tokenizer,xml,xmlreader,xmlwriter,zip,zlib"
# Loaded on demand as .so files (D-008), not compiled in.
SHARED="xdebug,pcov"
# Pin a stable Xdebug; spc's default source picked 3.6.0alpha1.
XDEBUG_URL="https://pecl.php.net/get/xdebug-3.5.3.tgz"

# spc fetches some sources via the GitHub API; a token avoids the anonymous rate limit.
export GITHUB_TOKEN="${GITHUB_TOKEN:-$(gh auth token 2>/dev/null || true)}"
# Keep Homebrew out of the build: no pkg-config paths, and SDKROOT makes clang/ld
# skip /usr/local/include and /usr/local/lib (Homebrew openssl@4 headers broke curl).
unset PKG_CONFIG_PATH
export SDKROOT="$(xcrun --show-sdk-path)"
# Compiled-in ini paths must not point at Homebrew's /usr/local/etc/php; Devid sets PHPRC anyway.
INI_DIR=/opt/devid/etc/php

RES="results/$PHP_VER"
OUT="out/$PHP_VER"
mkdir -p "$RES" "$OUT/modules"

echo "== download start $(date '+%F %T')" | tee "$RES/summary.txt"
# php-src and xdebug change per run; every other source is reused from the cache.
/usr/bin/time -l ./spc download --with-php="$PHP_VER" --for-extensions="$EXT,$SHARED" \
  --custom-url="xdebug:$XDEBUG_URL" --ignore-cache-sources=php-src,xdebug \
  2> >(tee "$RES/download-time.txt" >&2)

echo "== build start $(date '+%F %T')" | tee -a "$RES/summary.txt"
/usr/bin/time -l ./spc build "$EXT" --build-cli --build-fpm --build-shared="$SHARED" \
  --with-config-file-path="$INI_DIR" --with-config-file-scan-dir="$INI_DIR/conf.d" \
  2> >(tee "$RES/build-time.txt" >&2)
echo "== build end $(date '+%F %T')" | tee -a "$RES/summary.txt"

# buildroot is shared by all versions, so keep a per-version copy (mv keeps running binaries intact).
for f in php php-fpm; do cp "buildroot/bin/$f" "$OUT/$f.new" && mv -f "$OUT/$f.new" "$OUT/$f"; done
cp buildroot/modules/*.so "$OUT/modules/"

{
  echo "== sizes"; ls -lh "$OUT/php" "$OUT/php-fpm" "$OUT/modules/"
  echo "== linked libs (must be /usr/lib or /System only, no /usr/local)"
  otool -L "$OUT/php" "$OUT/php-fpm" "$OUT/modules/"*.so
  echo "== version"; "$OUT/php" -n -d zend_extension="$PWD/$OUT/modules/xdebug.so" -v
  echo "== modules"; "$OUT/php" -n -m
} | tee -a "$RES/summary.txt"
