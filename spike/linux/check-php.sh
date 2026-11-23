#!/bin/bash
# Phase 12 L1, questions 1 and 2: run the PHP from build-php.sh on this system (a runner or a
# distro container): cli, php-fpm, Xdebug, pcov and apcu as .so files, and the compiled-in set.
#   spike/linux/check-php.sh <dir with php, php-fpm, modules/>
set -uo pipefail
D="$(cd "${1:?usage: check-php.sh <dir>}" && pwd)"
fail=0
pass() { echo "PASS: $1"; }
miss() { echo "FAIL: $1"; [ -z "${2:-}" ] || sed 's/^/    /' <<<"$2" | head -8; fail=1; }

echo "on $(. /etc/os-release && echo "$PRETTY_NAME"), $(ldd --version 2>&1 | head -1)"

out="$("$D/php" -n -v 2>&1)" && pass "php runs" || miss "php runs" "$out"
out="$("$D/php" -n -d zend_extension="$D/modules/xdebug.so" -v 2>&1)"
grep -q "with Xdebug" <<<"$out" && pass "xdebug.so loads in php" || miss "xdebug.so loads in php" "$out"
out="$("$D/php" -n -d extension="$D/modules/pcov.so" -m 2>&1)"
grep -qx pcov <<<"$out" && pass "pcov.so loads in php" || miss "pcov.so loads in php" "$out"
if [ -f "$D/modules/apcu.so" ]; then
  out="$("$D/php" -n -d extension="$D/modules/apcu.so" -d apc.enable_cli=1 \
    -r 'apcu_store("k", 41); apcu_inc("k"); echo apcu_fetch("k");' 2>&1)"
  [ "$out" = 42 ] && pass "apcu.so (built with phpize) loads and works" || miss "apcu.so loads and works" "$out"
else
  echo "SKIP: no apcu.so (build-apcu.sh failed; see its step)"
fi
out="$("$D/php-fpm" -n -d zend_extension="$D/modules/xdebug.so" -m 2>&1)"
grep -q "^Xdebug" <<<"$out" && pass "xdebug.so loads in php-fpm" || miss "xdebug.so loads in php-fpm" "$out"

modules="$("$D/php" -n -m 2>&1)"
missing=""
for ext in pdo_pgsql pdo_sqlite intl gd redis sodium zip bcmath mysqli readline soap; do
  grep -qx "$ext" <<<"$modules" || missing="$missing $ext"
done
[ -z "$missing" ] && pass "compiled-in extensions present" || miss "compiled-in extensions missing:$missing"

exit $fail
