# Phase 11 Plan: Laravel + Vite over HTTPS

> Scope: a Laravel site's Vite dev server works over Devid's HTTPS in every browser, Safari included, without config (D-056)
> Verified against: 2026-11-22 (v0.17.0 on the owner's M1; Vite 8.3.5, laravel-vite-plugin 3.1.3, Chrome 158, Safari 26.6)
> Status: proposed 2026-11-22. V1 (spike) done; the rest waits for the owner's go-ahead.

A Laravel site runs `npm run dev` (Vite) next to its PHP. laravel-vite-plugin writes Vite's address to `public/hot`, for example `http://[::1]:5173`, and Laravel's `@vite` tags load the page's scripts and CSS from there. The page itself comes from `https://<site>.test`.

Herd and Valet make Vite serve HTTPS with the site's certificate: the plugin finds that certificate in their config folders. With Devid the plugin finds nothing, so Vite stays on plain HTTP. Whether that matters depends on the browser.

On 2026-11-22 the owner picked this over Linux and Windows as the next work (the research's opportunity 6, `docs/research/competitors-2026-11.md`).

## Ground rules

- **Client rule (D-012):** the daemon decides what a command gets. The CLI, the TUI and the app only show it.
- **Only the user's own commands:** nothing starts by itself (D-036). Devid adds two variables to a command the user saved and started.
- **The project's settings win:** a project that sets its own Vite certificate keeps it.
- **Resources:** one read of the project's `.env` when a command starts. No timer, no watcher and no new process.
- **Owner's machine:** tests use their own `DEVID_HOME`, spare ports and copies of projects. `npm run dev` never runs in the owner's erdbpro-saas: its `public/hot` would reach the owner's tunnel visitors.

## V1 results (2026-11-22)

### Setup

- **Test daemon:** its own `DEVID_HOME` and `HOME`, on ports 28189 and 28544.
  - Its CA files are symlinks to the real CA, so its site certificate is trusted by macOS like the owner's own.
  - `ensure_ca()` writes nothing when the CA and its TLD file exist.
- **The fixture:** a PHP site, `vitespike`:
  - an APFS clone of erdbpro-saas's `node_modules` (Vite 8.3.5, laravel-vite-plugin 3.1.3, `@vitejs/plugin-react`, `@tailwindcss/vite`);
  - a React entry with a component in its own file, and Tailwind CSS;
  - a `public/index.php` that prints Laravel's `@viteReactRefresh` and `@vite` tags from `public/hot`.
- **Vite:** it ran as a Devid command on port 5190.
- **Reporting:** the page posted what it saw (rendered text, CSS, Vite's console lines) to a PHP endpoint. That way Safari could be checked without changing its settings.

### Results

| | Vite as today (`public/hot` = `http://[::1]:5190`) | Vite over HTTPS with the site's certificate (`https://vitespike.test:5190`) |
|---|---|---|
| Chrome 158 | works: scripts, CSS, the HMR WebSocket; Fast Refresh keeps state (2 clicks stayed after an edit) | works, with HMR |
| Safari 26.6 | blank page: all 4 requests to `http://[::1]:5190` blocked, with no script and no CSS | works: page, CSS, and HMR (`hot updated`, no reload) |

### Findings

- **Chrome and Safari differ:** Chrome counts loopback (`[::1]`, `127.0.0.1`, `localhost`) as trustworthy, so an HTTPS page may load scripts and a WebSocket from it. Safari blocks them as mixed content.
- **CORS is already fine:** the plugin allows `*.test` origins and `APP_URL`.
- **Two ways the plugin turns HTTPS on:**
  - **`detectTls`** looks only in Herd's and Valet's config folders (`~/Library/Application Support/Herd/config/valet`, `~/.config/valet`). Neither exists with Devid.
  - **`VITE_DEV_SERVER_KEY` and `VITE_DEV_SERVER_CERT`.** With these, Vite serves HTTPS on the host of `APP_URL`, sets HMR to that host, and writes `https://<host>:<port>` to `public/hot`. Vite then listens on 127.0.0.1, which the site's name resolves to through Devid's DNS.
- **Devid's variables win:** Vite's `loadEnv` gives the process environment priority over `.env`.
- **APP_URL decides whether it works:**
  - With `APP_URL=https://vitespike.test` in `.env`, the two variables alone were enough. erdbpro-saas's `.env` already names `https://erdbpro-saas.test`.
  - With `APP_URL=http://localhost:8000` and the two variables, Vite does not start: `getaddrinfo ENOTFOUND localhost:8000`. So Devid must not set them when `APP_URL` names another host.
- **Not checked:**
  - Firefox, which is not installed on the owner's Mac.
  - The app's built-in browser pane, which blocks `[::1]` requests itself (`ERR_BLOCKED_BY_CLIENT`) and so cannot stand in for a browser here.

## Design (D-056)

- **When the daemon looks:** a PHP site's command starts. The daemon reads two things from the project's `.env`:
  - the host of `APP_URL`;
  - whether `VITE_DEV_SERVER_KEY` or `VITE_DEV_SERVER_CERT` is set.
- **The variables:** the command gets `VITE_DEV_SERVER_KEY` and `VITE_DEV_SERVER_CERT`, pointing at the site's key and certificate in `~/.devid/certs`. This happens only when both hold:
  - `APP_URL`'s host is the site's host, with no port (`https://erdbpro-saas.test`; `http://` works too, since the variables make Vite use HTTPS);
  - the project sets neither variable.
- **Who gets them:** every command of that site. Queue workers and the scheduler ignore the variables, and `composer run dev`, which starts Vite among other processes, works too.
- **Otherwise nothing changes.** If the project uses laravel-vite-plugin and `APP_URL` names another host:
  - `devid cmd`, the TUI and the app show one note: for Safari, set `APP_URL=https://<site>.test` in `.env`.
  - It stays on the page, since it needs action (D-055).
- **Vite started in the user's own terminal** gets nothing from Devid. The guide gives the two lines for `.env`, with absolute paths, since the plugin does not expand `~`.
- **Certificates:** they keep their paths when Devid renews them. A Vite that runs across a renewal keeps the old certificate until it restarts.
- **Not chosen:**
  - **Setting `APP_URL` on the command too.** Laravel keeps an `APP_URL` from the environment over `.env`. Queue workers, and Vite plugins that call `artisan` (such as Wayfinder), would then see another `APP_URL` than the app.
  - **Certificates in `~/.config/valet`,** where `detectTls` looks. Other tools take that folder to mean Valet is installed.
  - **Serving Vite through the site's web server.** Vite's module paths and CSS URLs still point at Vite's own address, so the project's `base` would have to change.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| V1 | **Spike** | the answers are in this doc (done 2026-11-22, see "V1 results") |
| V2 | **Daemon and clients:** the two variables on a PHP site's commands; the `APP_URL` note in the CLI, the TUI and the app | the fixture's Vite, started from Devid, works in Safari and Chrome with HMR; a project whose `APP_URL` names another host starts Vite as before and shows the note; unit tests cover the `.env` cases |
| V3 | **Docs and release 0.18.0:** the guide's Laravel page, README, timeline record | released and installed on the owner's Mac |

V2 is one PR. The note is a single line in the style of D-055, so no design board comes first unless the owner asks for one.

## Not in this phase

- **Waking Vite on demand.** Phase 10's reason still holds: the browser talks to Vite directly, not through the site's web server.
- **Sharing a site while Vite runs.** A visitor's page still points at the local Vite. Building the assets (`npm run build`) before sharing stays the way. A warning when `public/hot` exists is a possible follow-up.
- **Vite without laravel-vite-plugin** (Symfony, WordPress), whose tools read their own settings.
- **Firefox,** until it is checked.
