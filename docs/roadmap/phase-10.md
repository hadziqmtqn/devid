# Phase 10 Plan: dev servers on demand

> Scope: a proxy site's dev server starts with the first request and stops after it has been idle, so a dev server costs RAM only while it is used (D-054)
> Verified against: 2026-11-21 (v0.16.1 installed on the owner's M1; nginx 1.30.7)
> Status: accepted 2026-11-21 (D-054). O1 (spike) done; O2 shipped in 0.16.1; O3 built on branch `feat/on-demand-nginx`, see "O3 results".

Today a proxy site's dev server runs from the moment the user presses Start until they press Stop (D-036). Nothing starts at login, so after a login or a crash every proxy site shows "Nothing answers on localhost:<port>" until its command is started by hand. Once started, a dev server keeps its RAM all day, even when nobody has opened the site for hours.

On the owner's M1 with 8 GB (D-052), on 2026-11-21, three idle proxy sites held about 1.1 GB, and 2.6 GB of the 4 GB swap was in use:

| Site | Command | Processes | Footprint, idle |
|---|---|---|---|
| devid-site | `dev` (`next dev -p 4321`) | 20 | 777 MB |
| erd-builder-pro | `start` (Express, `tsx`) | 4 | 193 MB |
| erd-selfhost-local | `start` (the same server on 3001) | 2 | 156 MB |

Footprint is macOS's `footprint` total for the command's process group. It counts compressed and swapped pages, which RSS leaves out.

Phase 10 lets a proxy site name the command that serves it. The web server wakes that command with the first request, the request waits until the dev server answers, and the daemon stops the command once the site has been idle for a while. Herd 2 has the same feature, behind Herd Studio (see `docs/research/competitors-2026-11.md`, opportunity 5).

## Ground rules

- **Client rule (D-012):** the setting, the wake-up and the idle stop live in the daemon. The CLI, the TUI and the app render the state and send requests, and every action is a CLI subcommand.
- **Opt-in:** on-demand is off for every site until the user turns it on for one command of a proxy site. A site with it off behaves exactly as in 0.16.
- **Only the user's own saved command:** a request can start only the command the user picked for that site. A cloned repository still cannot make Devid run its code (D-036).
- **Resources (hard constraint):**
  - While no on-demand command runs, the daemon adds no timer and no work. The wake socket is one blocked `accept`.
  - While one runs, the idle check is one pass every 30 s: a `stat` of the site's access log and a look at the command's sockets, under 1 ms each (see "O1 results").
  - A request to a running dev server goes straight to it, as today. Only a request that finds it down reaches the daemon.
- **Local only:** the wake socket is a unix socket in `~/.devid/run`, mode 0600. Only the web server's config names a site on it.
- **Owner's machine:** tests use their own `DEVID_HOME`, spare ports and copies of projects. The owner's running commands are never stopped by a test.

## Design

### The setting

```toml
[apps.devid-site]
on_demand = "dev"   # the command that serves this site; the first request starts it
idle_stop = 10      # minutes without requests or open connections before it stops; 0 = never
```

- Only a proxy site can have it, and the command must be one of the site's saved commands. The command keeps `PORT=<Forward to>` (D-041), and the wake-up waits for that port.
- `idle_stop` defaults to 10 minutes (Herd: 5). A warm Next.js restart takes about 2 s (O1), so a short wait costs little.
- Removing the command, or changing the site's mode away from proxy, turns on-demand off.

### Waking: the web server asks the daemon (O1, nginx)

It builds on O2's retry (`upstream devid_again_<site>`, `@devid_again`). For an on-demand site, `@devid_again` hands a failure to `@devid_wake` instead of `@devid_down`:

```nginx
upstream devid_again_devid_site {                         # O2
    server 127.0.0.1:4321 max_fails=0;
    server [::1]:4321 max_fails=0;
}
server {
    ...
    recursive_error_pages on;
    location / { proxy_pass http://localhost:4321; ... }  # as in 0.16: quiet error log
    error_page 502 504 = @devid_again;
    location @devid_again {                               # O2
        if ($upstream_bytes_sent ~ "[1-9]") { return 502; }   # it took the request: never send it twice
        proxy_pass http://devid_again_devid_site; ...
        error_page 502 504 = @devid_wake;                  # @devid_down while on-demand is off
    }
    location @devid_wake {
        if ($upstream_bytes_sent ~ "[1-9]") { return 502; }   # -> @devid_down
        proxy_pass http://unix:<home>/run/wake.sock;
        proxy_pass_request_body off;
        proxy_set_header Content-Length "";
        proxy_set_header X-Devid-Site devid-site;
        proxy_read_timeout 75s;
        proxy_intercept_errors on;
        error_page 418 = @devid_retry;     # the daemon says: it answers now
        error_page 502 = @devid_down;      # the daemon is not running
    }
    location @devid_retry {                               # no guard: the wake call itself sent bytes
        proxy_pass http://devid_again_devid_site; ...      # the same request, method and body included
        error_page 502 504 = @devid_down;
    }
    location @devid_down { ... today's message ... }
}
```

- **Request flow:**
  - The dev server refuses the connection on both paths, and nginx goes to `@devid_wake`.
  - The daemon starts the command once, however many requests arrive together.
  - The daemon waits until the port accepts, then answers `418`, which nginx turns into the same request to the dev server.
  - The client sees one slower response.
- **What the daemon answers:**

  | Case | Answer |
  |---|---|
  | The port accepts | `418`; nginx retries the request |
  | A browser page load (GET with `text/html` in `Accept`) still starting after 5 s | `503` "Starting devid-site…" page: the command, the time so far and the last log lines, refreshing every second, `Retry-After: 1`, `no-store` |
  | Any other request still starting after 60 s | `504`, with the same text |
  | The command exited | `500`: its exit code and the last 20 log lines, HTML-escaped |

  - Every answer except `418` passes through to the client, because `@devid_wake` has an `error_page` only for `418` and `502`.
  - `502` there means nginx could not reach the wake socket, so the daemon is down. The daemon itself never answers `502`, because nginx could not tell the two apart.
- **The retry group (O2):**
  - `localhost:<port>` is two addresses, and nginx skips one that refused for 10 s ("no live upstreams"). Retrying through it would fail at once. This also hits 0.16 with on-demand off: after a dev server came up, its `.test` address kept answering "Nothing answers" for 10.6 s in O1.
  - The retry group has `max_fails=0`, so it always tries both addresses.
  - It is not the main path. With `max_fails=0` there, nginx would try the unused address on every other request (Vite listens on ::1 only, most servers on 127.0.0.1) and log each refusal: 20 lines per 40 requests, against 1.
- **No repeated requests:** the guards send on only a request that never reached the dev server (`$upstream_bytes_sent` is 0), so a POST that a restarting server took and dropped is not sent twice. `$upstream_connect_time` cannot tell them apart: a refused loopback connect also records `0.000`.
- **Local port and Share:** the site's local-port block (D-037) has the same body, so a tunnel's request wakes the site too. A Share visitor can see the "Starting…" page.
- **Not chosen:**
  - `X-Accel-Redirect` back to the dev server. nginx follows it as a GET, so a POST would lose its body.
  - The daemon proxying the first request itself. That needs a full HTTP proxy in the daemon, with streaming and WebSockets.
  - Holding every request until the server answers. A cold dev server can take a minute (erd-builder-pro took 29–74 s on the Intel Mac in Phase 8 A4), and a blank loading tab says nothing about progress or a broken build.

### Waking on Caddy and Apache (O4, unverified)

Neither server is installed on the owner's Mac. O4 starts with a spike of each one.

- **Caddy:**
  - Plan: `handle_errors 502` sends the request to the wake socket (`reverse_proxy unix/<sock>`), and `handle_response` on `418` proxies it to the dev server again.
  - Unknown: whether the request body survives the first proxy attempt.
  - Fallback: the daemon answers `307` to the same URL, which browsers and `fetch` follow with the same method and body.
  - Caddy's passive health checks are off unless configured, so it has no 10-second stale window (to confirm in O4).
- **Apache:**
  - mod_proxy answers `503` when the dev server refuses. Its proxy line already uses `retry=0`, so Apache has no stale window either.
  - Plan: `ErrorDocument 503` points at a local path that is `ProxyPass`ed to the wake socket, with the original URL in a header taken from `REDIRECT_URL`. The daemon then answers `307` to it.
- **If a server cannot do it cleanly,** on-demand shows as unavailable on that server, and the site falls back to today's message with "Start" in the app.

### The daemon

- **Wake socket:** `~/.devid/run/wake.sock`, bound only while some site has `on_demand`. One thread blocks in `accept`, with a thread per wake connection like the main socket. Each connection is one HTTP/1.1 request: at most 16 KB of headers, `X-Devid-Site` naming the site, no body, and one response, then the connection closes.
- **Starting:**
  - The same path as `devid cmd start`: `$SHELL -c '<run>'` in the site's folder, its own process group, `PORT`, nvm's Node, and a log headed "started by a request to <host>".
  - A per-site mutex makes concurrent wake-ups start it once.
  - Readiness is `ports::in_use(upstream)`, checked every 50 ms.
- **Idle stop:**
  - A watcher thread runs only while at least one on-demand command runs, every 30 s.
  - A site counts as active while either of these holds:
    - **Requests:** nginx writes an access log only for on-demand sites, `access_log <home>/log/access-<site>.log devid_last;` with `log_format devid_last '$msec';`. The daemon reads its mtime and empties the file when it passes 64 KB.
    - **Open connections:** any ESTABLISHED TCP socket of the command's process group on the upstream port (libproc: `proc_listpgrppids`, `PROC_PIDLISTFDS`, `PROC_PIDFDSOCKETINFO`). An open browser tab keeps its HMR WebSocket open, so a tab with the site counts as use even when nothing is requested.
  - A site that has been neither for `idle_stop` minutes is stopped the way `devid cmd stop` does: SIGINT to the group, SIGTERM after 5 s, SIGKILL after 10 s. Its state becomes `sleeping` until the next request.
- **Manual Start and Stop:**
  - Start works as today.
  - Stop puts an on-demand command to sleep, so the next request starts it again. To keep it off, turn on-demand off.
  - Start all and Stop all include it.
- **Lifetime:**
  - Nothing starts at login, as before. The first request after login starts the command, which removes most of the need for "start at login".
  - The update resume (D-040) skips on-demand commands, since the next request starts them.
  - A daemon stop stops them as it stops every command.
- **Status:**
  - `sites` gives each on-demand site `on_demand {command, idle_stop, state, idle_for}`. `state` is `sleeping`, `starting`, `running` or `failed`.
  - `status` counts sleeping sites next to `commands_running`.

### Daemon API

- `app_set` gains `on_demand: Option<String>` (a command name, or `""` for off) and `idle_stop: Option<u32>`. Both go through `State::mutate`, so a config nginx refuses rolls back.
- No new request for the wake-up itself: it arrives on the wake socket, from the web server.

### Clients (O5)

- **CLI:**
  - `devid cmd on-demand [site] <command> [--idle <minutes>]` and `devid cmd on-demand [site] off`.
  - `devid cmd` marks the command "on demand · sleeping" or "on demand · stops after 10 min idle".
  - `devid sites` adds "on demand".
- **App** (a board on the design canvas first):
  - In the site detail's Commands section, a "Start on first visit" switch on one command, with "Stop after N min idle".
  - The site row shows "Sleeping" or "On demand".
  - The menu bar counts sleeping sites apart from running commands.
- **TUI:** the site menu gains `o` On demand… (pick the command or off, then the minutes).

## Milestones

| # | Milestone | Done when |
|---|---|---|
| O1 | **Spike** | the numbers and answers are in this doc (done 2026-11-21, see "O1 results") |
| O2 | **Fix: no stale 502 after a dev server starts.** A failed request tries once more through a `max_fails=0` group (`@devid_again`) before "Nothing answers". A 0.16.x patch, independent of the rest | a dev server that comes up answers through `.test` on the next request, not 10 s later, checked by `scripts/smoke.sh` on both arches (shipped in 0.16.1, 2026-11-21) |
| O3 | **Daemon and CLI on nginx:** the setting, the wake socket, starting and waiting, the Starting and failure pages, idle stop, status, `devid cmd on-demand` | on the owner's Mac, devid-site with `on_demand = "dev"` opens from sleep at `https://devid-site.test` with one slower load, stops after its idle time with no tab open, and keeps running while a tab is open (built 2026-11-21, branch `feat/on-demand-nginx`: checked with a test daemon and a copy, see "O3 results"; the check on the owner's own devid-site waits for an install) |
| O4 | **Caddy and Apache:** spike, then the wake path on each, or "unavailable on this server" | the O3 check passes on both servers with a test daemon, or the docs and the app say why not |
| O5 | **App and TUI** | the owner turns on-demand on and off for devid-site in Devid.app and sees it sleep and wake |
| O6 | **Docs and release 0.17.0:** devid-site guide, README, roadmap, timeline record | released and installed on the owner's Mac (the release waits for the owner's go-ahead) |

Each milestone is one worktree and one PR. O2 can ship before the plan is accepted, since it fixes 0.16 behaviour on its own.

## O1 results (2026-11-21)

On the owner's M1 (macOS 26.6.1, 8 GB). Every test ran on spare ports (4399, 18080–18082) with copies, next to the owner's running sites, which kept answering throughout.

### Start time and RAM of a dev server

A copy of devid-site (Next.js 16.4.6, Turbopack, `NEXT_TELEMETRY_DISABLED=1`), started with `next dev -p 4399`:

| Start | Port accepts | First `/` (200) | Then `/docs` | Footprint 20 s later |
|---|---|---|---|---|
| Cold (no `.next`) | 1.1 s | 6.3 s | +2.2 s | 1209 MB, 20 processes |
| Warm (`.next` from the cold run) | 0.9 s | 2.1 s | +0.9 s | 1077 MB, 19 processes |

- The owner's own devid-site server, running for 12 hours with no tab open, was at 777 MB: macOS had compressed part of it.
- A wake-up therefore costs about 2 s on a warm project. It frees 0.8–1.2 GB for a Next.js site and 150–200 MB for the two Express servers.
- SIGINT to the group stopped all 20 processes within 5 s, and nothing was left.

### The wake path on nginx

nginx 1.30.7 (Devid's build) in front of a Python stand-in for the dev server (it starts listening after 1.5 s) and a stand-in for the daemon on a unix socket, configured as in "Waking" above:

- **One request while the server is down:** 200 in 1.74 s, with the server's own answer, and no change on the client's side.
- **While it runs:** 200 in 2 ms, straight to the server, without touching the wake socket.
- **Eight requests at once while it is down** (5 GETs, a 200 KB POST, a PUT, a HEAD):
  - All answered 200 in 1.75–1.82 s.
  - The server was started once.
  - The POST body arrived intact (same SHA-1), and the wake socket received no body.
- **A server slower than the wait (4 s, with a 1 s hold):**
  - The daemon's `503` page passed through to the client unchanged.
  - The first reload got the page again.
  - The second reload got 200 from the server.
- **The stale-502 problem:**
  - With `proxy_pass http://localhost:<port>` as in 0.16, a refused address is skipped for 10 s ("no live upstreams").
  - The wake-up then looped through `error_page` until nginx gave up with 500 ("rewrite or internal redirection cycle").
  - With Devid 0.16's exact proxy shape, a server that came up kept getting "Nothing answers" for 10.6 s.
  - An `upstream` block with `max_fails=0` on both addresses fixed both. `@devid_retry` needs its own `error_page`, or a failed retry loops back to `@devid_wake`.
  - Building O2 showed the cost of `max_fails=0` on the main path: a refused attempt, and an error-log line, on every other request to a server on one address. O2 therefore keeps the main path and retries through the `max_fails=0` group only after a failure, guarded by `$upstream_bytes_sent`. The details and numbers are in `docs/implementation-timeline/daemon/2026-11-21-nginx-stale-502.md`.

### Idle detection

- **An open tab is visible:** an HMR WebSocket to devid-site's Next.js server (`/_next/webpack-hmr`, through `devid-site.test`) showed as one ESTABLISHED connection on port 4321 while it was open, and none 0.5 s after it closed.
- **Cost:**
  - `netstat -anp tcp` over the Mac's 131 TCP sockets takes under 10 ms; `lsof` for one port about 10 ms.
  - The daemon will read its own command's sockets through libproc instead, without starting a process. That cost is measured in O3.

### Not covered yet

- A WebSocket upgrade that arrives while the server is down (the stand-in has no WebSocket). An HMR client reconnects by itself once the page has loaded, so a failed upgrade only delays it.
- Caddy and Apache (O4).
- How long erd-builder-pro takes to start on the M1. The Intel Mac's 29–74 s was for a different script with a Vite build.

## O3 results (2026-11-21)

Built on branch `feat/on-demand-nginx`; the record with every check is `docs/implementation-timeline/daemon/2026-11-21-on-demand-nginx.md`.

- **The O3 check, on a clone of devid-site:** a test daemon on spare ports ran a clone of devid-site with `next dev` on demand and `idle_stop = 1`.
  - It opened from sleep with one slower load: 200 in 4.8 s.
  - With no tab open, it stopped by itself 70–80 s after the last request.
  - With a tab open, it kept running for 152 s, and stopped 59–69 s after the tab closed.
- **Cost of the connection check:** 0.1–0.3 ms per idle check against devid-site's 19 processes.
- **Differences from the design above:**
  - The request file is `run/requests-<site>`, not a log.
  - The wake socket stays bound until the daemon stops.
  - `idle_stop` is 0–1440 minutes.
  - A command that exits shows its page after up to 1 s, the command watcher's poll.
- **Still open:**
  - **The check on the owner's own devid-site** needs this build installed.
  - **A stray `detached-flush.js` process:** every stop of devid-site's `next dev` leaves one behind (Next 16.4 with fumadocs-mdx, even with `NEXT_TELEMETRY_DISABLED=1`). With on demand, each sleep adds one, so devid-site needs a fix before it goes on demand.

## Not in this phase

- The Vite server of a PHP site (Laravel's `npm run dev`). Its requests go to `public/hot`'s port from the browser, not through the site's web server, so nothing can wake it.
- Queue workers, schedulers and other commands that do not serve the site.
- Several on-demand commands per site, or one command waking another.
- Waking a dev server that is reached directly on `localhost:<port>` instead of through `.test` or the local port.
- Starting commands at login (D-036 stays).
