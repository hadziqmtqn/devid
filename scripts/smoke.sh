#!/bin/bash
# End-to-end check of a devid build with the real PHP and nginx from runtime-latest (D-044): a daemon
# in a throwaway DEVID_HOME on spare ports installs them, serves a linked site, and stops. It needs
# no sudo and leaves the system alone. release.yml runs it on each arch before publishing, since
# Apple silicon is tested on CI only.
# Usage: scripts/smoke.sh <path to devid>
set -euo pipefail
DEVID="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
# A short path: macOS limits the daemon's socket path to 104 bytes.
T="$(mktemp -d /tmp/dvsmoke.XXXXXX)"
export DEVID_HOME="$T/home"
mkdir -p "$DEVID_HOME" "$T/shop/public"
cat > "$DEVID_HOME/config.toml" <<'TOML'
http_port = 28088
https_port = 28443
dns_port = 28535
update_check_minutes = 0
TOML
echo '<?php echo "smoke ", PHP_VERSION, " ", php_uname("m");' > "$T/shop/public/index.php"

"$DEVID" daemon run > "$T/daemon.log" 2>&1 &
DAEMON=$!
finish() {
  status=$?
  "$DEVID" daemon stop > /dev/null 2>&1 || kill "$DAEMON" 2> /dev/null || true
  if [ "$status" -ne 0 ]; then
    echo "--- daemon.log" && cat "$T/daemon.log"
    for f in "$DEVID_HOME"/log/*.log; do [ -f "$f" ] && echo "--- $f" && tail -20 "$f"; done
  fi
  rm -rf "$T"
}
trap finish EXIT

for _ in $(seq 100); do "$DEVID" status > /dev/null 2>&1 && break; sleep 0.2; done
"$DEVID" status > /dev/null

"$DEVID" php install 8.5
"$DEVID" server install nginx
"$DEVID" link --path "$T/shop"

# php_uname("m") is x86_64 under Rosetta, so this also proves PHP runs natively.
want="smoke 8.5."
for _ in $(seq 50); do
  got="$(curl -fsSk --resolve shop.test:28443:127.0.0.1 https://shop.test:28443/ 2> /dev/null || true)"
  case "$got" in "$want"*) break ;; esac
  sleep 0.2
done
echo "got: $got"
case "$got" in
  "$want"*" $(uname -m)") echo "smoke test passed on $(uname -m)" ;;
  *) echo "FAIL: expected '$want<patch> $(uname -m)'" >&2; exit 1 ;;
esac

# Mail (D-049), once the runtime release has Mailpit: the real Mailpit on spare ports catches a test
# email and one from PHP's mail(), and refuses a foreign Host header (DNS rebinding).
"$DEVID" mail port 28525 > /dev/null
if out="$("$DEVID" mail on 2>&1)"; then
  "$DEVID" mail test
  PHPRC="$DEVID_HOME/php/8.5/etc" PHP_INI_SCAN_DIR="$DEVID_HOME/php/8.5/etc/conf.d" "$DEVID_HOME/php/8.5/bin/php" \
    -r 'mail("smoke@example.com", "PHP mail() smoke", "hello", "From: smoke@example.com") || exit(1);'
  for _ in $(seq 50); do "$DEVID" mail | grep -q 'messages:    2,' && break; sleep 0.2; done
  "$DEVID" mail
  "$DEVID" mail | grep -q 'messages:    2,' || { echo "FAIL: Mail did not catch both messages" >&2; exit 1; }
  code="$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: evil.example' http://127.0.0.1:28526/api/v1/info)"
  [ "$code" = 403 ] || { echo "FAIL: Mail's inbox answered a foreign Host with $code" >&2; exit 1; }
  # The inbox (Phase 9 M6): the list finds the test mail, and show reads its text.
  id="$("$DEVID" mail list | grep 'Test email from Devid' | grep -o -E '[A-Za-z0-9]{20,}' | head -1)"
  [ -n "$id" ] || { echo "FAIL: devid mail list has no test email" >&2; exit 1; }
  "$DEVID" mail show "$id" | grep -q 'If you can read this, Mail works.' || { echo "FAIL: devid mail show $id" >&2; exit 1; }
  echo "mail smoke test passed on $(uname -m)"
else
  case "$out" in
    *"has no Mailpit"*) echo "mail smoke test skipped: the runtime release has no Mailpit yet" ;;
    *) echo "$out" >&2; exit 1 ;;
  esac
fi
