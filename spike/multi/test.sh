#!/bin/bash
# Checks for the setup.sh spike. Servers must already run (php-fpm 8.5, php-fpm 8.4, Caddy).
# No /etc/resolver yet, so *.test is pinned to 127.0.0.1 with --resolve / CURLOPT_RESOLVE.
set -uo pipefail

MAIN=/Users/meowpush/Projects/devid/spike
STATE="${STATE:-$MAIN/runtime/multi}"
PORT=8443
fail=0
check() { if [ "$2" = "$3" ]; then echo "PASS $1 ($2)"; else echo "FAIL $1: got '$2', want '$3'"; fail=1; fi; }

for pair in blog:8.5 api:8.4; do
  site=${pair%%:*}; want=${pair##*:}
  hdr=$(curl -s -o /dev/null -D - --cacert "$STATE/ca/devid-ca.crt" \
    --resolve "$site.test:$PORT:127.0.0.1" "https://$site.test:$PORT/")
  check "$site.test status" "$(echo "$hdr" | awk 'NR==1{print $2}')" 200
  check "$site.test php" "$(echo "$hdr" | tr -d '\r' | awk -F'PHP/' 'tolower($0) ~ /^x-powered-by/ {print substr($2,1,3)}')" "$want"
done

# Without the Devid CA the certificate must be rejected (proves verification is real).
curl -s -o /dev/null --resolve "blog.test:$PORT:127.0.0.1" "https://blog.test:$PORT/"
check "untrusted CA rejected (curl exit 60)" "$?" 60

# Site-to-site call from PHP 8.5, relying only on curl.cainfo from the Devid php.ini.
php85() { PHPRC="$STATE/etc" PHP_INI_SCAN_DIR="$STATE/etc/conf.d" "$MAIN/php/out/8.5/php" "$@"; }
code='$c = curl_init("https://api.test:'$PORT'/");
curl_setopt_array($c, [CURLOPT_RESOLVE => ["api.test:'$PORT':127.0.0.1"], CURLOPT_RETURNTRANSFER => true, CURLOPT_HEADER => true]);
$r = curl_exec($c);
echo $r === false ? "err:".curl_errno($c) : curl_getinfo($c, CURLINFO_RESPONSE_CODE)."|".(preg_match("#PHP/(\d\.\d)#", $r, $m) ? $m[1] : "?");'
check "php 8.5 -> api.test (Devid cacert)" "$(php85 -r "$code")" "200|8.4"
check "php 8.5 -> api.test without cacert" "$(php85 -d curl.cainfo= -d openssl.cafile= -r "$code")" "err:60"
check "php 8.5 -> public https" "$(php85 -r 'echo strlen(file_get_contents("https://www.php.net/")) > 0 ? "ok" : "fail";')" ok

exit $fail
