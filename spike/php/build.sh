#!/bin/bash
# Spike: measure the cost of a native static PHP 8.5 build (cli + fpm) on this Mac.
# Everything stays inside this folder; nothing is installed or put on PATH.
set -euo pipefail
cd "$(dirname "$0")"

PHP_VER=8.5
EXT="bcmath,ctype,curl,dom,exif,fileinfo,filter,gd,iconv,intl,mbstring,opcache,openssl,pcntl,pdo,pdo_mysql,pdo_pgsql,pdo_sqlite,pgsql,phar,posix,redis,session,simplexml,sockets,sodium,sqlite3,tokenizer,xml,xmlreader,xmlwriter,zip,zlib"
# Loaded on demand as .so files (D-008), not compiled in.
SHARED="xdebug,pcov"

# spc fetches some sources via the GitHub API; a token avoids the anonymous rate limit.
export GITHUB_TOKEN="${GITHUB_TOKEN:-$(gh auth token 2>/dev/null || true)}"
# Keep Homebrew out of the build: no pkg-config paths, and SDKROOT makes clang/ld
# skip /usr/local/include and /usr/local/lib (Homebrew openssl@4 headers broke curl).
unset PKG_CONFIG_PATH
export SDKROOT="$(xcrun --show-sdk-path)"
# Compiled-in ini paths must not point at Homebrew's /usr/local/etc/php; Devid sets PHPRC anyway.
INI_DIR=/opt/devid/etc/php

mkdir -p results
[ "${SKIP_DOWNLOAD:-}" ] || {
echo "== download start $(date '+%F %T')" | tee results/summary.txt
/usr/bin/time -l ./spc download --with-php="$PHP_VER" --for-extensions="$EXT,$SHARED" \
  2> >(tee results/download-time.txt >&2)
}

echo "== build start $(date '+%F %T')" | tee -a results/summary.txt
/usr/bin/time -l ./spc build "$EXT" --build-cli --build-fpm --build-shared="$SHARED" \
  --with-config-file-path="$INI_DIR" --with-config-file-scan-dir="$INI_DIR/conf.d" \
  2> >(tee results/build-time.txt >&2)
echo "== build end $(date '+%F %T')" | tee -a results/summary.txt

{
  echo "== sizes"; du -sh downloads source buildroot 2>/dev/null; ls -lh buildroot/bin/php buildroot/bin/php-fpm buildroot/modules/ 2>/dev/null
  echo "== linked libs (must be /usr/lib or /System only, no /usr/local)"
  otool -L buildroot/bin/php buildroot/bin/php-fpm
  echo "== version"; buildroot/bin/php -n -v
  echo "== modules"; buildroot/bin/php -n -m
} | tee -a results/summary.txt
