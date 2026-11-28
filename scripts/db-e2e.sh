#!/bin/bash
# Phase 14 (D-061): Devid's databases end to end, with the real builds from devid-binaries'
# db-latest. `devid db start` downloads each engine, sets up its data, runs it on 127.0.0.1 and gives
# Devid its own way in; each server answers its own client the way D-061 says (passwords and all).
# D3: `devid db user` makes accounts that use their own database over TCP with their password, gives
# one a new password and removes it while its data stays; `devid db port` moves a server. The
# servers come back after the daemon restarts, and `devid db stop` stops them. No sudo: it runs on a
# throwaway DEVID_HOME under /tmp, short enough for MySQL's socket path (103 bytes).
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
free_port() { python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }
# The password in the output of `devid db user add|password`.
pw_of() { sed -n 's/^password: //p'; }
# MongoDB's main account's password, from `devid db env mongodb`.
mongo_root() { "$DEVID" db env mongodb | sed -n 's|^DB_URI=mongodb://root:\([^@]*\)@.*|\1|p'; }

# As an account, over TCP with its password, in a database: as <engine> <port> <user> <password>
# <database> <statement>. MongoDB's accounts live in their database.
as_user() {
  case "$1" in
    mysql)
      printf '[client]\nuser=%s\npassword=%s\n' "$3" "$4" >"$DEVID_HOME/as.cnf"
      chmod 600 "$DEVID_HOME/as.cnf"
      "$bin/mysql" --defaults-file="$DEVID_HOME/as.cnf" -h127.0.0.1 -P"$2" -N -D "$5" -e "$6" ;;
    postgresql) PGPASSWORD="$4" "$bin/psql" -X -q -A -t -w -v ON_ERROR_STOP=1 -h 127.0.0.1 -p "$2" -U "$3" -d "$5" -c "$6" ;;
    mongodb) "$mongosh" --quiet --norc "mongodb://$3:$4@127.0.0.1:$2/$5?authSource=$5" --eval "$6" ;;
  esac
}
# What an app's first migration does: a table with a row, then count it.
migrate() {
  case "$1" in
    mongodb) echo 'db.migrations.insertOne({id: 1}); print(db.migrations.countDocuments())' ;;
    *) echo 'CREATE TABLE migrations (id INT PRIMARY KEY); INSERT INTO migrations VALUES (1); SELECT COUNT(*) FROM migrations' ;;
  esac
}
count() {
  case "$1" in
    mongodb) echo 'print(db.migrations.countDocuments())' ;;
    *) echo 'SELECT COUNT(*) FROM migrations' ;;
  esac
}
# The rows the main account sees in <database>.migrations: as <engine> <port> <database>.
as_main() {
  case "$1" in
    mysql) "$bin/mysql" --no-defaults -uroot -h127.0.0.1 -P"$2" -N -e "SELECT COUNT(*) FROM $3.migrations" ;;
    postgresql) "$bin/psql" -X -q -A -t -w -h 127.0.0.1 -p "$2" -U postgres -d "$3" -c 'SELECT COUNT(*) FROM migrations' ;;
    mongodb) "$mongosh" --quiet --norc "mongodb://root:$(mongo_root)@127.0.0.1:$2/admin" \
      --eval "print(db.getSiblingDB('$3').migrations.countDocuments())" ;;
  esac
}

# D-065 (Phase 16 S3): an account that may make databases, as a tenancy package needs, for one
# per tenant. On MySQL only those named <account>_…; on PostgreSQL any, which it then owns, while
# another account's database stays out of its reach. `off` takes the right back.
tenants() {
  local engine="$1" port="$2" tpw
  tpw="$("$DEVID" db user add "$engine" tenant --create-databases | pw_of)" ||
    fail "devid db user add $engine tenant --create-databases"
  [[ "$("$DEVID" db user list "$engine")" == *"tenant may make"* ]] || fail "$engine: db user list does not say so"
  as_user "$engine" "$port" tenant "$tpw" tenant "CREATE DATABASE tenant_t1" >/dev/null ||
    fail "$engine: tenant cannot make tenant_t1"
  [ "$(as_user "$engine" "$port" tenant "$tpw" tenant_t1 "$(migrate "$engine")")" = 1 ] ||
    fail "$engine: tenant cannot use tenant_t1"
  as_user "$engine" "$port" tenant "$tpw" tenant "DROP DATABASE tenant_t1" >/dev/null ||
    fail "$engine: tenant cannot drop tenant_t1"
  case "$engine" in
    mysql)
      ! as_user mysql "$port" tenant "$tpw" tenant "CREATE DATABASE other" >/dev/null 2>&1 ||
        fail "mysql: tenant made a database outside tenant_…"
      ! as_user mysql "$port" tenant "$tpw" tenant "SELECT COUNT(*) FROM shop.migrations" >/dev/null 2>&1 ||
        fail "mysql: tenant reads shop's database" ;;
    postgresql)
      ! as_user postgresql "$port" tenant "$tpw" tenant "DROP DATABASE shop" >/dev/null 2>&1 ||
        fail "postgresql: tenant dropped shop's database" ;;
  esac
  "$DEVID" db user create-databases "$engine" tenant off >/dev/null ||
    fail "devid db user create-databases $engine tenant off"
  ! as_user "$engine" "$port" tenant "$tpw" tenant "CREATE DATABASE tenant_t2" >/dev/null 2>&1 ||
    fail "$engine: tenant still makes databases after off"
  "$DEVID" db user remove "$engine" tenant >/dev/null || fail "devid db user remove $engine tenant"
  echo "ok: $engine: an account that may make databases (D-065)"
}

# D3 (D-061): accounts on a running server.
accounts() {
  local engine="$1" port="$2" out pw new bpw
  out="$("$DEVID" db user add "$engine" shop)" || fail "devid db user add $engine shop"
  pw="$(echo "$out" | pw_of)"
  [ "${#pw}" = 24 ] || fail "$engine: the generated password has ${#pw} characters"
  [ "$(as_user "$engine" "$port" shop "$pw" shop "$(migrate "$engine")")" = 1 ] ||
    fail "$engine: shop cannot use its own database"
  ! as_user "$engine" "$port" shop wrong-password shop "$(count "$engine")" >/dev/null 2>&1 ||
    fail "$engine: a wrong password got in"
  [[ "$("$DEVID" db env "$engine" shop)" == *"$pw"* ]] || fail "$engine: db env does not show shop's password"

  new="$("$DEVID" db user password "$engine" shop | pw_of)" || fail "devid db user password $engine shop"
  [ -n "$new" ] && [ "$new" != "$pw" ] || fail "$engine: no new password"
  ! as_user "$engine" "$port" shop "$pw" shop "$(count "$engine")" >/dev/null 2>&1 ||
    fail "$engine: the old password still works"
  [ "$(as_user "$engine" "$port" shop "$new" shop "$(count "$engine")")" = 1 ] || fail "$engine: the new password"

  if [ "$engine" != mongodb ]; then
    bpw="$("$DEVID" db user add "$engine" blog | pw_of)" || fail "devid db user add $engine blog"
    ! as_user "$engine" "$port" blog "$bpw" shop "$(count "$engine")" >/dev/null 2>&1 ||
      fail "$engine: blog reads shop's database"
    "$DEVID" db create "$engine" news >/dev/null || fail "devid db create $engine news"
    tenants "$engine" "$port"
  else
    ! "$DEVID" db user add mongodb tenant --create-databases >/dev/null 2>&1 ||
      fail "mongodb: an account was let make databases"
  fi

  [[ "$("$DEVID" db user list "$engine")" == *$'\n'"shop "* ]] || fail "$engine: db user list has no shop"
  "$DEVID" db user remove "$engine" shop >/dev/null || fail "devid db user remove $engine shop"
  ! as_user "$engine" "$port" shop "$new" shop "$(count "$engine")" >/dev/null 2>&1 ||
    fail "$engine: a removed account got in"
  [ "$(as_main "$engine" "$port" shop)" = 1 ] || fail "$engine: shop's data went with the account"
  echo "ok: $engine accounts: add, new password, remove (the data stays)"
}

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
      # A role of the test's own (not `shop`, which the accounts below make through Devid).
      probe="$(python3 -c 'import secrets; print(secrets.token_hex(8))')"
      psql -h 127.0.0.1 -p "$port" -U postgres -c "CREATE ROLE probe LOGIN PASSWORD '$probe'" || fail "create a role"
      ! psql -h 127.0.0.1 -p "$port" -U probe -d postgres -c 'SELECT 1' 2>/dev/null ||
        fail "another account got in without a password"
      [ "$(PGPASSWORD="$probe" psql -h 127.0.0.1 -p "$port" -U probe -d postgres -c 'SELECT 1')" = 1 ] ||
        fail "another account with its password"
      ;;
    mongodb)
      mongosh="$DEVID_HOME/db/mongosh/bin/mongosh"
      ok="$("$mongosh" --quiet --norc "mongodb://root:$(mongo_root)@127.0.0.1:$port/admin" \
        --eval 'print(db.runCommand({ping: 1}).ok)')" || fail "root with its password"
      [ "$ok" = 1 ] || fail "root's ping said $ok"
      ! "$mongosh" --quiet --norc "mongodb://127.0.0.1:$port/test" --eval 'db.t.find().toArray()' >/dev/null 2>&1 ||
        fail "a read without credentials"
      ;;
  esac
  echo "ok: $engine $(field "$engine" version) on 127.0.0.1:$port"
  accounts "$engine" "$port"
done

# `devid db port` moves a server: it restarts there, and only there.
old="$(field postgresql port)"
new="$(free_port)"
"$DEVID" db port postgresql "$new" >/dev/null || fail "devid db port postgresql $new"
[ "$(field postgresql state)" = running ] && listening "$new" || fail "PostgreSQL does not run on $new"
! listening "$old" || fail "PostgreSQL still listens on $old"
bin="$DEVID_HOME/db/postgresql-$(field postgresql series)/bin"
[ "$(as_main postgresql "$new" shop)" = 1 ] || fail "PostgreSQL's data after the move"
echo "ok: PostgreSQL moved from $old to $new"

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
