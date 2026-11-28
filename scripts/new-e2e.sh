#!/bin/bash
# Phase 15 N5 (D-063): `devid new` with the real create tools, for every template. A daemon in a
# throwaway DEVID_HOME on spare ports, with the shims and composer.phar a real install has, makes
# each template in a parked folder (Laravel's and Symfony's databases and kits included), and each
# site answers over HTTPS. A Node.js project's dev server starts on its first visit, through the
# proxy, and stops after the check. new-templates.yml runs it every week on macOS and Ubuntu, so a
# create tool whose options change shows within a week. No sudo; the system is left alone.
#   scripts/new-e2e.sh <path to devid> [project ...]
# The projects are the names in CASES; given names, only those run.
# SPARE_PORTS=1 keeps MySQL, PostgreSQL and the dev servers off their usual ports (3306, 5432, 3000,
# 4321, 5173), for a computer whose own servers use them. It moves each dev server with `devid app
# set`, which also makes the daemon listen for on-demand visits, so only a run without it shows
# that `devid new` alone does. NEW_E2E_LOGS=<dir> keeps the logs there.
set -euo pipefail
SRC="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
shift
T="$(cd "$(mktemp -d /tmp/dvnew.XXXXXX)" && pwd -P)"
export DEVID_HOME="$T/home"
BIN="$DEVID_HOME/bin"
DEVID="$BIN/devid"
HTTPS=18443
DAEMON=""

# name|devid new's arguments|status|text the page has|more checks
CASES=(
  "php|php|200|Hello from php, on PHP"
  "static|static|200|<h1>static</h1>"
  "laravel|laravel|200|Laravel"
  "codeigniter|codeigniter|200|CodeIgniter"
  "symfony|symfony|404|Welcome to Symfony"
  "skeleton|symfony --kit skeleton|404|Welcome to Symfony"
  "wordpress|wordpress|200|wp-content|wp"
  "wp-subdir|wordpress --multisite subdir|200|wp-content|network subdir"
  "wp-subdomain|wordpress --multisite subdomain|200|wp-content|network subdomain"
  "laravel-mysql|laravel --db mysql|200|Laravel|env DB_CONNECTION=mysql"
  "symfony-mysql|symfony --db mysql|404|Welcome to Symfony|sql"
  "laravel-pgsql|laravel --db pgsql|200|Laravel|env DB_CONNECTION=pgsql"
  "symfony-pgsql|symfony --db pgsql|404|Welcome to Symfony|sql"
  "kit-react|laravel --kit react|200|Laravel|build"
  "kit-vue|laravel --kit vue|200|Laravel|build"
  "kit-svelte|laravel --kit svelte|200|Laravel|build"
  "kit-livewire|laravel --kit livewire|200|Laravel|build"
  "react|react|200|/@vite/client|node"
  "vue|vue|200|/@vite/client|node"
  "svelte|svelte|200|/@vite/client|node"
  "vite|vite|200|/@vite/client|node"
  "sveltekit|sveltekit|200|__sveltekit|node"
  "next|next|200|/_next/|node"
  "nuxt|nuxt|200|/_nuxt/|node"
  "astro|astro|200|/@vite/client|node"
  "express|express|200|Hello from Express|node"
)

fail() {
  echo "FAIL: $*" >&2
  exit 1
}
cleanup() {
  local status=$?
  "$DEVID" daemon stop >/dev/null 2>&1 || true
  [ -z "$DAEMON" ] || wait "$DAEMON" 2>/dev/null || true
  if [ -n "${NEW_E2E_LOGS:-}" ]; then
    mkdir -p "$NEW_E2E_LOGS"
    cp -R "$DEVID_HOME/log/." "$T/daemon.log" "$T"/out-*.txt "$NEW_E2E_LOGS/" 2>/dev/null || true
  fi
  rm -rf "$T"
  exit "$status"
}
trap cleanup EXIT

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# visit <name> <status> <text> [path] [seconds]: ask https://<name>.test<path> until it answers
# <status> with <text> in the page. VISIT says what it answered first and how long that took.
visit() {
  local name="$1" want="$2" text="$3" path="${4:-/}" limit="${5:-60}" start=$SECONDS code seen=""
  while :; do
    code="$(curl -sk -L --max-time 75 -o "$T/page" -w '%{http_code}' \
      --resolve "$name.test:$HTTPS:127.0.0.1" "https://$name.test:$HTTPS$path" 2>/dev/null)" || true
    if [ "$code" = "$want" ] && grep -qF -- "$text" "$T/page"; then
      VISIT="${seen:+$seen, then }$code in $((SECONDS - start)) s"
      return 0
    fi
    case " $seen " in *" $code "*) ;; *) seen="${seen:+$seen }$code" ;; esac
    if [ $((SECONDS - start)) -ge "$limit" ]; then
      VISIT="$seen for $limit s"
      return 1
    fi
    sleep 1
  done
}

# The checks after a page: build (a kit's built assets), env <line> (in .env), sql (Symfony reaches
# its database), wp (WordPress's URL and login page), network <kind> (a WordPress network, D-065: a
# second site, its page and its admin, which sends the visitor to its login page), node (stop the
# dev server again).
more() {
  local name="$1" dir="$2" check="$3"
  case "$check" in
    "") ;;
    build) [ -f "$dir/public/build/manifest.json" ] || { echo "no public/build/manifest.json"; return 1; } ;;
    env\ *) grep -qx -- "${check#env }" "$dir/.env" || { echo ".env has no ${check#env }"; return 1; } ;;
    sql) (cd "$dir" && "$BIN/php" bin/console dbal:run-sql 'SELECT 1' >/dev/null) || { echo "dbal:run-sql failed"; return 1; } ;;
    wp)
      local url
      url="$(cd "$dir" && "$BIN/wp" option get siteurl)" || { echo "wp option get siteurl failed"; return 1; }
      [ "$url" = "https://$name.test:$HTTPS" ] || { echo "siteurl is $url"; return 1; }
      visit "$name" 200 user_login /wp-login.php || { echo "wp-login.php: $VISIT"; return 1; }
      ;;
    network\ *)
      (cd "$dir" && "$BIN/wp" site create --slug=shop2 --title=Shop2 >/dev/null) || { echo "wp site create failed"; return 1; }
      local host="$name" base=/shop2
      [ "$check" = "network subdomain" ] && { host="shop2.$name"; base=""; }
      visit "$host" 200 "Shop2" "$base/" || { echo "the second site: $VISIT"; return 1; }
      visit "$host" 200 user_login "$base/wp-admin/" || { echo "its admin: $VISIT"; return 1; }
      ;;
    node) "$DEVID" cmd stop "$name" dev >/dev/null || { echo "devid cmd stop $name dev failed"; return 1; } ;;
  esac
}

# A real install's binary, and its shims: projects' own scripts run `php` and `composer` (D-053).
# Hard links, as `devid setup` makes them, so PHP_BINARY comes back through the shim.
mkdir -p "$BIN" "$T/sites"
cp "$SRC" "$DEVID"
for shim in php composer wp; do ln "$DEVID" "$BIN/$shim"; done
cat >"$DEVID_HOME/config.toml" <<EOF
http_port = 18088
https_port = $HTTPS
dns_port = 15353
update_check_minutes = 0
EOF
if [ -n "${SPARE_PORTS:-}" ]; then
  printf '\n[databases.mysql]\nport = 13306\n\n[databases.postgresql]\nport = 15432\n' >>"$DEVID_HOME/config.toml"
fi

# The tools run as on a user's computer: some skip their prompts, or more, when they see CI.
env -u CI -u GITHUB_ACTIONS "$DEVID" daemon run >"$T/daemon.log" 2>&1 &
DAEMON=$!
for _ in $(seq 100); do "$DEVID" status >/dev/null 2>&1 && break; sleep 0.2; done
"$DEVID" status >/dev/null || fail "the daemon did not start: $(cat "$T/daemon.log")"
"$DEVID" php install 8.5 >/dev/null
"$DEVID" server install nginx >/dev/null
url=https://getcomposer.org/download/latest-stable/composer.phar
mkdir -p "$DEVID_HOME/composer"
curl -fsSL --retry 3 --proto =https -o "$DEVID_HOME/composer/composer.phar" "$url"
want="$(curl -fsSL --retry 3 --proto =https "$url.sha256sum" | cut -d' ' -f1)"
[ "$(sha256 "$DEVID_HOME/composer/composer.phar")" = "$want" ] || fail "composer.phar does not match its sha256"
"$DEVID" park "$T/sites" >/dev/null
echo "devid $("$DEVID" --version | cut -d' ' -f2), $("$BIN/php" -r 'echo "PHP ", PHP_VERSION;'), on $(uname -s) $(uname -m)"

ROWS=()
FAILED=()
port=15300
for case in "${CASES[@]}"; do
  IFS='|' read -r name args want text check <<<"$case"
  if [ $# -gt 0 ] && [[ " $* " != *" $name "* ]]; then continue; fi
  echo "== devid new $args $name"
  out="$T/out-$name.txt"
  start=$SECONDS
  # shellcheck disable=SC2086 # the arguments split on purpose
  if ! "$DEVID" new $args "$name" 2>&1 | tee "$out"; then
    ROWS+=("| $name | \`$args\` | $((SECONDS - start)) s | — | failed: devid new |")
    FAILED+=("$name")
    continue
  fi
  made=$((SECONDS - start))
  limit=60
  if [ "$check" = node ]; then
    limit=240
    if [ -n "${SPARE_PORTS:-}" ]; then
      "$DEVID" app set "$name" --upstream "$port" >/dev/null
      port=$((port + 1))
      # nginx reloads by SIGHUP, and its old worker may take one more request with the old port.
      sleep 1
    fi
  fi
  dir="$T/sites/$name"
  if ! visit "$name" "$want" "$text" / "$limit"; then
    echo "the page: $(head -c 600 "$T/page")"
    ROWS+=("| $name | \`$args\` | $made s | $VISIT | failed: the page |")
    FAILED+=("$name")
    [ "$check" != node ] || "$DEVID" cmd stop "$name" dev >/dev/null 2>&1 || true
    continue
  fi
  first="$VISIT"
  if ! why="$(more "$name" "$dir" "$check" 2>&1)"; then
    echo "$why"
    ROWS+=("| $name | \`$args\` | $made s | $first | failed: ${why//$'\n'/ } |")
    FAILED+=("$name")
    continue
  fi
  ROWS+=("| $name | \`$args\` | $made s | $first | ok |")
done

summary() {
  echo "| Project | \`devid new\` | Made in | First visit | Result |"
  echo "|---|---|---|---|---|"
  printf '%s\n' "${ROWS[@]}"
}
echo
summary
[ -z "${GITHUB_STEP_SUMMARY:-}" ] || { echo "### $(uname -s) $(uname -m)"; echo; summary; } >>"$GITHUB_STEP_SUMMARY"
if [ ${#FAILED[@]} -gt 0 ]; then
  for name in "${FAILED[@]}"; do
    for f in "$DEVID_HOME/log/new-$name.log" "$DEVID_HOME/log/app-$name-dev.log"; do
      [ -f "$f" ] && { echo "== $f"; tail -40 "$f"; }
    done
  done
  fail "${#FAILED[@]} of ${#ROWS[@]}: ${FAILED[*]}"
fi
echo "every project was made and answered (${#ROWS[@]})"
