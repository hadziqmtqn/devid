#!/bin/bash
# End-to-end check of a devid build with the real PHP and nginx from runtime-latest (D-044): a daemon
# in a throwaway DEVID_HOME on spare ports installs them, serves a linked site, and stops. The
# on-demand check also installs Caddy from the release and points Devid at the macOS httpd. It needs
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
  [ -n "${SERVER:-}" ] && kill "$SERVER" 2> /dev/null
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

# A proxy site (D-035) answers as soon as its dev server starts, whether that listens on 127.0.0.1
# or only on ::1 (as Vite does). nginx skips a localhost address that refused for 10 s, so up to
# 0.16 the site said "Nothing answers" for that long; the retry group fixes it (Phase 10 O2).
mkdir -p "$T/front"
echo "proxy smoke" > "$T/front/index.html"
"$DEVID" link --path "$T/front" > /dev/null
"$DEVID" app set front --mode proxy --upstream 28300 > /dev/null
front() { curl -sk "$@" --resolve front.test:28443:127.0.0.1 https://front.test:28443/ 2> /dev/null || true; }
down="Nothing answers on localhost:28300"
for _ in $(seq 50); do front | grep -q "$down" && break; sleep 0.2; done
for addr in 127.0.0.1 ::1; do
  front | grep -q "$down" || { echo "FAIL: front.test with no dev server: $(front)" >&2; exit 1; }
  if [ "$addr" = ::1 ]; then listen="[::1]:28300"; else listen="$addr:28300"; fi
  url="http://$listen/"
  # The dev server is PHP's built-in one, from the PHP installed above. `python3 -m http.server`
  # looks up its own name (getfqdn) before it listens, which took about 35 s on the Intel runner.
  "$DEVID_HOME/php/8.5/bin/php" -n -S "$listen" -t "$T/front" > /dev/null 2>&1 &
  SERVER=$!
  for _ in $(seq 100); do curl -fs -o /dev/null "$url" && break; sleep 0.05; done
  got="$(front)"
  [ "$got" = "proxy smoke" ] || { echo "FAIL: front.test right after its dev server started on $addr: $got" >&2; exit 1; }
  kill "$SERVER" && wait "$SERVER" 2> /dev/null || true
  SERVER=
done
echo "proxy smoke test passed on $(uname -m)"

# On demand (D-054): with nothing on port 28300, a request to front.test starts the command marked
# for it and gets its page; a page load to a command still starting after 5 s gets the Starting
# page; a command that exits shows its exit code.
php="$DEVID_HOME/php/8.5/bin/php"
"$DEVID" cmd add front dev -- "$php" -n -S '127.0.0.1:$PORT' -t . > /dev/null
"$DEVID" cmd add front slow -- "echo compiling; sleep 8; exec $php -n -S 127.0.0.1:\$PORT -t ." > /dev/null
"$DEVID" cmd add front broken -- 'echo smoke-broken; exit 3' > /dev/null
"$DEVID" cmd on-demand front dev > /dev/null
# nginx takes the new config a moment after the reply.
for _ in $(seq 50); do [ "$(front)" = "proxy smoke" ] && break; sleep 0.2; done
[ "$(front)" = "proxy smoke" ] || { echo "FAIL: on-demand front.test: $(front)" >&2; exit 1; }
"$DEVID" cmd front | grep 'on demand: running' > /dev/null || { echo "FAIL: dev is not running on demand" >&2; "$DEVID" cmd front; exit 1; }
grep -q 'started by a request to front.test' "$DEVID_HOME/log/app-front-dev.log" || { echo "FAIL: dev was not started by the request" >&2; exit 1; }
page() { curl -sk -H 'Accept: text/html' -w '\n%{http_code}' --resolve front.test:28443:127.0.0.1 https://front.test:28443/ 2> /dev/null || true; }
"$DEVID" cmd stop front dev > /dev/null
"$DEVID" cmd on-demand front slow > /dev/null
got="$(page)"
case "$got" in *"<title>Starting front.test…</title>"*compiling*$'\n'503) ;; *) echo "FAIL: no Starting page: $got" >&2; exit 1 ;; esac
for _ in $(seq 60); do [ "$(front)" = "proxy smoke" ] && break; sleep 0.5; done
[ "$(front)" = "proxy smoke" ] || { echo "FAIL: the slow command never answered: $(front)" >&2; exit 1; }
"$DEVID" cmd stop front slow > /dev/null
"$DEVID" cmd on-demand front broken > /dev/null
got="$(page)"
case "$got" in *"exited with code 3"*smoke-broken*$'\n'500) ;; *) echo "FAIL: no failure page: $got" >&2; exit 1 ;; esac
# Caddy (from the runtime release) and Apache (the macOS httpd) wake it too (Phase 10 O4). Apache
# answers 307 to the same URL once the dev server accepts; browsers follow it, curl with -L.
"$DEVID" cmd on-demand front dev > /dev/null
"$DEVID" server install caddy > /dev/null
"$DEVID" server install apache > /dev/null
for server in caddy apache; do
  "$DEVID" cmd stop front dev > /dev/null 2>&1 || true
  "$DEVID" server use "$server" > /dev/null
  for _ in $(seq 50); do [ "$(front -L)" = "proxy smoke" ] && break; sleep 0.2; done
  [ "$(front -L)" = "proxy smoke" ] || { echo "FAIL: on-demand front.test on $server: $(front -L)" >&2; exit 1; }
  "$DEVID" cmd front | grep 'on demand: running' > /dev/null || { echo "FAIL: $server did not wake dev" >&2; "$DEVID" cmd front; exit 1; }
done
"$DEVID" server use nginx > /dev/null
"$DEVID" cmd on-demand front off > /dev/null
echo "on-demand smoke test passed on nginx, caddy and apache on $(uname -m)"

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
