# M3: sites, nginx and CI

> Date: 2026-11-02 · Branch: `phase2/m3-sites` · Plan: [Phase 2](../../roadmap/phase-2.md)

## Outcome

- **Sites:** `devid park [dir]`, `unpark`, `link [name] [--path dir]`, `unlink` and `sites`.
  - Parked subfolders and links become `<name>.test`.
  - Names follow DNS-label rules (`My_App.v2` → `my-app-v2`); hidden and invalid folders are skipped.
  - A link overrides a parked folder with the same name.
- **Per-site PHP:** `devid isolate <ver> [--site name]` and `unisolate`. Without `--site`, they use the site that contains the current folder. The daemon runs php-fpm for the default version plus every version a site uses, and stops masters nobody needs.
- **nginx (D-015):**
  - `devid server install nginx --from <file>`.
  - The daemon renders `~/.devid/nginx/conf/nginx.conf` for all sites on `127.0.0.1:<http_port>` (default 8088), with a 404 default server and the Laravel FastCGI buffers.
  - It checks every new config with `nginx -t`, supervises nginx, and reloads it with SIGHUP on change.
  - Paths that cannot be quoted safely are left out of the config rather than injected.
- **Transactional config changes:** if nginx rejects a new config, the command fails, `config.toml` is not saved, and the previous `nginx.conf` stays active.
- **Supervisor refactor:** a generic `Spec` (program, args, env, pid file, process name), so php-fpm and nginx share the restart, orphan-cleanup and stop logic.
- **CI:** `.github/workflows/ci.yml` runs `cargo fmt --check`, `clippy -D warnings` and `cargo test` on Ubuntu for every PR and every push to `main`. `rustfmt.toml` sets `max_width = 120`.

## Problems found and fixed

- Integration tests left orphaned daemons when they panicked. The orphans held cargo's output pipe and the run hung. A `Daemon` guard in `tests/common` now always stops the daemon on drop.
- macOS temp paths are symlinked (`/var` → `/private/var`). The CLI sends canonical paths, so the tests now use canonical temp roots.
- nginx tried to open its compiled-in error log before reading our config. The error log is now passed with `-e`.

## Verification

- `cargo test`: 9 tests pass. They include `park_link_isolate_and_nginx` (park, link, isolate from `--site` and from the cwd, PHP masters following site use, config content, SIGHUP reload, rollback on a rejected config, validation errors) and unit tests for name rules, site resolution and nginx rendering (including skipping an injection attempt). The fakes keep the suite CI-safe.
- Manual check with the real static PHP 8.4/8.5 and nginx 1.30.6 in a temp `DEVID_HOME`, with `erdbpro-saas` linked and isolated to 8.4 and `spike/` parked:
  - `/` and `/login` return 200 (Inertia `data-page` present), `/favicon.ico` 200, `/.env` 403 and an unknown path Laravel's 404;
  - other sites run on 8.5;
  - an unknown host gets "No Devid site for this host.";
  - the daemon stops cleanly.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.
