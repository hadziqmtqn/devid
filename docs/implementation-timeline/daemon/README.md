# Daemon

| Date | Record |
|---|---|
| 2026-11-02 | [M1: workspace, daemon socket and `devid status`](2026-11-02-m1-daemon-skeleton.md) |
| 2026-11-02 | [M2: PHP manager and php-fpm supervision](2026-11-02-m2-php-manager.md) |
| 2026-11-02 | [M3: sites, nginx and CI](2026-11-02-m3-sites-nginx.md) |
| 2026-11-02 | [M4: local CA and HTTPS](2026-11-02-m4-https.md) |
| 2026-11-02 | [M5: DNS responder and system helper](2026-11-02-m5-dns-helper.md) |
| 2026-11-02 | [M6: setup, doctor, uninstall, LaunchAgent and shims](2026-11-02-m6-setup.md) |
| 2026-11-02 | [M7: Caddy and Apache adapters, `devid server use`](2026-11-02-m7-servers.md) |
| 2026-11-02 | [Phase 5 R3: install and update from releases](2026-11-02-r3-release-downloads.md) |
| 2026-11-02 | [Phase 5 R4: devid releases, install.sh and self-update](2026-11-02-r4-devid-release.md) |
| 2026-11-03 | [Fix: `php artisan serve` through the shim (v0.2.1)](2026-11-03-shim-artisan-serve.md) |
| 2026-11-03 | [Fix: update check with an unreadable installed version (v0.2.2)](2026-11-03-update-unknown-version.md) |
| 2026-11-03 | [`devid setup` installs PHP and a web server (v0.2.3)](2026-11-03-setup-installs-php-server.md) |
| 2026-11-03 | [Fix: `.test` sites slow while the Mac is busy (LaunchAgent priority)](2026-11-03-agent-priority.md) |
| 2026-11-06 | [Ignore and unignore a parked site, in the CLI, TUI and app (v0.5.0)](2026-11-06-site-ignore.md) |
| 2026-11-08 | [Phase 8 A2: app types, PHP / static / proxy (v0.8.0)](2026-11-08-a2-app-types.md) |
| 2026-11-08 | [Phase 8 A3: a local port per site, for tunnels (v0.9.0)](2026-11-08-a3-local-port.md) |
| 2026-11-08 | [Fix: self-update falls back to the GitHub API (v0.9.1)](2026-11-08-self-update-api-fallback.md) |
| 2026-11-08 | [Phase 8 A4: user commands run by the daemon (v0.10.0)](2026-11-08-a4-commands.md) |
| 2026-11-08 | [devid is GitHub's Latest release; PHP/nginx builds from `runtime-latest` (D-038, v0.10.1)](2026-11-08-github-latest.md) |
| 2026-11-08 | [Phase 8 A6: Share through a Cloudflare quick tunnel (v0.12.0)](2026-11-08-a6-share.md) |
| 2026-11-09 | [Fix: Share reaches the site on its local port or :80, not on 8088 (v0.12.1)](2026-11-09-share-origin.md) |
| 2026-11-09 | [Starting a command warns when the dev server port is held outside Devid (v0.12.2)](2026-11-09-command-port-warning.md) |
| 2026-11-09 | [Commands run again after Devid restarts for an update (D-040, v0.12.3)](2026-11-09-resume-after-update.md) |
| 2026-11-12 | [A proxy site's commands get PORT from "Forward to" (D-041, v0.12.4)](2026-11-12-command-port-env.md) |
| 2026-11-12 | [A new proxy site gets a free dev server port; `devid cmd <site>` lists (D-042, v0.12.5)](2026-11-12-auto-port.md) |
| 2026-11-15 | [Fix: ports 80/443 are checked live, and `helper install` undoes `skip on lo0` (D-047)](2026-11-15-pf-lo0-skip.md) |
| 2026-11-16 | [Fix: a service could stay "starting" for days because a sibling held its exec pipe](2026-11-16-spawn-fd-leak.md) |
| 2026-11-16 | [Mail in the daemon and the CLI: Devid runs Mailpit (Phase 9 M3, D-049)](2026-11-16-mail-service.md) |
| 2026-11-16 | [The inbox in the daemon and the CLI: list, show, delete, save (Phase 9 M6a)](2026-11-16-mail-inbox-requests.md) |
| 2026-11-17 | [Fix: Share and the inbox requests wait for a service that is still starting](2026-11-17-wait-for-a-starting-service.md) |
| 2026-11-21 | [Fix: a proxy site answers as soon as its dev server starts, not 10 s later (Phase 10 O2, v0.16.1)](2026-11-21-nginx-stale-502.md) |
| 2026-11-21 | [Dev servers on demand on nginx: the daemon and the CLI (Phase 10 O3, D-054)](2026-11-21-on-demand-nginx.md) |
