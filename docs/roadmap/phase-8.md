# Phase 8 Plan: apps, commands and sharing

> Scope: app types detected per site (PHP, static, Node), a local port per site, user commands the daemon runs, and sharing a site through a quick tunnel (D-035, D-036, D-037)
> Verified against: 2026-11-08 (A6 Share built on branch `feat/a6-share`)
> Status: accepted 2026-11-08; A1 (spike) done, see "A1 results"; A2 released in 0.8.0, see "A2 as built"; A3 released in 0.9.0, see "A3 as built"; A4 released in 0.10.0, see "A4 as built"; A5 (desktop app, menu bar, TUI) built, see "A5 as built"; A6 Share built, see "A6 as built"; the devid-site docs and the release next

Today Devid serves every site as PHP through php-fpm, and the docroot is `public/` when it exists (`daemon::sites`). Everything else the owner runs is still started by hand in a terminal:

- `php artisan serve` for erdbpro-saas on `127.0.0.1:8000`, only because the Cloudflare tunnel targets that port. Devid already serves the same app at `https://erdbpro-saas.test`.
- `npm run dev:pg:local` in `~/Projects/erd-builder-pro`: Express with Vite middleware on port 3000, which the tunnel exposes as `erd-loc-test.bkn.my.id`.
- `npm run dev:tauri` in the same repository: API on 3100, Vite on 3098, and a Tauri window.
- Node apps on 3000, 3001 and 8080.

Phase 8 lets Devid start and stop these, and serve Node and static apps at `.test` like PHP sites, from the CLI, the TUI and the desktop app.

## Ground rules

- **Client rule (D-012):** detection, serving, commands and sharing live in the daemon. The CLI, the TUI and the desktop app only render state and send requests, and every action is a CLI subcommand.
- **Nothing runs by itself:**
  - Detection only reads files: a few `stat` calls plus `composer.json` and `package.json`.
  - Commands found in `package.json` or `composer.json` are suggestions. A command runs only after the user saves it and presses Start.
  - A cloned repository cannot make Devid run its code.
- **Resources (hard constraint):**
  - With no command or share running, the daemon's idle RAM and CPU stay as they are now.
  - Commands and shares are never started at login in this phase.
  - A command's output goes straight to its log file, with no pipe through the daemon and no reader thread.
- **Compatibility:** a site with no override and no detected type is served exactly as today: PHP, with `public/` as the docroot when it exists.
- **Owner's machine:** the system tunnel `com.cloudflare.cloudflared` is never touched. Ports 8000 and 3xxx move to Devid only when the owner does it by hand, and A3 lists the steps.

## Design

### App types (D-035)

A **serving mode** says what the web server does with a site. A **preset** is a framework label that fills in the defaults. Users can change both.

| Mode | The web server | Fields |
|---|---|---|
| `php` | serves files from `root` and passes `.php` to the site's php-fpm (today's behaviour) | `root` |
| `static` | serves files from `root`; with `spa = true`, unknown paths fall back to `/index.html` | `root`, `spa` |
| `proxy` | forwards everything, WebSocket upgrades included (for HMR), to `127.0.0.1:<upstream>`, with the headers in "Proxy headers" below | `upstream`, `pass_host` |

Detection runs in order, and the first match wins:

| Preset | Marker | Mode and defaults |
|---|---|---|
| Laravel | `artisan`, and `laravel/framework` in `composer.json` | php, `public/` |
| CodeIgniter 4 | `spark` | php, `public/` |
| CodeIgniter 3 | `system/CodeIgniter.php` or `system/core/CodeIgniter.php` | php, the folder |
| Symfony | `bin/console`, and `symfony/framework-bundle` in `composer.json` | php, `public/` |
| WordPress | `wp-config.php` or `wp-load.php` | php, the folder |
| PHP | `composer.json`, `index.php` or `public/index.php` | php, `public/` if it exists |
| Next.js | `next` in `package.json` | proxy, 3000 |
| Nuxt | `nuxt` in `package.json` | proxy, 3000 |
| Node.js | `express`, `fastify`, `koa` or `hono` in `package.json` | proxy, 3000 |
| Vite | `vite` in `package.json` | proxy, 5173; or static `dist/` with `spa = true` |
| Node.js | another `package.json` with a `dev` or `start` script | proxy, 3000 |
| Static | `index.html` only | static, the folder |
| (none) | none of the above | php, as today |

- **Upstream port:** a `PORT=` line in the project's `.env` beats the preset's default.
- **erd-builder-pro** has `express` and `vite`, so it is a Node server on port 3000, which is right. It is a test fixture for the detector.
- **Laravel with Vite** stays a Laravel site. Vite only serves assets through `public/hot`, and `npm run dev` becomes one of the site's commands.
- **Cost:** reading the markers of all 19 folders in `~/Projects` takes about 3 ms warm and 20 ms cold (A1, in Python). There is no cache. A2 adds one only if the Rust version costs more than 1 ms per `status` poll.

**Proxy headers** (A1 results):
- nginx sends `Host: localhost:<upstream>` to the dev server, plus `X-Forwarded-Host: <name>.test`, `X-Forwarded-Proto` and the WebSocket upgrade headers.
  - Without this, Vite 6.4 refuses the `.test` host (403 "Blocked request").
  - The same holds for erd-builder-pro, whose Vite middleware lists only its `.env` hosts in `allowedHosts`.
- An `Origin` equal to the site's own origin (`https://<name>.test` or `http://<name>.test`) becomes `http://localhost:<upstream>`. Any other `Origin` passes unchanged, so the dev server still refuses other sites.
  - Next.js 16 blocks its dev assets and HMR socket for any other origin.
- nginx only routes `<name>.test` to the site, so a DNS-rebinding page cannot use the rewritten `Host`.
- `pass_host = true` turns both rewrites off, for an app that builds links from `Host` and does not read `X-Forwarded-Host`.
- **Override:** in `config.toml`, under the site's name. A missing field falls back to detection.

```toml
[apps.erd-builder-pro]
preset = "node"        # optional; detected when missing
mode = "proxy"         # php | static | proxy
upstream = 3000        # proxy only
pass_host = false      # proxy only; true sends the .test Host and Origin unchanged
port = 3001            # optional local port, see D-037

[[apps.erd-builder-pro.commands]]
name = "dev:pg:local"
run = "npm run dev:pg:local"

[[apps.erd-builder-pro.commands]]
name = "tauri"
run = "npm run dev:tauri"
```

### Commands (D-036)

A site can have any number of named commands. The user starts and stops each one, or all of a site's commands at once.

- **Process:**
  - The daemon starts `$SHELL -c '<run>'` in the site's folder.
  - The environment is captured once per daemon start from the user's login shell, `$SHELL -l -c 'env -0'` (about 20 ms). The daemon's LaunchAgent alone has only `/usr/bin:/bin:/usr/sbin:/sbin`.
  - The shell is not interactive (`-i`): on the owner's Mac that took 5.9 s and printed gitstatus errors, because nvm and the prompt load from `~/.zshrc`.
  - Devid finds nvm itself (`$NVM_DIR`, else `~/.nvm`) and puts one Node version's `bin` first on `PATH`:
    - the version in the folder's `.nvmrc`;
    - otherwise `alias/default`;
    - in both cases the highest installed `versions/node/v<…>` that matches.
    - Without nvm, `PATH` stays as the login shell set it.
  - stdin is `/dev/null`. A command that asks a question fails, and its log says why. For example, `prisma db push` asks before dropping data.
  - Each command runs in its own session (`setsid`), so its children (npm → tsx → node, or tauri → cargo → the app) form one process group.
- **Stop:** SIGINT to the group, as Ctrl+C does, then SIGTERM after 5 s and SIGKILL after 10 s. Done means no process of the group is left.
- **Log:** `~/.devid/log/app-<site>-<name>.log`. It is emptied at each start and cut to its last 1 MB when it passes 10 MB. The existing Logs page and `devid logs` show it with no new code.
- **State:** `stopped`, `running` (pid, since) or `exited` (exit code, time). There is no automatic restart: dev servers watch their own files, and a broken build would only loop.
- **Lifetime:**
  - A daemon stop (`devid daemon stop`, logout) stops every command.
  - Each running group's pid is kept in `~/.devid/run/app-<site>-<name>.pid`. If the daemon crashed, the next daemon start stops the groups left behind, checking the pid file and the process start time.
- **GUI apps:** the daemon is a LaunchAgent in the user's `gui/<uid>` domain, so `npm run dev:tauri` can open its window. A1 proves it.
- **Suggestions:** `package.json` scripts whose names start with `dev`, `start` or `watch`, plus `php artisan queue:work`, `schedule:work` and `reverb:start` for Laravel. The user can also type any command.

### Local port (D-037)

A site can also be served on `http://127.0.0.1:<port>`, next to `https://<name>.test`. That is for tunnels and tools that need a fixed port.

- **Default:** no port. "Add port" with no number picks the first free port from 8000 up; the user can type another.
- **Allowed:**
  - between 1024 and 65535;
  - not Devid's own `http_port`, `https_port` or `dns_port`;
  - not another site's port.
- **Busy port:** before each render, the daemon tries to connect to the port over 127.0.0.1 and ::1. A bind test is not enough: on macOS, binding 127.0.0.1 succeeds next to a listener on `*`, and the web server would take that program's traffic (A3). A busy port is left out of the config, and `sites` names the process holding it (for example `php` 38571 = `artisan serve`). A busy port must never stop nginx, and so take down every site.
- **Plain HTTP only:** the tunnel's edge does TLS.
- **Every server:** nginx gets an extra `listen 127.0.0.1:<port>` server block for the site, Caddy an `http://127.0.0.1:<port>` block, and Apache a `Listen` plus a vhost.

### Share (D-037)

"Share" exposes one site through a Cloudflare quick tunnel, for showing work to someone.

- **Command:** the daemon runs `cloudflared tunnel --no-autoupdate --url http://127.0.0.1:<http_port> --http-host-header <name>.test`, reads the `https://….trycloudflare.com` URL from its output, and shows it.
- **Where cloudflared comes from:** an existing `cloudflared` binary (`/usr/local/bin/cloudflared` on the owner's Mac, not from Homebrew). If there is none, Devid says how to get one. Devid does not download it in this phase.
- **Lifetime:** one share per site, and it runs until Stop or a daemon stop. Its RAM cost exists only while it runs.
- **Labels:** the URL is random each time. A fixed hostname stays the job of the user's own tunnel together with the local port.
- **Limit:** apps that build URLs from `APP_URL` or ignore `X-Forwarded-Proto` (for example Laravel without trusted proxies) give the visitor wrong links. The docs say so; Devid does not rewrite responses.

## Daemon API

New requests, each with a CLI subcommand:

| Request | CLI |
|---|---|
| `sites` (extended in A2) | `devid app show [site]`: type (detected or set), how it is served, what the owner changed |
| `AppSet { site?, cwd, preset?, mode?, root?, upstream?, spa?, pass_host? }` (A2; `port` comes in A3) | `devid app set [site] --mode proxy --upstream 3000`; A3 adds `devid app port [site] [<port>\|off]` |
| `AppReset { site?, cwd }` | `devid app reset [site]` (back to detection) |
| `AppPort { site?, cwd, port? }` (A3) | `devid app port [site] [PORT\|off]`: without a port, the site keeps its own or gets the first free one from 8000; sent again, it checks a busy port anew |
| `CommandAdd { site, name, run }` / `CommandRemove { site, name }` | `devid cmd add <site> <name> -- <command>`, `devid cmd rm <site> <name>` |
| `CommandStart { site, name? }` / `CommandStop { site, name? }` | `devid cmd start <site> [name]`, `devid cmd stop <site> [name]` (no name = all of the site's commands) |
| `ShareStart { site }` / `ShareStop { site }` | `devid share <site>`, `devid share <site> --stop` |

`status` and `sites` gain each site's preset, mode, port (and a busy-port note), command states and share URL. Every config change goes through `State::mutate`, so a config the web server rejects is rolled back as today.

## Clients

- **TUI, Sites page:** the type column; a site's action menu gains type/mode, port, commands (start, stop, restart, log, add, remove) and share.
- **Desktop app:**
  - The Sites page shows each site's type.
  - A site opens a detail view with type, serving, port, commands with a Start/Stop button each, Start all and Stop all, "Add command" (name, command, suggestions), and Share with a copy button.
  - The view follows the prototype's look (D-026). The prototype has no such view, so A5 draws it in the same style first.
- **Menu bar:** the number of running commands and shares, and "Stop all commands".
- **Security note:** the web view gains Tauri commands that start user commands. That is no more than the user can do in Terminal, and the existing limits stay: bundled files only, a strict CSP, a fixed command list.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| A1 | **Spike** (see "Spike questions") | the answers and numbers are in this doc and in PROJECT_CONTEXT |
| A2 | **App types:** detection, presets, `[apps]` overrides, modes `php`/`static`/`proxy` in nginx, Caddy and Apache, `devid app` | erd-builder-pro, unignored, opens at `https://erd-builder-pro.test` through its dev server; erdbpro-saas and phpMyAdmin are served as before |
| A3 | **Local port:** `port` with the bind check on all three servers | erdbpro-saas is served by Devid on `127.0.0.1:8000` and the tunnel works with `artisan serve` stopped |
| A4 | **Commands:** add, remove, start, stop, start all, stop all, logs, states, leftover cleanup after a crash, `devid cmd` | both erd-builder-pro commands run from `devid cmd start erd-builder-pro`; stop leaves no `node`, `cargo` or Tauri process |
| A5 | **TUI and desktop app:** the site detail view and the menu-bar items | the owner runs and stops both commands from Devid.app, with no terminal |
| A6 | **Share, docs and release:** `devid share`, the docs on `devid-site` (D-034), the timeline record and a release | released and installed on the owner's Mac |

Each milestone is one worktree and one PR.

### Owner's migration (after A3 and A4)

1. Stop `php artisan serve` for erdbpro-saas, then `devid app port erdbpro-saas 8000`.
2. `devid unignore erd-builder-pro`, then add its two commands.
3. The Cloudflare dashboard stays as it is: `:8000` is now Devid, and `:3000` is still the `dev:pg:local` command, started from Devid.

### Spike questions (A1)

Measured on the owner's Mac (Intel, 2 cores, macOS 14.8):

- **Environment:** from the LaunchAgent, does the captured shell environment find `npm`, nvm's `node` and `~/.cargo/bin`? How long does the capture take with the owner's `.zshrc`? How should `.nvmrc` be honoured?
- **Process group:** do `npm run dev:pg:local` and `npm run dev:tauri` start from the daemon, and does stop leave nothing behind? Does the Tauri window open?
- **Proxy:**
  - Does Vite's HMR work over `wss://<name>.test` through nginx?
  - Vite 6.0.9 and 5.4.12 or later refuse a `Host` they do not know: should the Vite preset send `Host: localhost`, or ask the user to add `.test` to `server.allowedHosts`?
  - The same for Next.js `allowedDevOrigins`.
- **Laravel and Vite:** on an HTTPS `.test` page, do Chrome and Safari load `http://localhost:5173` assets, or does Vite need Devid's certificate?
- **Cost:**
  - detection over `~/Projects` (19 folders), cold and cached;
  - the daemon's RSS idle, and with two commands running;
  - cloudflared's RSS while sharing.

### A1 results (2026-11-08)

Run on the owner's Mac with the scripts in `spike/apps/` (`run.sh` ties them together). The Python harness starts a command the way the daemon will:
- the login-shell environment plus nvm;
- its own session;
- stdin `/dev/null`;
- output to a log file.

The proxy tests used a throwaway nginx (Devid's 1.30 binary, its own prefix and ports 18088–18090). The owner's running `dev:pg:local` (port 3000), erd-selfhost-local (3001) and `artisan serve` (8000) were left alone.

- **Environment from launchd:**
  - A transient `launchctl submit` job (same `gui/<uid>` domain as the daemon) sees only `/usr/bin:/bin:/usr/sbin:/sbin`.
  - `zsh -l -c` gives `cargo` but no nvm, in 217 ms there and 10–26 ms elsewhere.
  - `zsh -l -i -c` gives everything, but takes 5.9 s and prints gitstatus errors.
  - Decision: `-l -c`, plus Devid's own nvm lookup (see "Commands"). nvm's `alias/default` is `24`, which resolves to `v24.20.0`.
- **GUI from launchd:** a Tk window started by a `launchctl submit` job maps on screen (`viewable=1`). So `npm run dev:tauri` can open its window from the daemon.
  - Calculator was killed (status 9) when started that way, but it is a sandboxed App Store app, not a dev build.
- **Process group:** SIGINT to the group stops everything on the first signal, and the port closes:
  - `npm run dev` of erdbpro-saas-template (npm, node and esbuild);
  - `next dev` of devid-site (npm, node, next-server and two node workers).
  - SIGTERM and SIGKILL were never needed.
- **Start time:**
  - Vite 6.4.3 is ready in 2.1 s.
  - Next.js 16.4.2 with Turbopack is ready in 2.4 s, and the first page takes 15 s to compile.
- **Cost of a running dev server:** the Next.js group uses 941 MB RSS. That is why commands never start by themselves.
- **Vite through nginx:**
  - With the `.test` Host: 403 "Blocked request".
  - With `Host: localhost:<port>`: 200, and the HMR WebSocket (with Vite's token) upgrades (101).
  - Vite's client opens its socket to the page's own host and port, so `wss://<name>.test` goes through nginx.
- **Next.js through nginx:**
  - The page loads either way.
  - Dev assets and the HMR socket (`/_next/hmr`) are blocked (403 or closed) unless `Origin` is rewritten as well. With the rewrite they return 200 and 101.
  - A foreign `Origin` (`https://evil.example`) is still blocked: 403, and no upgrade.
- **Laravel with Vite:**
  - laravel-vite-plugin 3.1.3 allows CORS from `*.test` by default (its Valet and Herd rule), so `https://<name>.test` may load assets from Vite on localhost.
  - Not run: `npm run dev` in erdbpro-saas would write `public/hot` while the owner's tunnel serves that app.
  - Still open for A2, with the owner in a browser: whether Safari loads `http://localhost` assets into an HTTPS page. Chrome treats loopback as secure.
- **Detection:** the order in "App types" classifies the 19 folders in `~/Projects` as expected:
  - erd-builder-pro is a Node server;
  - phpmyadmin is PHP, even though it has a `package.json`;
  - devid-site is Next.js and erdbpro-saas-template is Vite;
  - Rust and docs repositories match nothing.
- **erd-builder-pro's `dev:pg:local` on another port:**
  - The owner's own instance kept port 3000, so the command ran as `PORT=3010 npm run dev:pg:local` with the same harness. A command is a shell line, so `PORT=…` in it is enough.
  - The Prisma schema was unchanged, so `db push` changed nothing. Each schema variant generates its runtime client into its own folder (`@erdbpro/prisma-pg-local`, `@erdbpro/prisma-sqlite`).
  - Ready in 14 s, at 784 MB RSS for the group.
  - Through nginx with the `.test` Host: 403, because its Vite middleware lists only the `.env` hosts in `allowedHosts`. With Devid's proxy headers: 200 for the page and `@vite/client`.
  - One SIGINT stopped all six processes (npm, sh, three node, esbuild), and ports 3010 and 24678 closed. The owner's instance on 3000 kept running, and the repository was unchanged.
  - **HMR limit, in the app and not in Devid:**
    - Vite in middleware mode, with no `hmr.server`, runs its HMR socket on its own port, 24678.
    - An HTTPS page tries `wss://<name>.test:24678`, where there is no TLS. The socket also refuses the `.test` host (400).
    - The page works, but without hot reload.
    - The fix is in the app: pass the HTTP server as `server.hmr.server`, so HMR shares the app's port and goes through nginx. The docs (A6) mention it for Express with Vite middleware.
- **Later milestones:**
  - Running both erd-builder-pro commands from the daemon is A4's acceptance test. `dev:pg:local` can use another port (as above) while the owner's own instance holds 3000.
  - `npm run dev:tauri` was not run in A1. It deletes `src-tauri/target/debug` and rebuilds Tauri from scratch (G1: 8.5 min cold debug on this Mac), so it waits until the owner can spare the CPU. The GUI probe above covers the window question.
  - The daemon's RSS with commands running is measured in A4.
  - cloudflared's RSS while sharing is measured in A6. A test opens a public URL, so it waits for the owner.

### A2 as built (2026-11-08)

- **Config:** `[apps.<site>]` in `config.toml` holds only the overrides: `preset`, `mode`, `root`, `upstream`, `spa` and `pass_host`. An empty table is removed.
- **Detection (`sites::detect`):** the order in "App types". "Node server" and "Node" became one preset, `node` ("Node.js"), because they behave the same.
- **Resolving a site:**
  - the owner's preset, else the detected one, else `php`;
  - then the mode;
  - the web root: an override (relative, no `..`), else `public/` when it exists (php), the preset's build folder (static: Vite `dist`, Next.js `out`, Nuxt `.output/public`) or the folder itself;
  - the upstream: an override, else `PORT=` in `.env` (proxy sites only), else the preset's port;
  - `spa` is on for Vite.
- **`AppSet`:**
  - A new preset without a mode drops the old mode, root and upstream.
  - Choosing the detected preset stores nothing.
  - An upstream of 0, or Devid's own HTTP or HTTPS port (a proxy loop), is refused.
- **php-fpm:** only PHP-mode sites count when the daemon decides which PHP versions to run, so a static or proxy site's PHP version starts nothing.
- **nginx:**
  - Static sites use `try_files $uri $uri.html $uri/ =404` (or `/index.html` with `spa`).
  - Proxy sites use `proxy_pass http://localhost:<up>`, with the A1 headers through a per-site `map` of the site's own origins (with and without Devid's ports).
  - Also on proxy sites: `proxy_buffering off` (SSE), `proxy_read_timeout 1h` (idle HMR sockets), and a plain-text 502 that names the port when nothing answers.
- **Caddy:**
  - Static: `try_files {path} {path}.html [/index.html]`.
  - Proxy: `reverse_proxy localhost:<up>` with `header_up Host`, a `header_regexp` matcher with `request_header` for the own Origin, and `handle_errors 502`.
- **Apache:**
  - `mod_proxy_http` with `ProxyPass … upgrade=websocket retry=0`. The `retry=0` means a dev server that was down is tried again at once.
  - `RequestHeader` sets Host, the own Origin (an `expr` condition) and the `X-Forwarded-*` headers. Devid sets them itself (`ProxyAddHeaders Off`).
  - Static sites: a rewrite for `.html` and `FallbackResource` for SPA.
- **Fixed on the way, Apache:** hidden folders were served. `FilesMatch` checks only the file name, so `/.git/config` returned 200 on PHP and static sites. Now a per-`<Directory>` rewrite refuses `(^|/)\.` (except `.well-known`). It is not used on proxy sites, whose dev servers serve `/node_modules/.vite/…`.
- **Vite binds `[::1]` only:** without `--host`, Vite 6.4 listened on `[::1]:5198` and refused `127.0.0.1` (checked 2026-11-08). So every server proxies to `localhost`, which they all try over both IPv4 and IPv6.
- **CLI:**
  - `devid app show|set|reset [site]`.
  - `devid sites` gains a type column, and shows "static dist/" or "proxy to localhost:3000" instead of a PHP version for those modes.
- **Version:** 0.8.0, since the protocol and the CLI grow.

**Verification:**
- **Checks:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. New unit tests cover detection and overrides (`sites.rs`), and static and proxy rendering for nginx, Caddy and Apache. A new integration test (`crates/cli/tests/sites.rs`, `app_types_and_overrides`) covers detection, `app set/reset/show`, validation, and php-fpm starting only for PHP sites.
- **A second daemon on the owner's Mac:**
  - Setup: its own `DEVID_HOME`, ports 18188/18543, nginx and a copy of PHP 8.4. It parked the real mikurkid-lp, erdbpro-saas-template, erd-builder-pro, devid-site and erdbpro-saas, plus three scratch projects (PHP, a built SPA, and a header-echo server).
  - It detected all eight sites as expected, and the same checks passed on nginx, Apache and Caddy:
    - PHP;
    - static: `/`, `/profil` from `profil.html`, 404, and 403 for `/.git/config` and a hidden folder;
    - SPA fallback;
    - proxy headers: `Host: localhost:<up>`, the own Origin rewritten, a foreign Origin kept, `X-Forwarded-Host` and `X-Forwarded-Proto`;
    - Vite on `[::1]` (200) and its HMR WebSocket over `wss` (101);
    - the "nothing answers" page (nginx and Caddy 502, Apache 503).
  - The erdbpro-saas server block in the new nginx.conf is identical to the live one, paths and ports aside.
  - The owner's daemon, sites, php-fpm and dev servers were not touched.
- **Cost:** `devid sites` with eight sites takes 24 ms per call (debug build, including the CLI's own start), so detection needs no cache.
- **Seen once, not a bug:**
  - devid-site's preset port (3000) reached the owner's running erd-builder-pro, a port clash that the override solves.
  - A request sent right after `devid app set` can still meet the old nginx config, because the reload is a SIGHUP and the reply does not wait for it. That has always been so for every config change.

**Still open:**
- On the owner's Mac after the 0.8.0 release:
  - `devid unignore erd-builder-pro`, then `https://erd-builder-pro.test` through the owner's dev server on 3000;
  - Laravel with Vite in Safari.
- The TUI and the desktop app show the new fields in A5. Until then, their Sites pages still show a PHP version for every site.
- The user docs on devid-site follow with the release (D-034).

### A3 as built (2026-11-08)

- **Config:** `port` in `[apps.<site>]`.
- **`AppPort` request:** a port, `Some(0)` for off, or None, which keeps the site's own port or picks the first free one from 8000 to 8999.
- **Refused ports:**
  - below 1024;
  - Devid's own HTTP, HTTPS and DNS ports;
  - another site's port;
  - any proxy site's dev server port (on the site itself, a proxy loop).
- **A program already listening is not an error** (`ports::in_use`, a connect test over 127.0.0.1 and ::1):
  - `apply` leaves the port out of the web server's config, and `sites` reports `port_busy` with the holder from `lsof` ("php (pid 38571)").
  - While a Devid web server runs, the ports in its last applied config are its own and are not tested again.
  - Sending `devid app port` again picks the port up once it is free.
- **Web servers:** a plain-HTTP block on `127.0.0.1:<port>` with the site's usual body, answering any Host (a tunnel sends its public name):
  - **nginx:** `listen … default_server`.
  - **Caddy:** `http://:<port>`.
  - **Apache:** `Listen` and a `<VirtualHost>`.
- **Tunnel headers:** a tunnel's own `X-Forwarded-*` headers are kept:
  - **nginx:** a `$devid_forwarded_proto` map;
  - **Caddy:** `trusted_proxies` for loopback;
  - **Apache:** `setifempty` on the local port.
- **Apache fix on the way:** `X-Forwarded-For` used `%{REMOTE_ADDR}s`, which is an SSL variable, so it was empty. It is now `expr=%{REMOTE_ADDR}`. `X-Forwarded-Host` now carries the Host the visitor sent.
- **CLI:**
  - `devid app port [site] [PORT|off]`; one argument that looks like a port is a port.
  - `devid app show` prints the port, or who holds it.
  - `devid sites` adds ", :8000" or ", :8000 busy".
- **Version:** 0.9.0.

**Verification:**
- **Checks:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass (74 tests).
- **New unit tests:**
  - `ports.rs`: a listener on `*` counts as in use, and its holder is named.
  - Local-port rendering on all three servers, and Apache's forwarded headers.
- **New integration test** (`local_port`): the first free port, a busy port on `*` left out while other sites stay, taken once free, validation, and off.
- **A second daemon on the owner's Mac** (own `DEVID_HOME`), on nginx, Apache and Caddy, with a request shaped like cloudflared's (public Host, `X-Forwarded-Proto: https`, `X-Forwarded-For`):
  - PHP sees the public Host and https.
  - A proxy site gets `Host: localhost:<up>`, the public name in `X-Forwarded-Host`, and https.
  - A port held on `*:18612` is reported busy, and the other sites keep serving. Once free, the port is served (Caddy about 1 s after its restart).
  - The owner's daemon, sites and dev servers were not touched.

**Owner's migration** (after the release, A3 part of "Owner's migration" above): stop `php artisan serve`, then `devid app port erdbpro-saas 8000`. If `artisan serve` is still running, Devid says so and keeps serving everything else.

### A4 as built (2026-11-08)

- **Config:** `[[apps.<site>.commands]]` with `name` and `run`, in the order they were added. A name is letters, digits, `-`, `_`, `.` and `:` (as npm scripts are named), up to 40 characters. Saving a name again replaces its line.
- **`daemon::commands`:**
  - **Start:** `$SHELL -c '<run>'` in the site's folder, in its own session (`setsid`), stdin `/dev/null`, the signal mask cleared (the daemon blocks its stop signals), stdout and stderr appended to the log.
  - **Environment:** `$SHELL -l -c 'env -0'` once per daemon, at the first start, ten seconds at most. If it fails, the daemon's own environment is used and the log says so. `PATH` is Devid's `bin` (its `php` and `composer` shims), then nvm's Node for the folder, then the login shell's `PATH`.
  - **nvm:** `.nvmrc`, else `alias/default`, following aliases (`lts/*` → `lts/jod` → `v22.…`), then the highest installed `versions/node/v…` that matches; `node` means the newest. No match leaves `PATH` alone.
  - **Stop:** SIGINT to the group, SIGTERM after 5 s, SIGKILL after 10 s; it returns when no process of the group is left. Stopping several commands signals them together.
  - **State:** `running` (pid, since), `exited` (code or signal, when) or `stopped`. A watcher thread per running command checks once a second; nothing restarts.
  - **Log:** `~/.devid/log/app-<site>-<name>.log`, emptied at each start, headed by `$ <run>`, cut to its last 1 MB from a line start once it passes 10 MB (checked every 30 s). `devid logs` and the Logs page show it; log names may now contain `:`.
  - **Leftovers:** `~/.devid/run/app-<site>-<name>.pid` holds the pid and its `ps` start time. A starting daemon stops every group whose leader still has that start time (in the background), and removes the files. A daemon stop stops every command.
- **Requests:** `CommandAdd`, `CommandRemove` (stops it first), `CommandStart` and `CommandStop` (one command, or all of the site's when `name` is None). `CommandStop` also takes a site that was ignored or unlinked while its commands ran.
- **`sites` reply:** `commands` (name, run, log and the state above) and `suggestions`: `package.json` scripts named `dev*`, `start*` or `watch*` with the lockfile's package manager (pnpm, yarn, bun, else npm), and for Laravel `queue:work`, `schedule:work` and `reverb:start` when `laravel/reverb` is required. A suggestion whose name or line is already saved is left out. `status` adds `commands_running`.
- **`app reset`** now keeps the local port as well as the commands: it resets the type only. In A3 it also dropped the port.
- **CLI:** `devid cmd [list] [site]`, `devid cmd add [site] <name> -- <command…>`, `devid cmd rm [site] <name>`, `devid cmd start|stop [site] [name]`. Inside a project, one argument to start or stop is a command of that site when it has one by that name, else a site. `devid status` shows the number of running commands.
- **Version:** 0.10.0.

**Verification:**
- **Checks:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass (78 tests).
- **New unit tests:** nvm resolution (`.nvmrc`, default alias, alias chains, `node`, numeric order, no match, a path in `.nvmrc`); name validation; log cutting; suggestions; `[[apps.<site>.commands]]` in the config.
- **New integration test** (`commands`), with `SHELL=/bin/sh`: suggestions; add and validation; start all; an exited command's code and log; the command runs in the site's folder with Devid's `bin` first on `PATH`; stop one; stop all, where a command that ignores SIGINT with a child needs SIGTERM (5 s) and nothing is left; `kill -9` of the daemon leaves a command running and the next daemon stops it; `app reset` keeps the commands; `cmd rm` stops a running one; `daemon stop` stops them all.
- **Acceptance on the owner's Mac:** a second daemon (own `DEVID_HOME`) with launchd's environment (`PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `HOME`, `SHELL`), erd-builder-pro linked there, `PORT=3010 npm run dev:pg:local` and `npm run dev:tauri` saved, then `devid cmd start` in the project:
  - npm and Node v24.20.0 came from Devid's nvm lookup; cargo from the script's own `$HOME/.cargo/bin`.
  - Vite (3098) answered after 29 s, the API (3100) and `dev:pg:local` (3010) after 74 s. The cold Tauri debug build took 11 min 28 s while the Mac was busy (load average up to 50), and the "ERD Builder Pro" dev window opened (the owner saw it).
  - With both running: `dev:pg:local` was 6 processes and 477 MB RSS, `dev:tauri` 15 processes (npm, concurrently, tsx, node, esbuild, the app) and 930 MB. The daemon (debug build) was 5.3 MB RSS with 6 threads.
  - `devid cmd stop erd-builder-pro` took 1 s: no process of either group was left, ports 3010, 3100, 3098 and 24678 were closed, the window was gone and the pid files were removed.
  - The owner's own `dev:pg:local` on port 3000 and their other erd-builder-pro instance kept running; the repository was unchanged.

**Owner's migration** (A4 part): `devid unignore erd-builder-pro`, then in the project `devid cmd add dev:pg:local -- npm run dev:pg:local` and `devid cmd add tauri -- npm run dev:tauri`, and `devid cmd start` once the terminal copies are stopped (the saved `dev:pg:local` needs port 3000).

### A5 as built (2026-11-08)

**Part 1: desktop app and menu bar** (0.11.0, branch `feat/a5-app-ui`). The view follows the approved Phase 8 prototype (`Phase8.dc.html` on the Devid Redesign canvas), without Share, which comes in A6.

- **Sites list:** each row shows the type (`preset_label`), the local port (`:8000`, or `:8000 busy`), the number of running commands, and "Proxy :3000" or "Static" in place of the PHP picker for those modes. The host name and a chevron open the site's detail; the ⋯ menu gains "Details".
- **Site detail** (title: the host; subtitle: the folder and the type; a back button):
  - **App:** Type (the eleven presets, "(detected)" marked, "Use detected" when the owner set one, which sends `app_reset`), Serve as (PHP / Static / Proxy), the PHP version (php), Web root (php and static), Single-page app (static), Forward to (proxy).
  - **Local port:** a switch (on picks the first free port from 8000), the port with "Serving" or "In use", and when busy the holder and "Check again".
  - **Commands:** Start all, Stop all and Add command; each command with its state (running with its pid, exited with its code or signal, stopped), Start or Stop, its log under the list (followed every second), and Restart, Show log and Remove… in its ⋯ menu.
  - **Add a command:** name and command fields, the site's suggestions as buttons that fill both, the daemon's error inline.
- Fields save on Enter or when they lose focus, and a poll does not redraw the page while a field has focus.
- **Menu bar:** the number of running commands next to the icon, a "Commands · N running" section with "Stop <site> · <name>" per command and "Stop all commands". `status` adds `running_commands` (site and name) for it.
- **CLI:** `devid cmd` indents suggestions under their heading.

**Verification:**
- **Checks:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`) and `cargo test` pass. New: the tray view lists running commands.
- **The UI with a real daemon's data:** a second daemon (own `DEVID_HOME`) with a Vite and a Laravel sample, three commands (running, exited with code 2, stopped) and a local port; `app.js` and `app.css` unchanged in Chrome, with `status` and `sites` from that daemon as fixtures:
  - the list (chips, "Proxy :5173", the port);
  - the detail of both samples (every row per mode, the port switch and chip, the commands with their states, the log panel);
  - the Add dialog, where a suggestion fills both fields.
  - Fixed on the way: the title stayed "Sites" until the next redraw, the port chip had no background, and "Proxy :5173" wrapped.
- **Not run:** clicks inside Devid.app itself (cmux's computer use was not set up yet) and the menu-bar title with a running command. They use the daemon requests the CLI and its tests cover.

**Part 2: the TUI** (branch `feat/a5-tui`, still 0.11.0).

- **Sites table:** URL, Type, Serving (the PHP version, `proxy :3000` or `static`, then ` · :8000` or ` · :8000 busy` and ` · ● 2 running`) and Folder.
- **A site's menu** (enter on a site) adds:
  - `y` App type…: "Use detected (…)" (`app_reset`), then the eleven presets with ✓ on the current one.
  - `m` Serve as…: php, static or proxy.
  - `p` Local port…: a port, `off`, or empty for the first free one from 8000; anything else is refused in the footer.
  - `g` Commands… (start or stop the picked one), `a` Start all commands, `s` Stop all commands, and `x` Remove a command… (confirmed). These appear only when the site has commands, and `a`/`s` only when something would change.
  - `n` Add a command…: the site's suggestions, or "Other…", which takes a name, a space and the command (`dev:pg:local npm run dev:pg:local`).
- Each command's log is on the Logs page (`app-<site>-<name>.log`).

**Verification (part 2):** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. The new `app_type_port_and_commands` test walks every new menu entry to its daemon request (`app_reset`, `app_set` with a preset or a mode, `app_port` with none, 0 or a refused value, `command_stop`/`command_start` for one or all, `command_remove` after y, `command_add` from a suggestion and from "Other…"). The render test checks the new menu lines and the command input. A rendered Sites screen (100×20) shows `Laravel  PHP 8.5 · :8000 · ● 1 running`.

**Still for the owner (A5's "done when"):** after the 0.11.0 release, run and stop both erd-builder-pro commands from Devid.app with no terminal.

### A6 as built: Share (2026-11-08)

Branch `feat/a6-share`, 0.12.0.

- **Request:** `ShareStart { site, cwd }`.
  - It finds an existing `cloudflared` in `/usr/local/bin`, `/opt/homebrew/bin` or the daemon's `PATH`. `DEVID_CLOUDFLARED` overrides it for tests. Without one, it says where to get it.
  - It refuses while the web server is not running.
  - It runs `cloudflared tunnel --no-autoupdate --url http://127.0.0.1:<http_port> --http-host-header <site>.<tld>` through the command machinery (D-036): its own session, the login shell's environment, a log, stop by group, leftovers after a crash and a daemon stop.
  - It waits up to 30 s for the `https://….trycloudflare.com` line in the log and replies with the URL, or with none while cloudflared is still connecting. Sent again, it only reports the URL.
- **Fix (2026-11-09):** cloudflared now reaches the site on its own local port when it has one, else on `:80` when pf forwards it, and on `http_port` only without pf. On the owner's Mac a share of erdbpro-saas showed "host down": every request failed with `dial tcp 127.0.0.1:8088: connect: operation timed out`.
  - In measurements, new direct connections to pf's redirect targets 8088 and 8443 hung in `SYN_RCVD` (0/20 and 1/20; only a first one, after 100 ms, got through). `:80`, `:443` and the local port 8000, served by the same nginx, answered 20/20.
  - pf's state table was empty, and restarting nginx changed nothing. The exact cause stays open; the share no longer depends on that path.
- **`ShareStop { site, cwd }`** stops it. It also accepts a site that was ignored meanwhile.
- **Internals:**
  - A share is the site's command entry `.share`, a name no user command can have.
  - Its log is `share-<site>.log`; its pid file is `app-<site>-.share.pid`.
  - "Stop all" of a site's commands leaves the share running; a daemon stop ends it.
  - It is not counted in `commands_running` and not listed in `running_commands`.
- **Replies:** `sites` adds `share` per site: null, or the state (`starting` until the URL is in the log, `running`, `exited`), the URL and the log name. `status` adds `shares` (site and URL).
- **URL parsing:** `commands::share_url` takes the first `https://<name>.trycloudflare.com` in the log and skips the API endpoint that cloudflared logs when the tunnel request fails.
- **Clients:**
  - **CLI:** `devid share [site]` and `devid share [site] --stop`. `devid status` and `devid app show` print the link.
  - **App:** the prototype's Share card (Share, "Connecting…", Stop sharing, the link with Copy, and an exited state that points at the log). The list shows a "Shared" chip. `copy_url` gains `share: true`, which copies the link the daemon reports.
  - **Menu bar:** "Stop sharing <site>" per share.
  - **TUI:** `h` in a site's menu shares the site or stops sharing it (the item shows the link). The table adds " · shared".
- **Version:** 0.12.0.

**Verification:**
- **Checks:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`) and `cargo test` pass. New unit test `share_url_from_cloudflareds_log`; the TUI and tray tests cover the new entries.
- **New integration test** `share_runs_cloudflared_and_reads_its_url`, with a fake cloudflared that prints the quick-tunnel line, so nothing reaches the internet:
  - it is refused while the web server is not running;
  - the URL is read, and the arguments are checked;
  - `sites` and `status` report the share, and it is not counted as a command;
  - stopping the site's commands leaves the share;
  - `--stop` ends the process.
- **The Share card** with a fixture, unchanged `app.js`/`app.css` in Chrome.
- **Not run yet:** a real quick tunnel and cloudflared's RSS while sharing. A real share opens a public URL, so it waits for the owner's go-ahead (see "A1 results").

## Not in this phase

- Commands for folders that are not sites (a mode with no web serving).
- Starting commands at login, and restarting them on a crash.
- Per-command environment variables in Devid. Projects keep their `.env`.
- A project file such as `devid.toml` that a repository carries.
- Devid-managed Node versions. Devid uses the user's Node.
- Managing a named Cloudflare tunnel or its dashboard routes.
- Finding a running command's ports automatically.
- macOS notifications (D-028 leaves them for later).
