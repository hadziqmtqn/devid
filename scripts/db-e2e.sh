#!/bin/bash
# Phase 14 (D-061): Devid's databases end to end, with the real builds from devid-binaries'
# db-latest. `devid db start` downloads each engine, sets up its data, runs it on 127.0.0.1 and gives
# Devid its own way in; each server answers its own client the way D-061 says (passwords and all);
# the servers come back after the daemon restarts, and `devid db stop` stops them. No sudo: it runs
# on a throwaway DEVID_HOME under /tmp, short enough for MySQL's socket path (103 bytes).
#   scripts/db-e2e.sh <path to devid>
set -euo pipefail
DEVID="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
DEVID_HOME="$(mktemp -d /tmp/dbe2e.XXXXXX)"
export DEVID_HOME
DAEMON=""

fail() {
  echo "FAIL: $*" >&2
  for f in "$DEVID_HOME"/log/*.log "$DEVID_HOME/daemon.log"; do
    [ -f "$f" ] && { echo "== $f" >&2; tail -25 "$f" >&2; }
  done
  exit 1
}
cleanup() {
  "$DEVID" daemon stop >/dev/null 2>&1 || true
  [ -n "$DAEMON" ] && wait "$DAEMON" 2>/dev/null || true
  rm -rf "$DEVID_HOME"
}
trap cleanup EXIT

start_daemon() {
  "$DEVID" daemon run >>"$DEVID_HOME/daemon.log" 2>&1 &
  DAEMON=$!
  for _ in $(seq 100); do "$DEVID" status >/dev/null 2>&1 && return 0; sleep 0.2; done
  fail "the daemon did not start"
}
stop_daemon() {
  "$DEVID" daemon stop >/dev/null || fail "devid daemon stop"
  wait "$DAEMON" 2>/dev/null || true
  DAEMON=""
}
# A field of one engine's entry in `devid status --json`.
field() {
  "$DEVID" status --json | python3 -c '
import json, sys
d = {x["engine"]: x for x in json.load(sys.stdin)["databases"]}[sys.argv[1]]
v = d.get(sys.argv[2])
print("" if v is None else v)' "$1" "$2"
}
listening() { python3 -c 'import socket, sys; s = socket.socket(); s.settimeout(1); sys.exit(s.connect_ex(("127.0.0.1", int(sys.argv[1]))))' "$1"; }
secret() { sed -n "s/^$2 = \"\(.*\)\"$/\1/p" "$1"; }

mkdir -p "$DEVID_HOME"
cat >"$DEVID_HOME/config.toml" <<EOF
http_port = 18088
https_port = 18443
dns_port = 15353
update_check_minutes = 0
EOF
start_daemon

for engine in mysql postgresql mongodb; do
  "$DEVID" db start "$engine" || fail "devid db start $engine"
  [ "$(field "$engine" state)" = running ] || fail "$engine is $(field "$engine" state)"
  port="$(field "$engine" port)"
  series="$(field "$engine" series)"
  bin="$DEVID_HOME/db/$engine-$series/bin"
  accounts="$DEVID_HOME/data/$engine-$series/accounts.toml"
  case "$engine" in
    mysql)
      who="$("$bin/mysql" --no-defaults -uroot -h127.0.0.1 -P"$port" -N -e 'SELECT CURRENT_USER()')" ||
        fail "root over TCP without a password"
      [ "$who" = root@localhost ] || fail "root over TCP is $who"
      settings="$("$bin/mysql" --no-defaults -uroot -h127.0.0.1 -P"$port" -N -e \
        'SELECT @@bind_address, @@log_bin, @@performance_schema')"
      [ "$(echo "$settings" | tr -s '\t ' ' ')" = "127.0.0.1 0 0" ] || fail "MySQL's settings: $settings"
      [ "$(stat -c %a "$accounts" 2>/dev/null || stat -f %Lp "$accounts")" = 600 ] || fail "accounts.toml is readable by others"
      printf '[client]\nuser=devid\npassword=%s\nsocket=%s\n' "$(secret "$accounts" devid)" "$DEVID_HOME/run/mysql.sock" \
        >"$DEVID_HOME/devid.cnf"
      chmod 600 "$DEVID_HOME/devid.cnf"
      who="$("$bin/mysql" --defaults-file="$DEVID_HOME/devid.cnf" -N -e 'SELECT CURRENT_USER()')" ||
        fail "Devid's own account over the socket"
      [ "$who" = devid@localhost ] || fail "Devid's own account is $who"
      ;;
    postgresql)
      psql() { "$bin/psql" -X -q -A -t -w -v ON_ERROR_STOP=1 "$@"; }
      [ "$(psql -h 127.0.0.1 -p "$port" -U postgres -c 'SELECT current_user')" = postgres ] ||
        fail "postgres over TCP without a password"
      [ "$(psql -h "$DEVID_HOME/run" -p "$port" -U postgres -c 'SELECT 1')" = 1 ] || fail "the socket"
      psql -h 127.0.0.1 -p "$port" -U postgres -c "CREATE ROLE shop LOGIN PASSWORD 'secret'" || fail "create a role"
      ! psql -h 127.0.0.1 -p "$port" -U shop -d postgres -c 'SELECT 1' 2>/dev/null ||
        fail "another account got in without a password"
      [ "$(PGPASSWORD=secret psql -h 127.0.0.1 -p "$port" -U shop -d postgres -c 'SELECT 1')" = 1 ] ||
        fail "another account with its password"
      ;;
    mongodb)
      mongosh="$DEVID_HOME/db/mongosh/bin/mongosh"
      ok="$("$mongosh" --quiet --norc "mongodb://root:$(secret "$accounts" root)@127.0.0.1:$port/admin" \
        --eval 'print(db.runCommand({ping: 1}).ok)')" || fail "root with its password"
      [ "$ok" = 1 ] || fail "root's ping said $ok"
      ! "$mongosh" --quiet --norc "mongodb://127.0.0.1:$port/test" --eval 'db.t.find().toArray()' >/dev/null 2>&1 ||
        fail "a read without credentials"
      ;;
  esac
  echo "ok: $engine $(field "$engine" version) on 127.0.0.1:$port"
done

# The daemon stops its servers, and the ones that are on start again with it.
ports=""
for engine in mysql postgresql mongodb; do ports="$ports $(field "$engine" port)"; done
stop_daemon
for port in $ports; do
  ! listening "$port" || fail "the server on $port still runs after the daemon stopped"
done
start_daemon
for engine in mysql postgresql mongodb; do
  for _ in $(seq 150); do [ "$(field "$engine" state)" = running ] && break; sleep 0.2; done
  [ "$(field "$engine" state)" = running ] || fail "$engine did not start again with the daemon"
done
echo "ok: all three started again with the daemon"

# `devid db stop` stops each one and keeps it off; the data stays.
for engine in mysql postgresql mongodb; do
  port="$(field "$engine" port)"
  series="$(field "$engine" series)"
  "$DEVID" db stop "$engine" >/dev/null || fail "devid db stop $engine"
  [ "$(field "$engine" state)" = off ] || fail "$engine is $(field "$engine" state) after db stop"
  ! listening "$port" || fail "$engine still listens on $port"
  [ -d "$DEVID_HOME/data/$engine-$series" ] || fail "$engine's data is gone"
done
echo "db-e2e: all checks passed"
