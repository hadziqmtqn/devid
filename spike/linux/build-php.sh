#!/bin/bash
# Phase 12 L1 (docs/roadmap/phase-12.md), questions 1 and 2: PHP 8.4 for Linux from
# static-php-cli's glibc target (zig), with Xdebug and pcov as .so files. build-apcu.sh then
# builds a catalog extension against it.
#   spike/linux/build-php.sh   -> out/ (php, php-fpm, modules/*.so, BUILDINFO)
# Same extension set and pins as devid-binaries' scripts/build-php.sh (macOS).
set -euo pipefail

ROOT="$(pwd)"
WORK="${WORK:-$ROOT/work}"
OUT="${OUT:-$ROOT/out}"
SPC_VERSION=2.8.5
SPC_SHA256=523ba4279c54c7a377156c0dd3a36adf92ee64b01e9a7f5e9e2ec084b8e458e5   # GitHub's digest of spc-linux-x86_64.tar.gz
# glibc 2.17 is spc's documented example; the plan needs Debian 12 (2.36) and later. The arch is
# x86_64, not "native": zig's native target uses the build machine's CPU features, and the first
# build died with SIGILL (exit 132) on runners with an older CPU.
export SPC_TARGET="${SPC_TARGET:-x86_64-linux-gnu.2.17}"
PHP_VER=8.4
EXT="bcmath,ctype,curl,dom,exif,fileinfo,filter,gd,iconv,intl,mbstring,mysqli,opcache,openssl,pcntl,pdo,pdo_mysql,pdo_pgsql,pdo_sqlite,pgsql,phar,posix,readline,redis,session,simplexml,soap,sockets,sodium,sqlite3,tokenizer,xml,xmlreader,xmlwriter,zip,zlib"
SHARED="xdebug,pcov"
XDEBUG_URL="https://pecl.php.net/get/xdebug-3.5.3.tgz"
INI_DIR=/opt/devid/etc/php

mkdir -p "$WORK" "$OUT/modules"
cd "$WORK"
if [ ! -x spc ]; then
  curl -fsSL -o spc.tar.gz "https://github.com/crazywhalecc/static-php-cli/releases/download/$SPC_VERSION/spc-linux-x86_64.tar.gz"
  echo "$SPC_SHA256  spc.tar.gz" | sha256sum -c -
  tar xzf spc.tar.gz spc && rm spc.tar.gz
fi

echo "::group::spc doctor"
./spc doctor --auto-fix
echo "::endgroup::"
echo "::group::spc download"
./spc download --with-php="$PHP_VER" --for-extensions="$EXT,$SHARED" \
  --custom-url="xdebug:$XDEBUG_URL" --ignore-cache-sources=php-src,xdebug --retry=3
echo "::endgroup::"
echo "::group::spc build"
start=$(date +%s)
./spc build "$EXT" --build-cli --build-fpm --build-shared="$SHARED" \
  --with-config-file-path="$INI_DIR" --with-config-file-scan-dir="$INI_DIR/conf.d"
build_seconds=$(( $(date +%s) - start ))
echo "::endgroup::"

cp buildroot/bin/php buildroot/bin/php-fpm "$OUT/"
cp buildroot/modules/xdebug.so buildroot/modules/pcov.so "$OUT/modules/"

cd "$OUT"
{
  echo "php $(./php -n -r 'echo PHP_VERSION;')"
  echo "spc $SPC_VERSION, SPC_TARGET=$SPC_TARGET"
  echo "build_seconds $build_seconds"
  echo "built on $(lsb_release -ds), $(ldd --version | head -1)"
} > BUILDINFO-php
cat BUILDINFO-php
