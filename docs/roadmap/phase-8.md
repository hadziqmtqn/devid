# Phase 8 Plan: apps, commands and sharing

> Scope: app types detected per site (PHP, static, Node), a local port per site, user commands the daemon runs, and sharing a site through a quick tunnel (D-035, D-036, D-037)
> Verified against: 2026-11-08 (A2 built on branch `feat/a2-app-types`)
> Status: accepted 2026-11-08; A1 (spike) done, see "A1 results"; A2 built, see "A2 as built"; A3 next

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
- **Busy port:** before each render, the daemon tries to bind the port. A busy port is left out of the config, and `status` names the process holding it (for example `php` 38571 = `artisan serve`). A busy port must never stop nginx, and so take down every site.
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

## Not in this phase

- Commands for folders that are not sites (a mode with no web serving).
- Starting commands at login, and restarting them on a crash.
- Per-command environment variables in Devid. Projects keep their `.env`.
- A project file such as `devid.toml` that a repository carries.
- Devid-managed Node versions. Devid uses the user's Node.
- Managing a named Cloudflare tunnel or its dashboard routes.
- Finding a running command's ports automatically.
- macOS notifications (D-028 leaves them for later).
