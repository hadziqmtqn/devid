# Vite over HTTPS for Laravel sites (Phase 11 V2, D-056)

> Date: 2026-11-22 · Branch: `feat/vite-https`

## Problem

laravel-vite-plugin writes Vite's address to `public/hot`, and with Devid that was plain HTTP (`http://[::1]:5173`). Chrome loads it from an `https://<site>.test` page because it trusts loopback. Safari blocks it as mixed content, so a Laravel page with `@vite` stayed blank (Phase 11 V1).

## Change

- **`sites::vite_tls(site)`:**
  - **When it answers:** only for a PHP site whose `package.json` lists laravel-vite-plugin; otherwise None.
  - **What it reads:** `APP_URL`, `VITE_DEV_SERVER_KEY` and `VITE_DEV_SERVER_CERT`, from the `.env` files Vite reads in development mode. Later files win.
  - **Results:**
    - `Own` when the project sets either variable;
    - `Devid` when `APP_URL`'s host, port included, is the site's host;
    - otherwise `AppUrl { app_url, want }`.
- **`commands::site_vars(paths, site)`:**
  - Replaces the `port` argument of `Commands::start`.
  - Returns `PORT` for a proxy site (D-041). It also returns `VITE_DEV_SERVER_KEY` and `VITE_DEV_SERVER_CERT` with `~/.devid/certs/<host>.key` and `.crt` when `vite_tls` is `Devid` and both files exist.
  - Start, the update resume and on-demand all use it. Share passes nothing.
- **The sites reply:** gains `vite`, which serializes `vite_tls`.
- **Clients:**
  - **CLI:** `devid cmd` prints one `vite:` line. `vite_note()` words the fix.
  - **TUI:** the site menu shows the note first.
  - **App:** the Commands card shows the note (`viteNote`). With `devid`, the heading's tooltip says Vite gets the site's certificate.

## Why this way

- **Vite's own mechanism:** the plugin already reads the two variables and listens on `APP_URL`'s host. Vite's `loadEnv` lets the process environment win over `.env`, so values that Devid sets reach it without touching the project.
- **No change when `APP_URL` does not match:** with `APP_URL` on another host, the plugin would make Vite listen there, and `localhost:8000` failed with `ENOTFOUND`.
- **`APP_URL` is never set,** since Laravel keeps an environment `APP_URL` over `.env`, and queue workers would see another value than the app (D-056, "Not chosen").

## Verified

- **The V1 fixture** (Vite 8.3.5, laravel-vite-plugin 3.1.3, React, Tailwind) ran in a test daemon with its own `DEVID_HOME` on spare ports, with the branch's build. Its command is plain `npm run dev -- --port 5190 --strictPort`.
  - **`APP_URL=https://vitespike.test`:** `public/hot` became `https://vitespike.test:5190`, and Vite's process had both variables.
    - Safari 26.6 loaded the page and Vite connected; an edit hot-updated `Counter.tsx`.
    - Chrome 158 did the same, keeping state.
  - **`APP_URL=http://localhost:8000`, and an empty one:** Vite started on `http://[::1]:5190` as before, and `devid cmd` showed the note.
- **The app's note and tooltip:** checked in the app's browser with the real `app.js`/`app.css` against the test daemon's replies.
- **Tests:** `vite_tls_from_env`, `site_vars_port_and_vite_certificate` and `vite_note_in_the_site_menu`. `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (132 tests) pass.
- **Firefox:** not checked; it is not installed.
