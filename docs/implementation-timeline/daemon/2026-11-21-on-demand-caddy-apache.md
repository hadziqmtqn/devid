# Dev servers on demand on Caddy and Apache (Phase 10 O4)

> Date: 2026-11-21 · Branch: `feat/on-demand-caddy-apache` · Phase 10 O4 (`docs/roadmap/phase-10.md`, D-054)

## What it does

`devid cmd on-demand` now works the same on all three web servers. With Caddy or Apache active, a request to a sleeping on-demand site starts its command, gets its page, and the command stops after the site's idle time. On-demand is never "unavailable" any more, and `devid cmd on-demand` no longer warns about Caddy and Apache.

## How

- **Caddy (`caddy.rs`):** an on-demand site's `handle_errors 502` wakes the command only for a dial error (`vars_regexp {http.error.message} "^dial "`).
  - **Dial errors only:** a dev server that took the request and closed gives `EOF` or a write error instead, and the client keeps getting "Nothing answers".
  - **The wake request:** `reverse_proxy unix/<wake.sock>` with `method GET`, which makes Caddy send no body, and `X-Devid-Method: {method}`.
  - **The retry:** on the daemon's 418, `handle_response` proxies the same request, body included, to the dev server; any other answer (the Starting page, a failure) passes through.
  - **The request file:** a `log` with `format filter` (`request` and `resp_headers` deleted) writes a 170-byte line per request to `run/requests-<site>`.
- **Apache (`apache.rs`):** mod_proxy answers 503 for a refused connection and 502 for a dev server that failed after taking the request.
  - **The wake request:** an on-demand site's `ErrorDocument 503` is `/.devid-wake`, `ProxyPass`ed (before `/`) to the wake socket with `timeout=75`. `X-Devid-Request: %{THE_REQUEST}` carries the original request line.
  - **No direct calls:** `Require env REDIRECT_STATUS` refuses a client that asks for the path itself.
  - **The request file:** `CustomLog` writes `%{msec}t` to `run/requests-<site>`.
- **Both:** an on-demand site keeps no idle connection to its dev server. Caddy uses `transport http { keepalive off }` on both proxies, and Apache `disablereuse=On`.
- **Daemon (`ondemand.rs`):**
  - **Method:** it reads the original method from `X-Devid-Method` (Caddy) or `X-Devid-Request` (Apache), so a form POST is never taken for a page load.
  - **Body:** it reads and drops a body the web server sends with the wake request (`Content-Length` or chunked), since Apache's mod_proxy_http still passes it.
  - **Ready answer:** 418 for nginx and Caddy. For Apache it is 307, with `Location` set to the original target if that is a path with one leading `/` and only visible ASCII, else `/`.
- **Removed:** `ondemand::available`, the `unavailable` state and the `app_set` warning.

## Where it differs from the plan

- **Apache's URL:** it comes from `%{THE_REQUEST}`, not `REDIRECT_URL`. The raw request line keeps the method and the encoded URL (`/a/b%20c?next=%2F`) exactly as sent, while `REDIRECT_URL` is decoded and has no query.
- **Apache passes the body:** the plan expected the ErrorDocument redirect to carry no body. mod_proxy_http still sent it to the socket (50 KB in the spike) and failed with a broken pipe when the daemon closed without reading it, so the daemon now drops it. Removing `Content-Length` with `RequestHeader unset` did not help: mod_proxy_http reads the body from the connection and sets the length itself.
- **Idle connections to the dev server:** not in the plan, found by a check (below).

## Found on the way: long connections through Caddy and Apache

The O4 checks found three problems with long or idle connections. The two on Apache predate O4; all three hit on-demand sites hardest, because an open tab keeps a WebSocket to the dev server and the idle check counts connections.

- **60 s limit:** Apache's `Timeout` (60 s) cut a response that took longer and an idle WebSocket. nginx gives proxy sites `proxy_read_timeout 1h`, and Caddy has no limit. A 130 s request through Apache got "Nothing answers" at 60 s. The proxy line now has `timeout=3600`, and the same request gets its 200.
- **Stall after a config change:** Devid reloads Apache gracefully (SIGUSR1) with `ServerLimit 1`. The new process could not start until the old one had finished its open connections. With a WebSocket open, the next request to any site waited 89 s after a config change.
  - **Fix:** `ServerLimit 16` leaves room for the old process, while `MaxRequestWorkers` still keeps one serving process. The same request now takes 0.99 s, and the WebSocket stays open through the reload.
  - **Not chosen:** a hard restart (SIGHUP), which was tried first. Apache needs about 10 s to kill a process with an open WebSocket (SIGTERM twice, then SIGKILL), and requests waited 9.4–10.4 s.
- **An idle connection pool:** against a dev server that keeps idle HTTP/1.1 connections open, Apache held 1 connection and Caddy 2, still open after 40 s. The idle check would count them as an open tab and never stop the command. nginx held none. Hence `disablereuse=On` and `keepalive off` on on-demand sites; after the change, neither held any.

## Checks

On the owner's M1 (macOS 26.6.1) with this branch's debug build.

- **Test daemon:** its own `DEVID_HOME` and `HOME`, on spare ports (http 28188, https 28543, dev servers 28400–28410).
- **Servers:** Caddy 2.11.9 from the runtime release (downloaded into the test folder, with the owner's OK), and the system Apache 2.4.67.
- **The owner's setup:** their daemon and its five commands kept running throughout.
- **The spike:** Caddy and httpd ran by hand against the test daemon's wake socket and a fake one before the code was written.

`php -S` as the dev server:

| Case | Caddy | Apache |
|---|---|---|
| Page load to a sleeping site | 200 in 0.12–0.15 s | 307, then 200; 0.07 s in all |
| A request while it runs | 200 in 0.005 s | 200 in 0.005 s |
| 200 KB POST while sleeping | delivered once, same SHA-1 | 307, sent again by curl `-L`: same SHA-1 |
| A form POST that does not follow | 200 (retried inside Caddy) | 307 to `/echo.php?f=1`, query kept |
| 8 requests at once while sleeping | all 200; one start, one `php -S` | the same |
| HEAD while sleeping | 200 | 307, then 200 |
| Page load, command listens after 7 s | 503 Starting page at 5.04 s | 503 Starting page at 5.04–5.07 s |
| API call to the same command | waited, 200 at 2.0 s | 307 at readiness, 200 at 2.0 s |
| Command that exits with 3 | 500 in 1.04 s, code and escaped log | the same |
| HEAD to that command | 500 in 1.05 s | 500 in 1.05 s |
| Dev server takes a POST and drops it | 502 "Nothing answers", not woken | 500 from Apache over HTTP/2, not woken |
| No requests, `idle_stop = 1` | stopped 81 s after the last request | stopped 65 s after |
| A request held open 130 s | kept running; stopped 81 s after it ended | the request now completes (200 at 130 s) |
| The next request | woke it, 200 in 0.14 s | woke it, 200 in 0.10 s |

An idle WebSocket (Node), held 100 s without traffic to an on-demand dev server with `idle_stop = 1`:

| | nginx | Caddy | Apache |
|---|---|---|---|
| First WebSocket to the sleeping site | open in 0.10 s | open in 0.10 s | fails (a WebSocket does not follow a 307), but wakes the command; the next one opens |
| Held 100 s | stayed open; command kept running | the same | the same |

The O3 check with a real Next.js dev server: a new APFS clone of devid-site (Next.js 16.4.6) as `od4-site`, `next dev -p $PORT` on demand with `idle_stop = 1`, and a tab in the app's browser at `http://od4-site.test:28188/`:

| Case | Caddy | Apache |
|---|---|---|
| First page load from sleep | 200 in 1.5 s | 307, then 200; 1.5 s in all |
| No tab open | stopped 81–91 s after the last request | stopped 70–81 s after |
| The page opened in a tab while sleeping | it woke and loaded | it woke and loaded (the browser followed the 307) |
| Tab open for 150 s | kept running; the HMR WebSocket was one open connection | the same |
| Tab closed | stopped 60–70 s later | stopped 85–95 s later |

The idle check runs every 30 s and counts from the last time it saw a request or connection, so a stop comes 60–90 s after the last use with `idle_stop = 1`.

Other checks:

- **Unit tests:**
  - wake requests from each server (the method from `X-Devid-Method` and `X-Devid-Request`, the redirect target and its sanitising, body framing);
  - a 50 KB and a chunked body read to their end and no further;
  - the 418 and 307 answers;
  - `render_on_demand` for Caddy and Apache;
  - Apache's `ServerLimit` and `timeout=3600`.
- **`scripts/smoke.sh`** now also wakes the on-demand site through Caddy (installed from the release) and Apache (the system httpd), and checks that the command runs. The whole smoke passed on arm64 in 30 s.
- **A flaky smoke check:** the O3 on-demand checks piped `devid cmd` into `grep -q` under `set -o pipefail`. `grep -q` exits at the first match, devid then panics on the broken pipe, and the check fails at random; it did once here. They now use `grep … > /dev/null`, which reads all the output.
- **Leftover `detached-flush.js`:** the clone's `next.config.mjs` skips `createMDX()` when Next's telemetry flush loads it. After all stops of the clone (on Apache and Caddy), none was left; in O3, every stop of a clone without that guard left one.
