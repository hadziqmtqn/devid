#!/bin/bash
# Phase 15 N1 spike: each official create tool with fixed options, no terminal, cold caches.
#   spike.sh <work> [template...]
# Per template: exit code, wall time and disk use of the project. For the Node templates it also
# checks that `npm run dev` answers on a port Devid picks. Results go to <work>/results.txt and
# what the tools print to <work>/logs. PHP, Composer and Node come from PATH.
set -u
W="$1"; shift
mkdir -p "$W/proj" "$W/logs" "$W/cache"
R="$W/results.txt"
PORT=4701
LIMIT=1200

run() { # name, command for bash -c; runs in $W/proj with its own empty caches and no stdin
  local name="$1" cmd="$2" t0 rc
  rm -rf "$W/proj/$name" "$W/cache/$name"
  mkdir -p "$W/cache/$name"
  t0=$(date +%s)
  (cd "$W/proj" && COMPOSER_CACHE_DIR="$W/cache/$name/composer" npm_config_cache="$W/cache/$name/npm" \
    WP_CLI_CACHE_DIR="$W/cache/$name/wp-cli" COMPOSER_NO_INTERACTION=1 \
    perl -e 'alarm shift; exec @ARGV' "$LIMIT" bash -c "$cmd") </dev/null >"$W/logs/$name.log" 2>&1
  rc=$?
  local secs=$(($(date +%s) - t0)) mb=-
  [ -d "$W/proj/$name" ] && mb=$(($(du -sk "$W/proj/$name" | cut -f1) / 1024))
  printf '%-14s rc=%-3s %4ss %6s MB\n' "$name" "$rc" "$secs" "$mb" | tee -a "$R"
}

dev() { # name, dev command with @PORT@: does it answer 200 on that port?
  local name="$1" cmd="$2" p=$PORT code=000
  PORT=$((PORT + 1))
  # its own process group, so the whole tree (npm, watchers, workers) stops at the end
  (cd "$W/proj/$name" && exec perl -e 'setpgrp; exec @ARGV' bash -c "${cmd//@PORT@/$p}") </dev/null >"$W/logs/$name.dev.log" 2>&1 &
  local pid=$! t0=$(date +%s)
  for _ in $(seq 120); do
    for host in 127.0.0.1 localhost; do
      code=$(curl -s -o /dev/null -w '%{http_code}' "http://$host:$p/" || true)
      [ "$code" = 200 ] && break 2
    done
    sleep 1
  done
  local secs=$(($(date +%s) - t0))
  kill -TERM -- -$pid 2>/dev/null
  sleep 1
  kill -KILL -- -$pid 2>/dev/null
  printf '%-14s dev on :%s -> %s in %ss\n' "$name" "$p" "$code" "$secs" | tee -a "$R"
}

wordpress() { # WP-CLI's phar against its published SHA-512, then WordPress from the tar.gz and the zip
  local v b
  v=$(curl -fsS https://api.github.com/repos/wp-cli/wp-cli/releases/latest | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"].lstrip("v"))')
  b=https://github.com/wp-cli/wp-cli/releases/download/v$v
  mkdir -p "$W/bin" && curl -fsSL -o "$W/bin/wp-cli.phar" "$b/wp-cli-$v.phar"
  local sum="shasum -a 512"; command -v sha512sum >/dev/null && sum=sha512sum
  [ "$(curl -fsSL "$b/wp-cli-$v.phar.sha512")" = "$($sum "$W/bin/wp-cli.phar" | cut -d' ' -f1)" ] &&
    echo "wp-cli $v     sha512 matches" | tee -a "$R" || echo "wp-cli $v     sha512 MISMATCH" | tee -a "$R"
  local wp="php -d error_reporting=24575 $W/bin/wp-cli.phar"
  run wp-targz "$wp core download --path=wp-targz && $wp core verify-checksums --path=wp-targz"
  local zip
  zip=$(curl -fsS https://api.wordpress.org/core/version-check/1.7/ | python3 -c 'import json,sys; print(json.load(sys.stdin)["offers"][0]["packages"]["full"])')
  run wp-zip "$wp core download $zip --path=wp-zip && $wp core verify-checksums --path=wp-zip"
}

ALL="laravel lk-react lk-vue lk-svelte lk-livewire symfony symfony-skel codeigniter vite-react vite-vue vite-svelte vite-vanilla next nuxt sveltekit astro express wordpress"
for t in ${@:-$ALL}; do
  case $t in
    laravel) run $t "composer create-project laravel/laravel $t --prefer-dist --no-interaction" ;;
    lk-*) run $t "composer create-project laravel/${t#lk-}-starter-kit $t --stability=dev --prefer-dist --no-interaction && cd $t && npm install && npm run build" ;;
    symfony) run $t "composer create-project symfony/skeleton $t --no-interaction && cd $t && composer require webapp --no-interaction" ;;
    symfony-skel) run $t "composer create-project symfony/skeleton $t --no-interaction" ;;
    codeigniter) run $t "composer create-project codeigniter4/appstarter $t --no-interaction" ;;
    vite-*) run $t "npm create vite@latest $t -- --template ${t#vite-}-ts --no-interactive --no-immediate && cd $t && npm install"
      dev $t "npm run dev -- --port @PORT@ --strictPort" ;;
    next) run $t "npx --yes create-next-app@latest $t --yes --use-npm"
      dev $t "npm run dev -- -p @PORT@" ;;
    nuxt) run $t "npm create nuxt@latest $t -- --template minimal --packageManager npm --no-gitInit --modules="
      dev $t "npm run dev -- --port @PORT@" ;;
    sveltekit) run $t "npx --yes sv@latest create $t --template minimal --types ts --no-add-ons --install npm"
      dev $t "npm run dev -- --port @PORT@ --strictPort" ;;
    astro) run $t "npm create astro@latest $t -- --yes --skip-houston"
      dev $t "npm run dev -- --port @PORT@" ;;
    express) run $t "mkdir $t && cd $t && npm init -y >/dev/null && npm pkg set type=module scripts.dev='node --watch server.js' && npm install express && printf '%s\n' \"import express from 'express';\" 'const app = express();' \"app.get('/', (req, res) => res.send('ok'));\" 'app.listen(process.env.PORT || 3000);' > server.js"
      dev $t "PORT=@PORT@ npm run dev" ;;
    wordpress) wordpress ;;
  esac
done
