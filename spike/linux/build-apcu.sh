#!/bin/bash
# Phase 12 L1 (docs/roadmap/phase-12.md), question 2: apcu built the catalog's way (phpize against
# the build root that build-php.sh left in work/), with the same zig and glibc target.
#   spike/linux/build-apcu.sh   -> out/modules/apcu.so
set -euo pipefail

ROOT="$(pwd)"
WORK="${WORK:-$ROOT/work}"
OUT="${OUT:-$ROOT/out}"
SPC_TARGET="${SPC_TARGET:-x86_64-linux-gnu.2.17}"
GLIBC_TARGET="x86_64-linux-gnu.${SPC_TARGET##*gnu.}"
APCU_URL="https://pecl.php.net/get/apcu-5.1.28.tgz"
APCU_SHA256=ca9c1820810a168786f8048a4c3f8c9e3fd941407ad1553259fb2e30b5f057bf

cd "$WORK"
# apcu the catalog's way (phpize against the build root), compiled by the same zig for the same glibc.
ZIG="$(find "$WORK/pkgroot" -type f -name zig -perm -u+x | head -1)"
[ -n "$ZIG" ] || { echo "FAIL: no zig in $WORK/pkgroot" >&2; exit 1; }
mkdir -p ext && cd ext
curl -fsSL -o apcu.tgz "$APCU_URL"
echo "$APCU_SHA256  apcu.tgz" | sha256sum -c -
rm -rf apcu && mkdir apcu && tar xzf apcu.tgz -C apcu --strip-components 1
cd apcu
export PATH="$WORK/buildroot/bin:$PATH"
phpize
# zig makes __DATE__ an error (-Werror=date-time); apcu prints its build date.
CC="$ZIG cc -target $GLIBC_TARGET" CFLAGS="-O2 -Wno-date-time" ./configure --with-php-config="$WORK/buildroot/bin/php-config"
make -j"$(nproc)"
cp modules/apcu.so "$OUT/modules/"

"$ZIG" version | sed 's/^/zig /' >>"$OUT/BUILDINFO-php"
