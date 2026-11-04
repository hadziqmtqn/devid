#!/bin/bash
# Phase 6 E1 spike: build a PECL extension as a shared .so against Devid's static PHP build root,
# then check that Devid's installed PHP (CLI and php-fpm) loads it. Nothing is installed.
# Usage: ./build.sh <pecl-name> [version] [configure args...]   (version default: PECL stable)
#        ./build.sh <php-src-ext> php-src [configure args...]   (ext/<name> of the build root's php-src)
set -euo pipefail

NAME="$1"
VER="${2:-}"
shift $(($# < 2 ? $# : 2))

# The PHP build root (headers, phpize, php-config) and spc's pkg-config are gitignored build
# cache in the main checkout (spike/php), shared with every worktree.
MAIN="${DEVID_MAIN:-$(git -C "$(dirname "$0")" worktree list --porcelain | awk 'NR==1{print $2}')}"
SPC="$MAIN/spike/php"
BR="$SPC/buildroot"
CACHE="$MAIN/spike/ext"

# Same isolation as spike/php/build.sh: no Homebrew headers, libraries or pkg-config paths.
unset PKG_CONFIG_PATH
export PKG_CONFIG_LIBDIR="$BR/lib/pkgconfig"
export SDKROOT="$(xcrun --show-sdk-path)"
export PATH="$BR/bin:$SPC/pkgroot/x86_64-darwin/bin:$PATH"
# The build root holds one PHP version at a time; test with the installed PHP of the same minor version.
MINOR="$(php-config --version | cut -d. -f1,2)"
PHP_HOME="${PHP_HOME:-$HOME/.devid/php/$MINOR}"

mkdir -p "$CACHE/downloads" "$CACHE/source"
if [ "$VER" = php-src ]; then
  VER="php-src-$(php-config --version)"
  SRC="$CACHE/source/$NAME-$VER"
  rm -rf "$SRC" && cp -R "$SPC/source/php-src/ext/$NAME" "$SRC"
  ORIGIN="ext/$NAME from $SPC/source/php-src"
else
  VER="${VER:-$(curl -fsSL "https://pecl.php.net/rest/r/$NAME/stable.txt")}"
  TGZ="$CACHE/downloads/$NAME-$VER.tgz"
  SRC="$CACHE/source/$NAME-$VER-php$MINOR"
  [ -f "$TGZ" ] || curl -fsSL -o "$TGZ" "https://pecl.php.net/get/$NAME-$VER.tgz"
  rm -rf "$SRC" "$CACHE/source/$NAME-$VER" && tar -xzf "$TGZ" -C "$CACHE/source" && mv "$CACHE/source/$NAME-$VER" "$SRC"
  ORIGIN="sha256 $(shasum -a 256 "$TGZ" | awk '{print $1}')  $(basename "$TGZ")"
fi
RES="$CACHE/results/$(basename "$SRC")"
mkdir -p "$RES"

{
  echo "== $NAME $VER  $(date '+%F %T')"
  echo "$ORIGIN"
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
  # The .so is a flat-namespace bundle: every undefined symbol is looked up at load time in PHP's
  # binary or a system library it links. One that is in neither fails the load, or crashes later.
  for b in php php-fpm; do
    missing="$(comm -23 <(nm -u "$SO" | sort -u) <(nm -gU "$PHP_HOME/bin/$b" | awk '{print $3}' | sort -u) |
      /usr/bin/python3 -I -c 'import ctypes, sys
libs = [ctypes.CDLL(p) for p in sys.argv[1:]]
# dyld_stub_binder (no leading underscore) is the lazy-binding helper of dyld itself.
print("\n".join(s for s in sys.stdin.read().split() if s.startswith("_") and not any(hasattr(l, s[1:]) for l in libs)))' \
      $(otool -L "$SO" | awk 'NR>1{print $1}'))"
    echo "== undefined in the .so, and neither $b nor its system libraries have it (must be 0): $(grep -c . <<<"$missing" || true) $(head -5 <<<"$missing" | tr '\n' ' ')"
  done
  echo "== installed PHP: $("$PHP_HOME/bin/php" -n -r 'echo PHP_VERSION;')"
  echo "== php -m";     "$PHP_HOME/bin/php" -n -d extension="$SO" -m 2>&1 | grep -iE "^$NAME$|warning|error" || echo "NOT LOADED"
  echo "== php-fpm -m"; "$PHP_HOME/bin/php-fpm" -n -d extension="$SO" -m 2>&1 | grep -iE "^$NAME$|warning|error" || echo "NOT LOADED"
} | tee -a "$RES/summary.txt"
echo "$SO"
