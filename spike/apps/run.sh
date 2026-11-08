#!/bin/sh
# Phase 8 A1 spike (docs/roadmap/phase-8.md, "A1 results"): run a dev command the way the daemon
# will, put a throwaway nginx in front of it, then stop its process group.
# Usage: spike/apps/run.sh <project dir> '<command>' <port> <site name>
# Proxy ports: 18088 passes Host/Origin, 18089 rewrites Host, 18090 rewrites Host and own Origin.
set -eu
here=$(cd "$(dirname "$0")" && pwd); out=${TMPDIR:-/tmp}/devid-a1; mkdir -p "$out/logs" "$out/tmp"
nginx=${DEVID_NGINX:-$HOME/.devid/nginx/bin/nginx}
python3 -I "$here/run_group.py" "$1" "$2" "$3" "$out/cmd.log" | tee "$out/run.json"
sed -e "s#PREFIX#$out#g" -e "s/UPSTREAM/$3/g" -e "s/SITE/$4/g" "$here/nginx.conf.template" > "$out/nginx.conf"
"$nginx" -p "$out" -c "$out/nginx.conf" -e "$out/logs/error.log"
echo "proxy up; try: curl -H 'Host: $4.test' http://127.0.0.1:18090/   (Enter stops everything)"; read _
"$nginx" -p "$out" -c "$out/nginx.conf" -e "$out/logs/error.log" -s quit
python3 -I "$here/stop_group.py" "$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['pgid'])" "$out/run.json")"
