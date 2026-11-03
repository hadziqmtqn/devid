#!/bin/bash
# Phase 6 E1 spike: build a PECL extension as a shared .so against Devid's static PHP build root,
# then check that Devid's installed PHP (CLI and php-fpm) loads it. Nothing is installed.
# Usage: ./build.sh <pecl-name> [version] [configure args...]   (version default: PECL stable)
set -euo pipefail

NAME="$1"
VER="${2:-$(curl -fsSL "https://pecl.php.net/rest/r/$NAME/stable.txt")}"
shift $(($# < 2 ? $# : 2))

# The PHP build root (headers, phpize, php-config) and spc's pkg-config are gitignored build
# cache in the main checkout (spike/php), shared with every worktree.
MAIN="${DEVID_MAIN:-$(git -C "$(dirname "$0")" worktree list --porcelain | awk 'NR==1{print $2}')}"
SPC="$MAIN/spike/php"
BR="$SPC/buildroot"
CACHE="$MAIN/spike/ext"
PHP_HOME="${PHP_HOME:-$HOME/.devid/php/8.5}"

# Same isolation as spike/php/build.sh: no Homebrew headers, libraries or pkg-config paths.
unset PKG_CONFIG_PATH
export PKG_CONFIG_LIBDIR="$BR/lib/pkgconfig"
export SDKROOT="$(xcrun --show-sdk-path)"
export PATH="$BR/bin:$SPC/pkgroot/x86_64-darwin/bin:$PATH"

TGZ="$CACHE/downloads/$NAME-$VER.tgz"
SRC="$CACHE/source/$NAME-$VER"
RES="$CACHE/results/$NAME-$VER"
mkdir -p "$CACHE/downloads" "$CACHE/source" "$RES"
[ -f "$TGZ" ] || curl -fsSL -o "$TGZ" "https://pecl.php.net/get/$NAME-$VER.tgz"
rm -rf "$SRC" && tar -xzf "$TGZ" -C "$CACHE/source"

{
  echo "== $NAME $VER  $(date '+%F %T')"
  echo "sha256 $(shasum -a 256 "$TGZ" | awk '{print $1}')  $(basename "$TGZ")"
  echo "build root: $(php-config --version) $(php-config --extension-dir | sed 's#.*/##')"
  echo "configure args: $*"
} | tee "$RES/summary.txt"

cd "$SRC"
phpize >"$RES/phpize.log" 2>&1
./configure --with-php-config="$BR/bin/php-config" "$@" >"$RES/configure.log" 2>&1
/usr/bin/time -l make -j2 >"$RES/make.log" 2>"$RES/make-time.txt"
SO="$SRC/modules/$NAME.so"

{
  echo "== build time"; grep -E "real|maximum resident" "$RES/make-time.txt"
  echo "== size"; ls -lh "$SO" | awk '{print $5}'
  echo "== linked libs (must be /usr/lib or /System only)"; otool -L "$SO" | tail -n +2
  echo "== undefined symbols resolved from the php binary at load time: $(nm -u "$SO" | wc -l | tr -d ' ')"
  echo "== OpenSSL symbols left undefined (would bind to PHP's copy): $(nm -u "$SO" | grep -ciE '_(SSL|EVP|OPENSSL|X509|BIO)_' || true)"
  echo "== OpenSSL symbols defined inside the .so (own copy): $(nm -gU "$SO" | grep -ciE ' _(SSL|EVP|OPENSSL|X509|BIO)_' || true)"
  echo "== installed PHP: $("$PHP_HOME/bin/php" -n -r 'echo PHP_VERSION;')"
  echo "== php -m";     "$PHP_HOME/bin/php" -n -d extension="$SO" -m 2>&1 | grep -iE "^$NAME$|warning|error" || echo "NOT LOADED"
  echo "== php-fpm -m"; "$PHP_HOME/bin/php-fpm" -n -d extension="$SO" -m 2>&1 | grep -iE "^$NAME$|warning|error" || echo "NOT LOADED"
} | tee -a "$RES/summary.txt"
echo "$SO"
