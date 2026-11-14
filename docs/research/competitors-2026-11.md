# Competitor research: where other local PHP and Node.js environments fall short

> Scope: pricing, limits and recurring complaints of other local PHP/Node.js environments, and what they suggest for Devid
> Verified against: 2026-11-15 (web research; pricing pages read on that date)
> Status: research. The opportunities at the end are **proposed**, not accepted; each needs its own decision in `DECISIONS.md`.

## How this was made

On 2026-11-15, four research agents read official docs, pricing pages, release notes, GitHub repositories and issues, and forum threads, one group of products each. Every claim below carries its source. "Unverified" marks what could not be confirmed.

Caveats:
- Many complaints come from a single issue or forum thread. Treat them as signals, not proof.
- No product publishes measured idle RAM or CPU numbers, so resource claims are vendor statements or user reports.
- Prices change; re-check a pricing page before quoting it.

## Summary

| Gap | Evidence | Devid today |
|---|---|---|
| Intel Macs without Homebrew or a VM | Herd 2 and Yerd, EnvKit and the 2026 XAMPP builds are arm64 only; Valet and PHP Monitor need Homebrew, which no longer builds Intel bottles; Lerd needs a Podman VM | Implemented: Devid's core. Apple's macOS 26 is the last for Intel, so this market shrinks; Apple silicon is D-044/D-046. |
| Features others charge for | Mail, dumps and Xdebug detection are Herd Pro ($99/yr); ServBay's free tier allows 5 sites and makes mail, tunnels and PHP settings Pro; MAMP PRO is $99/yr; FlyEnv caps unlicensed use at 3 sites; Laragon needs a license since 7.0 | Sites, PHP settings, the extension catalog and Share are free. No mail catcher and no dump viewer. |
| Node.js tooling that takes over the shell | Herd forces its own nvm and overrides fnm; ServBay's Node wins over nvm; FlyEnv adds a hook to `.zshrc` | Devid uses the user's Node and does not touch the shell. No Node version per site. |
| Dev servers only proxied, not run | ServBay, Yerd and FlyEnv proxy a server the user starts (FlyEnv by hand); in containers HMR needs polling, and Next.js advises against `next dev` in Docker | Implemented: commands, the proxy and HMR (Phase 8). Herd 2 starts and idles dev servers, but only with Herd Studio. |
| LAN and phone testing | Herd only tunnels; MAMP needs NAMO, a separate product; Valet needs nginx edits; EnvKit exposes sites on the LAN by default | Not available. A local port per site exists (D-037). |
| Idle cost and system side effects | No published idle numbers; Herd battery and CPU issues; Local's helper keeps using CPU after quit; ServBay broke Wi-Fi after sleep; Docker reserves 50% of RAM | About 30 MB at idle (README), but no published method. |
| Trust | ServBay collects usage data and fingerprints machines for licensing; FlyEnv runs a root helper and had an RCE through its AI chat; users push back on Herd 2's AI features | Open source (AGPL, D-043), no account, no telemetry, no AI features. |

### Risks for Devid found on the way

Yerd, like Devid, redirects ports 80/443 with pf, and has two open bugs:
- the pf port redirect breaks when OrbStack is installed: https://github.com/forjedio/yerd/issues/253
- iCloud Private Relay becomes unavailable: https://github.com/forjedio/yerd/issues/247

Devid's pf anchor should be checked for both (unknown).

## Proposed opportunities for Devid

Ordered by value for the effort. All proposed; none accepted.

1. **A free mail catcher.** Herd Pro, ServBay Pro and MAMP PRO charge for it. An SMTP listener in the daemon that already runs, with the mail shown in the app and the TUI. Reverses part of D-011, which deferred it from v1.
2. **A Node.js version per site** from `.nvmrc` or `.node-version`, using the versions already installed by nvm, fnm or volta, without shell hooks.
3. **LAN and phone testing:** opt-in per site, with a QR code, on top of the local port (D-037). Localhost-only stays the default.
4. **Published idle numbers with the method**, and checks with OrbStack and iCloud Private Relay (see the risks above).
5. **Dev servers started on the first request and stopped when idle.** Parity with Herd 2, without its AI layer.
6. **Laravel + Vite over HTTPS with no config**, and later a **dump viewer** (the largest of these).

The research also backs D-044's order: Linux is Herd's most-voted request.

## Laravel Herd, Valet with PHP Monitor, and Sail

### Laravel Herd (2.1.0, released 2026-11-05)

- **Platforms:**
  - macOS 14.6+ on Apple silicon, no Homebrew needed: https://herd.laravel.com/docs/macos/getting-started/installation.md
  - Intel Macs stay on 1.30.2, and 2.x is arm64 only (read from the Homebrew cask table): https://formulae.brew.sh/cask/herd
  - Windows 10+ with admin rights for the HerdHelper service: https://herd.laravel.com/docs/windows/getting-started/installation
  - No Linux; it is the top request (#329, 101 reactions): https://github.com/beyondcode/herd-community/issues/329
- **Pricing:**
  - Free: PHP, nginx, DNS, Node, sites, per-project config, Herd Studio and the profiler.
  - Pro: $99/yr for 2 devices, with no trial. Adds MySQL/Postgres/Redis and other services, Mail, Insights (dumps), Tinker and Xdebug detection.
  - Teams: $299/yr for 10 licenses.
  - Source: https://herd.laravel.com/
  - The renewal price is unverified. Laravel News gives $79/yr for Pro and $240/yr for Teams (https://laravel-news.com/laravel-herd-2), and the checkout page shows Teams at $99 (https://herd.laravel.com/checkout).
- **PHP:**
  - 7.4–8.6, with a per-site version via `herd isolate`: https://herd.laravel.com/docs/macos/getting-started/updates.md
  - About 55 bundled extensions. Others, swoole included, need `brew install php` plus `pecl install`: https://herd.laravel.com/docs/macos/technology/php-extensions.md
  - Xdebug is free but enabled by hand; the docs list arm64 builds only for PHP 7.4–8.3: https://herd.laravel.com/docs/macos/debugging/xdebug
- **Node.js:**
  - Herd bundles its own nvm, cannot use an existing one, and supports bash/zsh only: https://herd.laravel.com/docs/macos/technology/node-versions.md
  - It overrides fnm (#388): https://github.com/beyondcode/herd-community/issues/388
  - Herd 2 starts Vite or `php artisan dev` automatically and stops it after 5 minutes idle, but only with Herd Studio on: https://herd.laravel.com/docs/macos/herd-studio/dev-server.md
  - Bugs:
    - a 15-second wait on every page in 2.0.3 (#1786): https://github.com/beyondcode/herd-community/issues/1786
    - dev servers start while the feature is off (#1800): https://github.com/beyondcode/herd-community/issues/1800
    - WebSockets fail on secured sites (#1341): https://github.com/beyondcode/herd-community/issues/1341
    - `public/hot` is truncated during Share (#1769): https://github.com/beyondcode/herd-community/issues/1769
- **Sharing:**
  - Through Expose or ngrok, with no LAN mode: https://herd.laravel.com/docs/macos/sites/sharing-sites.md
  - Expose: the free plan has time-limited connections; Pro is $79/user/yr: https://expose.dev/
- **Resource complaints:** no published idle numbers.
  - Idle battery drain (#1429): https://github.com/beyondcode/herd-community/issues/1429
  - Energy use 4× the browser (#1450): https://github.com/beyondcode/herd-community/issues/1450
  - 300% CPU (#947): https://github.com/beyondcode/herd-community/issues/947
  - The helper at 200% CPU after 34 days of uptime (#1760): https://github.com/beyondcode/herd-community/issues/1760
  - 108 orphaned MCP processes using about 620 MB (#1694): https://github.com/beyondcode/herd-community/issues/1694
  - Pushback on the AI features in 2.x (#1773, #1776): https://github.com/beyondcode/herd-community/issues/1776

### Laravel Valet and PHP Monitor

- **Valet:**
  - Free; needs Homebrew: https://laravel.com/docs/12.x/valet
  - Homebrew puts Intel macOS at Tier 3: no new Intel bottles, and it stops running on Intel in or after September 2027: https://docs.brew.sh/Support-Tiers
  - `php`, `php@8.3`, `php@8.4` and `nginx` have no Intel bottles, so they compile from source: https://formulae.brew.sh/api/formula/php.json
  - Per-site PHP via `isolate` or `.valetrc`; v4.13.0 (2026-11-10) also reads composer.json: https://github.com/laravel/valet/releases/tag/v4.13.0
  - Node.js: none. `valet proxy` only forwards to a port: https://github.com/laravel/valet/blob/master/cli/stubs/secure.proxy.valet.conf
  - No mail catcher and no dump viewer; LAN access means editing nginx by hand. The docs claim "roughly 7 MB of RAM".
  - Homebrew updates have broken Valet before (#528, #544): https://github.com/laravel/valet/issues/528
- **PHP Monitor:**
  - MIT, macOS 13.5+, universal build: https://github.com/nicoverbruggen/phpmon
  - Extensions only from the shivammathur tap.
  - Dropping Intel was proposed in October 2026; the maintainer is "not going to change anything yet": https://github.com/nicoverbruggen/phpmon/issues/339

### Laravel Sail

- Free, but needs Docker: https://laravel.com/docs/sail
- Serves `artisan serve` on `http://localhost`, with no `.test` domain and no built-in TLS. A Caddy SSL pull request was never merged: https://github.com/laravel/sail/pull/505
- Extensions and Xdebug need an image rebuild. Mailpit is included, and `sail share` uses Expose.
- Laravel 13's install guide covers php.new and Herd, not Sail: https://laravel.com/docs/13.x/installation
- Moving `vendor` off the bind mount took requests from about 1000 ms to 200 ms in one report: https://dev.to/tylerlwsmith/speed-up-laravel-in-docker-by-moving-vendor-directory-19b9

## ServBay, MAMP / MAMP PRO, Local, Laragon

### ServBay

- **Platforms:** macOS 12+ on Intel or Apple silicon, and Windows 10+. No Linux: https://support.servbay.com/getting-started/system-requirements
- **Pricing:**
  - Free; Pro $9.90/month or $59/year (list price $99), for 1 user on 2 devices; Team $399/year: https://www.servbay.com/pricing
  - No perpetual license ("Continuous income is the driving force"): https://github.com/ServBay/ServBay/issues/13
- **Free-tier limits:** 5 websites. Pro-only:
  - custom PHP and php-fpm settings, and extension settings;
  - the mail server;
  - every tunnel;
  - backups.
- **PHP:** 5.3–8.6, with a per-site version. Xdebug is enabled per PHP version, not per site: https://support.servbay.com/php/modules/xdebug
- **Node.js:**
  - Versions 12–25. ServBay proxies a dev server the user starts; HMR is not documented: https://support.servbay.com/basic-usage/websites/adding-nodejs-development-website
  - Its Node wins over nvm: https://github.com/ServBay/ServBay/issues/10
- **Trust:**
  - Closed source. Collects a per-client UUID, usage and crash data: https://www.servbay.com/privacy-policy
  - Machine-fingerprint licensing since 1.33.0: https://support.servbay.com/getting-started/release-notes
- **Complaints:**
  - Wi-Fi did not reconnect after sleep until its services restarted: https://github.com/ServBay/ServBay/issues/37
  - "System DNS not restored on exit" needed a fix.
  - One report of kernel panics: https://github.com/ServBay/ServBay/issues/30

### MAMP / MAMP PRO

- **Platforms and pricing:**
  - MAMP PRO 7 needs macOS 11+ on Intel or Apple silicon: https://mampgmbh.github.io/Software-Documentation/en/MAMP-PRO-Mac/Getting-started/System-Requirements
  - $99/year; the license does not expire, but updates after the first year cost €99/year: https://www.mamp.info/en/mamp-pro/support/purchase-faq/
  - Free MAMP allows 1 host and 2 PHP versions: https://www.mamp.info/de/vergleichsmatrix/
- **Features:**
  - 7.4 added Xdebug 3.5.0 and a QR code for opening sites on a phone; 7.5 (2026-11-09) moved to Mailpit: https://www.mamp.info/en/release-notes/mac/
  - Mail capture is PRO only. LAN access needs NAMO, a separate product. No Node.js management found (unverified).
- **Complaints:**
  - Weeks-long support replies and MySQL freezes: https://mampsupportforum.com/viewtopic.php?p=670
  - 7.2 broke sites until a downgrade: https://forum.getkirby.com/t/fatal-error-message-after-updating-to-mamp-pro-7-2/33077/4

### Local (WP Engine)

- **Platforms, price and scope:**
  - macOS 12+ (Intel and Apple silicon), Windows and Linux: https://localwp.com/help-docs/getting-started/installing-local/
  - Free since 2021: https://wpengine.com/press-releases/wp-engine-supercharges-wordpress-development-local-pro-free-for-everyone/
  - WordPress only; requests for Laravel support go back years: https://community.localwp.com/t/support-for-laravel-and-other-non-wp-php-frameworks/18474
- **PHP:**
  - Users cannot update patch versions themselves: https://community.localwp.com/t/php-patch-versions-outdated/50550
  - 8.5 came three months after PHP's release: https://community.localwp.com/t/support-for-php-8-5/50827
- **Architecture and complaints:**
  - Electron. An old Electron version was linked to system-wide lag on macOS Tahoe: https://community.localwp.com/t/update-electron-version-for-macos-tahoe-performance/51492
  - A "Local Helper" process kept using 50–70% CPU after quit: https://community.localwp.com/t/local-helper-zombie-process-on-macos/37592
  - Live Links need a Local account and hit ngrok limits: https://community.localwp.com/t/too-many-connections-error-another-err-ngrok-702-post/13612

### Laragon

- Windows only: https://laragon.org/docs/faq
- A license is required since 7.0 (early 2025): https://newreleases.io/project/github/leokhoa/laragon/release/7.0.0
- The free tier is non-commercial only, with a reminder popup that appears more often when Laragon runs minimized: https://laragon.org/free-license-policy
- Licenses range from $10 (non-commercial) to $149 (commercial, perpetual) and $399 (team): https://laragon.org/pricing
- Users complain about the popups; the maintainer says the extra popups when minimized are deliberate: https://github.com/leokhoa/laragon/discussions/985

## Open-source native environments

### FlyEnv (formerly PhpWebStudy)

- **Platforms and license:**
  - macOS on Intel and Apple silicon, Windows and Linux; Electron: https://github.com/xpf0000/FlyEnv
  - BSD-3-Clause, but without a $10 personal license it allows at most 3 local sites: https://flyenv.com/guide/about-license
- **PHP and Node.js:**
  - PHP comes from Homebrew, MacPorts or its own static builds; extensions through those or pecl: https://github.com/xpf0000/FlyEnv/blob/master/docs/deepwiki/php.md
  - Node versions are set through a hook in `.zshrc`: https://flyenv.com/guide/manage-multiple-node-php-versions
  - Dev servers need a reverse-proxy rule added by hand: https://flyenv.com/guide/reverse-proxy-nestjs-multi-servers.html
- **Architecture:** a root helper installed as a LaunchDaemon: https://flyenv.com/guide/about-flyenv-helper
- **Complaints:**
  - high memory and a slow start-up scan: https://github.com/xpf0000/FlyEnv/issues/759
  - nearly a minute to load after an update, and the updater installed the wrong-architecture build: https://github.com/xpf0000/FlyEnv/issues/539
  - Homebrew updates break startup: https://github.com/xpf0000/FlyEnv/issues/962
  - an XSS that led to remote code execution through the AI chat: https://github.com/xpf0000/FlyEnv/issues/809
- **Maintenance:** one main maintainer; 17 releases since May 2026.

### Yerd (v2, Rust and Tauri)

- **Platforms:**
  - macOS on Apple silicon and Linux; Windows is planned: https://yerd.app/guide/getting-started
  - The Intel request is open (https://github.com/forjedio/yerd/issues/29), and the universal-build pull request was closed unmerged (https://github.com/forjedio/yerd/pull/20).
- **Features:**
  - MIT. Its own signed static PHP builds and a version per site: https://github.com/forjedio/yerd/blob/main/docs/guide/php-versions.md
  - Proxies dev servers to `.test` with HTTPS, but does not start them: https://github.com/forjedio/yerd/blob/main/docs/guide/proxies.md
  - Node version management is an open pull request: https://github.com/forjedio/yerd/pull/244
  - Mail capture, a dump inspector, Cloudflare sharing and databases.
- **Bugs:**
  - `xdebug.mode` is ignored under FPM: https://github.com/forjedio/yerd/issues/248
  - Composer does not see custom extensions: https://github.com/forjedio/yerd/issues/236
  - pf and OrbStack: https://github.com/forjedio/yerd/issues/253
  - iCloud Private Relay: https://github.com/forjedio/yerd/issues/247
- **Maintenance:** the last stable release was v2.0.4 (2026-07-25).

### EnvKit

- **Openness:** free, but the source is private: https://github.com/Env-Kit/envkit-releases
- **Platforms:** macOS 12+ on Apple silicon and Windows; Intel is "not planned": https://github.com/Env-Kit/envkit-releases/issues/117
- **Features:**
  - signed and notarized, with its own binaries;
  - PHP 7.4–8.5 with a version per site;
  - Node through nvm and `.nvmrc`;
  - Mailpit, a dump viewer, `*.envkit.net` sharing and an `envkit.json` project file: https://envkit.net/docs
- **Status:** beta.
- **LAN exposure:** sites are on the LAN by default, and on macOS this cannot be turned off while the web server uses a port below 1024: https://github.com/Env-Kit/envkit-releases/issues/184

### Lerd

- Not native: nginx and PHP-FPM run in rootless Podman containers. On macOS it needs Homebrew and a Podman VM with a 3–6 GB ceiling: https://lerd.sh/getting-started/requirements
- MIT. Mostly beta releases. Node through mise, fnm or nvm, and Vite on the site's own domain: https://github.com/lerd-env/lerd

### XAMPP

- The last macOS build on SourceForge is 8.2.4 (2023-04-06): https://sourceforge.net/projects/xampp/files/XAMPP%20Mac%20OS%20X/
- New builds since 2026-11-13 are osx-arm64 only, one installer per PHP version: https://github.com/ApacheFriends/xampp-build/releases
- No per-site PHP and no Node.js.

## Container-based environments and their runtimes

### Runtimes

| Runtime | Cost | Resources on macOS | Notes |
|---|---|---|---|
| Docker Desktop | Free for personal use and companies under 250 staff and $10M revenue; else $9–24/user/month: https://www.docker.com/pricing/ | The VM defaults to 50% of host RAM plus 1 GB swap: https://docs.docker.com/desktop/settings-and-maintenance/settings/ | Resource Saver stops the VM only when no containers run: https://docs.docker.com/desktop/use-desktop/resource-saver/. Synchronized file shares are paid plans only. Supports the current and two previous macOS releases. |
| OrbStack | Free for personal, non-commercial use only; $8/user/month otherwise, freelancers included: https://orbstack.dev/pricing | Memory on demand, about 0.1% idle CPU (vendor claim): https://docs.orbstack.dev/efficiency | macOS 14+. Closed source. |
| Colima | MIT, free: https://github.com/abiosoft/colima | VM defaults: 2 CPUs, 2 GiB RAM | Installs through Homebrew, MacPorts, Nix or Mise only. |
| Podman Desktop | Apache-2.0, free: https://github.com/podman-desktop/podman-desktop | The default 2 GB machine "runs out with more than a project or two": https://docs.ddev.com/en/stable/users/install/docker-installation/ | Rootless Podman cannot bind ports 80/443. |

Apple's macOS 26 is the last release for Intel Macs: https://heise.de/-10438678

### DDEV

- Free (Apache-2.0); macOS 14+, 8 GB RAM: https://docs.ddev.com/en/stable/
- Install steps: a Docker provider, then Homebrew, then `mkcert -install`: https://docs.ddev.com/en/stable/users/install/ddev-installation/
- Mutagen sync stores the code twice, can lose changes made while DDEV is stopped, and needs `node_modules` excluded: https://ddev.com/blog/mutagen-functionality-issues-debugging
- "Most people raise the memory allocation to 6GB or higher": https://docs.ddev.com/en/stable/users/install/performance/
- Cold starts take 9.2–46 s (Apple silicon only): https://ddev.com/blog/docker-performance
- Vite needs `web_extra_exposed_ports` plus host, origin and CORS edits: https://docs.ddev.com/en/stable/users/usage/vite/
- Mailpit is included, and `ddev xdebug on` needs no rebuild.

### Lando

- Free (MIT); macOS 12+, 16 GB RAM recommended, Docker Desktop required: https://docs.lando.dev/getting-started/requirements.html
- OrbStack is unofficial until Lando 4: https://docs.lando.dev/guides/how-do-i-use-orbstack.html
- Speed relies on excluding `vendor` and `node_modules`, which can then no longer be edited from the host: https://docs.lando.dev/config/performance.html
- `lando share` has not worked since 3.0.0-rrc.3: https://docs.lando.dev/cli/v/v3.21.2/share.html

### Node.js HMR in containers

- Next.js: Fast Refresh in Docker on a Mac can "take seconds or even minutes", so run `next dev` outside Docker: https://nextjs.org/docs/app/guides/local-development
- Vite: `usePolling` "leads to high CPU utilization": https://vite.dev/config/server-options

### Yearly cost for one user

| Setup | Solo or small company | Larger company |
|---|---|---|
| DDEV, Lando or Sail + Docker Desktop | $0 | $108–288 |
| + OrbStack | $96 (any commercial use) | $96 |
| + Colima or Podman | $0 | $0 |

## Not checked

- Measured idle RAM, CPU and battery for any product on an Intel Mac. None is published.
- ngrok and NAMO pricing.
- Whether ServBay needs an account or a connection to run.
- Whether MAMP's crash reporting is opt-in.
- EnvKit's GUI stack after its 0.34 rewrite.
- Signing of the 2026 XAMPP builds.
