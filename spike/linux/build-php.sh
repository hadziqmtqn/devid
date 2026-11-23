#!/bin/bash
# Phase 12 L1 (docs/roadmap/phase-12.md), questions 1 and 2: PHP 8.4 for Linux from
# static-php-cli's glibc target (zig), with Xdebug and pcov as .so files, plus apcu built
# separately against that PHP the way the catalog builds extensions.
#   spike/linux/build-php.sh   -> out/ (php, php-fpm, modules/*.so, BUILDINFO)
# Same extension set and pins as devid-binaries' scripts/build-php.sh (macOS).
set -euo pipefail

ROOT="$(pwd)"
WORK="${WORK:-$ROOT/work}"
OUT="${OUT:-$ROOT/out}"
SPC_VERSION=2.8.5
SPC_SHA256=523ba4279c54c7a377156c0dd3a36adf92ee64b01e9a7f5e9e2ec084b8e458e5   # GitHub's digest of spc-linux-x86_64.tar.gz
# glibc 2.17 is spc's documented example; the plan needs Debian 12 (2.36) and later.
export SPC_TARGET="${SPC_TARGET:-native-native-gnu.2.17}"
GLIBC_TARGET="x86_64-linux-gnu.${SPC_TARGET##*gnu.}"
PHP_VER=8.4
EXT="bcmath,ctype,curl,dom,exif,fileinfo,filter,gd,iconv,intl,mbstring,mysqli,opcache,openssl,pcntl,pdo,pdo_mysql,pdo_pgsql,pdo_sqlite,pgsql,phar,posix,readline,redis,session,simplexml,soap,sockets,sodium,sqlite3,tokenizer,xml,xmlreader,xmlwriter,zip,zlib"
SHARED="xdebug,pcov"
XDEBUG_URL="https://pecl.php.net/get/xdebug-3.5.3.tgz"
APCU_URL="https://pecl.php.net/get/apcu-5.1.28.tgz"
APCU_SHA256=ca9c1820810a168786f8048a4c3f8c9e3fd941407ad1553259fb2e30b5f057bf
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

# apcu the catalog's way (phpize against the build root), compiled by the same zig for the same glibc.
echo "::group::apcu"
ZIG="$(find "$WORK/pkgroot" -type f -name zig -perm -u+x | head -1)"
[ -n "$ZIG" ] || { echo "FAIL: no zig in $WORK/pkgroot" >&2; exit 1; }
mkdir -p ext && cd ext
curl -fsSL -o apcu.tgz "$APCU_URL"
echo "$APCU_SHA256  apcu.tgz" | sha256sum -c -
rm -rf apcu && mkdir apcu && tar xzf apcu.tgz -C apcu --strip-components 1
cd apcu
export PATH="$WORK/buildroot/bin:$PATH"
phpize
CC="$ZIG cc -target $GLIBC_TARGET" ./configure --with-php-config="$WORK/buildroot/bin/php-config"
make -j"$(nproc)"
cp modules/apcu.so "$OUT/modules/"
echo "::endgroup::"

cd "$OUT"
{
  echo "php $(./php -n -r 'echo PHP_VERSION;')"
  echo "spc $SPC_VERSION, SPC_TARGET=$SPC_TARGET, $("$ZIG" version | sed 's/^/zig /')"
  echo "build_seconds $build_seconds"
  echo "built on $(lsb_release -ds), $(ldd --version | head -1)"
} > BUILDINFO-php
cat BUILDINFO-php
