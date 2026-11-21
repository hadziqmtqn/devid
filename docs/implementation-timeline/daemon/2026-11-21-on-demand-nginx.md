# Dev servers on demand on nginx: the daemon and the CLI (Phase 10 O3)

> Date: 2026-11-21 · Branch: `feat/on-demand-nginx` · Phase 10 O3 (`docs/roadmap/phase-10.md`, D-054)

## What it does

A proxy site can mark one of its saved commands as on demand:

```bash
devid cmd on-demand devid-site dev            # --idle <minutes>: default 10, 0 never
devid cmd on-demand devid-site off
```

- **Waking:** the first request while the dev server is down starts the command; the request waits until the dev server accepts and then goes to it.
- **Starting page:** a browser page load still starting after 5 s gets a page that reloads itself.
- **Idle stop:** the command stops after the site has had no requests and no open connections (an open tab's HMR WebSocket counts) for the set minutes.
- **Scope:** nginx only so far; Caddy and Apache are O4. Nothing changes for a site without it.

## How

- **Config:** `[apps.<site>] on_demand` and `idle_stop` (`config.rs`). `Site.on_demand` is set only for a proxy site whose command is saved. `mutate` drops a stale setting through `sites::drop_stale_on_demand`.
- **nginx:**
  - An on-demand site's `@devid_again` (O2) goes on to `@devid_wake`, which proxies to `run/wake.sock` without the body.
  - The daemon's `418` sends the same request to the dev server through `@devid_retry`.
  - Requests leave `$msec` lines in `run/requests-<site>`.
- **Daemon (`ondemand.rs`, glue in `server.rs`):**
  - **Wake socket:** bound from the first on-demand site until the daemon stops, a thread per wake request.
  - **Wake-up:** the start runs under a lock, so concurrent requests start the command once. The port is then checked every 50 ms.
  - **Answers:** `418` once it accepts; `503` Starting page for a page load after 5 s; `504` for other requests after 60 s; `500` with the exit code and log when the command exits.
  - **Idle check:** a thread that runs only while an on-demand command runs, every 30 s. It reads the request file's mtime and looks for ESTABLISHED sockets of the command's process group on the upstream port through libproc.
  - **Stop:** it takes the same start lock and checks again, so a request that arrives just then is never told the server accepts right before it stops.
- **Requests:**
  - `app_set` takes `on_demand` and `idle_stop`.
  - `sites` reports `on_demand {command, idle_stop, state, idle_for}`, and `status` lists the `sleeping` sites.
  - `command_stop` lists stopped on-demand commands as `sleeping`.
  - The update resume (D-040) skips on-demand commands.
- **CLI:** `devid cmd on-demand`; `devid cmd`, `devid sites` and `devid status` show the state.

## Where it differs from the plan

- **Request file:** it lives in `run/requests-<site>` rather than `log/access-<site>.log`. It holds state, not a log, and `devid logs` should not list it.
- **Wake socket lifetime:** once bound, it stays until the daemon stops, rather than closing when the last site turns on-demand off. Nothing waits on it but one blocked `accept`.
- **Exit detection:** a command that exits is noticed within 1 s, because the command watcher polls every second. A failing command's page therefore takes about 1 s.
- **`idle_stop` range:** 0 to 1440 minutes.

## Checks

On the owner's M1, macOS 26.6.1, nginx 1.30.7. The test daemon ran from this branch's debug build, with its own `DEVID_HOME` and `HOME` and spare ports (28188, 28543, 28635, 28400–28410). The owner's daemon and its five commands kept running throughout.

| Case | Result |
|---|---|
| Page load to a sleeping site (`php -S`) | 200 in 0.055 s, the dev server's page |
| A request while it runs | 200 in 0.08 s, straight to the dev server |
| 200 KB POST to a sleeping site | delivered once, same SHA-1, method POST |
| 8 requests at once while sleeping | all 200 in 0.06–0.07 s; one start (one log note, one `php -S`) |
| HEAD while sleeping | 200 |
| Page load, command listens after 7 s | 503 after 5.0 s: "Starting slow.test…", refresh 1 s, `Retry-After: 1`, `no-store`, the log's "compiling" |
| API call to the same command | waited, then 200 when it listened |
| Command that exits with 3 | 500 after 1.05 s, "exited with code 3" and its log, HTML-escaped; plain text for an API call |
| On-demand for a missing command; idle 2000 | refused with the reason |
| Idle with no requests, `idle_stop = 1` | stopped 70 s after the last request, with a line in its log and the daemon's |
| Next request after that | woke it, 200 in 0.13 s |
| A request held open for 130 s, `idle_stop = 1` | kept running the whole time; stopped 70 s after it closed |

The O3 check with a real Next.js dev server: an APFS clone of devid-site (Next.js 16.4.6) as `od3-site` in the test daemon, `next dev -p $PORT` on demand with `idle_stop = 1`:

| Case | Result |
|---|---|
| First page load from sleep | 200 in 4.8 s (warm `.next`), under the 5 s wait, so no Starting page; 19 processes |
| No tab open | stopped by itself 70–80 s after the last request |
| The page opened in a browser tab | the tab's request woke it ("started by a request"); the HMR WebSocket showed as an open connection |
| Tab open for 152 s | kept running the whole time |
| Tab closed | stopped 59–69 s later |

The clone's `/docs` answered 500 from Next.js itself (a fumadocs module that is generated in the owner's checkout and missing in the clone), straight from the dev server too; not part of this check.

Other checks:

- **Unit tests:**
  - wake request parsing, the replies, escaping and the Starting and failure pages (`ondemand.rs`);
  - the log tail, and trimming the request file without losing its mtime;
  - `established` against a real loopback connection;
  - `render_on_demand` (`nginx.rs`), and the on-demand rules in `sites.rs` and `config.rs`.
- **`scripts/smoke.sh`,** which every release runs on both arches, gains an on-demand check: a sleeping site answers its first request; a page load to a slow command gets the Starting page; a failing command shows its exit code. All smoke checks passed on arm64 in 26 s.
- **Cost of the connection check:** 0.1–0.3 ms per check against the owner's running devid-site `next dev` (19 processes), read only. It saw the HMR WebSocket while devid-site.test was open in a browser tab, and nothing 2 s after the tab closed.
- **First request after turning it on:** in the first run, requests sent a few milliseconds after `devid cmd on-demand` reached nginx workers that still had the old config: the static page, or "Nothing answers". That was nginx's reload, as after any change Devid makes; the same requests a second later woke the command.

## Found on the way: a stray process per stop of devid-site's `next dev`

- **What happens:** every stop of the clone's `next dev` left one `node …/next/dist/telemetry/detached-flush.js` behind. It runs in its own process group with ppid 1, so a group stop does not reach it, and it does not exit by itself. `NEXT_TELEMETRY_DISABLED=1` was set on the command.
- **The owner's machine:** it also has two from its own devid-site, one from the restart for the 0.16.1 update and one from the day before.
- **Cause:** the cause was found on 2026-11-16 (Next 16.4 with fumadocs-mdx's watcher). It is not Devid's, but on demand makes it matter: each sleep would leave one more.
- **Where to fix it:** in devid-site, not in Devid, which should not kill processes outside a command's group. It needs an owner's decision before on-demand is turned on for devid-site.
