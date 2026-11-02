#!/bin/bash
# Compare erdbpro-saas served by Caddy (:9443), nginx (:9444) and Apache (:9445).
# GET requests only. *.test is pinned with --resolve because /etc/resolver is not set up.
set -uo pipefail

MAIN=/Users/meowpush/Projects/devid/spike
CA="$MAIN/runtime/multi/ca/devid-ca.crt"
SITE=erdbpro-saas.test
APP=/Users/meowpush/Projects/erdbpro-saas
# A real built asset from the Vite manifest, to check static file serving and MIME types.
ASSET=$(python3 -c "import json;m=json.load(open('$APP/public/build/manifest.json'));print('build/'+next(v['file'] for v in m.values() if v['file'].endswith('.js')))")
CSS=$(python3 -c "import json;m=json.load(open('$APP/public/build/manifest.json'));f=next((f for v in m.values() for f in v.get('css',[])), '');print('build/'+f if f else '')")

req() { # port path -> "status|content-type|http-version|bytes|time"
  curl -s -o /dev/null --cacert "$CA" --resolve "$SITE:$1:127.0.0.1" \
    -w '%{http_code}|%{content_type}|h%{http_version}|%{size_download}B|%{time_total}s' "https://$SITE:$1$2"
}

printf '%-8s %-14s %s\n' server path result
for pair in caddy:9443 nginx:9444 apache:9445; do
  name=${pair%%:*}; port=${pair##*:}
  for path in / /login "/$ASSET" ${CSS:+"/$CSS"} /favicon.ico /.env /does-not-exist; do
    printf '%-8s %-14s %s\n' "$name" "${path:0:14}" "$(req "$port" "$path")"
  done
  # Inertia: the login page must render the app root with page data.
  body=$(curl -s --cacert "$CA" --resolve "$SITE:$port:127.0.0.1" "https://$SITE:$port/login")
  printf '%-8s %-14s %s\n' "$name" inertia "$(echo "$body" | grep -q 'data-page=' && echo 'data-page present' || echo 'MISSING data-page')"
done
