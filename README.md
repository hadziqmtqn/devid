# Devid

Devid is a local development environment for Laravel, PHP and Node.js on Intel Macs. Every project folder you park or link gets its own `https://<name>.test` address with a trusted certificate. Devid ships PHP and the web servers prebuilt, so it needs no Homebrew, no Docker and no virtual machine. You use it from the command line, from a terminal UI (TUI) or from the menu-bar desktop app.

**Website and guide:** [devid.bekenweb.com](https://devid.bekenweb.com), with the user guide at [devid.bekenweb.com/docs](https://devid.bekenweb.com/docs).

> **Status:** v0.12, used daily on one Mac. It runs on Intel Macs only for now. This repository holds the source, under the [GNU AGPL v3](LICENSE) (D-043). The builds, the releases and the issue tracker are in [`devid-binaries`](https://github.com/hadziqmtqn/devid-binaries); the website and the guide are in [`devid-site`](https://github.com/hadziqmtqn/devid-site).

```sh
devid park ~/Projects                  # every subfolder becomes <folder>.test
devid cmd add app dev -- npm run dev   # Devid starts and stops your dev server
devid sites
#  https://shop.test   Laravel   PHP 8.4
#  https://app.test    Vite      proxy :5173
```

## Contents

- [Why Devid](#why-devid)
- [What you get](#what-you-get)
- [Install](#install)
- [Requirements](#requirements)
- [Roadmap](#roadmap)
- [Developing Devid](#developing-devid)
- [License](#license)

## Why Devid

Homebrew moved Intel Macs to Tier 3: there are no more prebuilt packages, so Valet's PHP and nginx build from source. Devid ships them prebuilt, without a package manager. PHP, nginx and Caddy come from Devid's releases and are sha256-checked. Everything runs as native processes: at idle, Devid with nginx and two PHP versions uses about 30 MB.

## What you get

- **Sites:** park `~/Projects` or link one folder. Laravel, WordPress, Symfony, CodeIgniter, Vite, Next.js and other projects are detected per folder and served to match, over HTTPS from a local CA limited to `.test`.
- **PHP:** 8.3, 8.4 and 8.5 side by side, with a version pinned per site. Xdebug and pcov are included, off by default.
- **Node.js:** Vite, Next.js, Nuxt and Node servers are proxied to your dev server, hot reload included. Node is yours, for example from nvm.
- **Dev servers:** save a command per site once, such as `npm run dev` or a queue worker, then start and stop it from Devid.
- **Extensions:** intl, gd, redis and PDO are built in. apcu, igbinary, imagick, ldap, memcached, mongodb, msgpack and swoole come prebuilt from Devid's catalog, with nothing to compile.
- **Web server:** nginx by default, or Caddy or Apache. One runs at a time; switch with `devid server use`.
- **Share:** `devid share` gives a site a temporary public link through a Cloudflare quick tunnel (it uses your own `cloudflared`). `devid app port` serves a site on a local port for a tunnel of your own.
- **Interfaces:** the CLI, the TUI and the menu-bar app. Every action works the same in all three. `devid doctor` checks the whole setup and tells you how to fix what's wrong.

## Install

With the desktop app:

```sh
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh -s -- --gui
```

The CLI and TUI only:

```sh
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
```

Devid is an open-source personal project, provided as is. The builds are ad-hoc signed and not notarized; the command installs them without a Gatekeeper prompt. Setup asks for your password once for `.test` DNS, Devid's CA and ports 80/443, and `devid uninstall` undoes it. The [guide](https://devid.bekenweb.com/docs) covers the install options, sites, PHP and extensions, web servers, the desktop app, the TUI, HTTPS, updates, troubleshooting and uninstall.

## Requirements

| | |
|---|---|
| System | Intel Mac, macOS 12 or newer (tested on 14 Sonoma) |
| Setup | Admin password, once: `.test` DNS, Devid's CA and ports 80/443 |
| Ports | 80 and 443 free (stop Valet first) |
| Not included | Databases (use a tool such as DBngin), and Node.js: Devid uses yours |

Everything lives in `~/.devid`. `devid uninstall` undoes setup.

## Roadmap

No dates yet. New macOS versions are added as GitHub's macOS runners make testing possible. The reasons behind each item are in [`docs/structure-of-app/DECISIONS.md`](docs/structure-of-app/DECISIONS.md).

**Shipped**

- `*.test` sites: park, link, and a PHP version per site.
- PHP 8.3 to 8.5, and the extension catalog.
- nginx, Caddy and Apache, with automatic HTTPS.
- Node.js projects and dev servers.
- Sharing a site through a Cloudflare tunnel.
- The CLI, the TUI and the menu-bar app.

**Next**

- Apple silicon: Devid, PHP, the extensions and the web servers built for arm64, and tested on GitHub's Apple silicon runners (D-044).
- macOS 15 up to the latest macOS, on Intel, tested as far as GitHub's macOS runners allow.
- A shorter install command, on Devid's own domain.

**Later**

- Linux (Debian-based), then Windows, one platform at a time (D-044). Windows is the largest effort: it needs its own PHP builds and has no php-fpm.
- Developer tools: a mail catcher and a dump viewer.

**Out of scope:** databases (use a tool such as DBngin), and Docker- or VM-based setups.

## Developing Devid

The source is a Rust workspace:

- `crates/daemon`: the daemon library;
- `crates/cli`: the `devid` binary and the TUI;
- `crates/gui`: the desktop app (Tauri v2). It is not a default member, so build it with `-p devid-gui`; `scripts/bundle-app.sh` makes `Devid.app`.

```sh
cargo build
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on every pull request. To publish a release, push a `v*` tag. The release workflow then publishes `devid` to `devid-binaries`. A change users can see also updates the guide, in a `devid-site` pull request that is merged when the release ships (D-034).

The plans and decisions are in [`docs/`](docs/README.md). Start with the project context, then read the architecture and the decisions.

## License

Devid is free software under the [GNU Affero General Public License v3.0](LICENSE) (`AGPL-3.0-only`). It comes without any warranty. If you distribute Devid, changed or not, you must offer its source under the same license, and a changed version offered to users over a network must offer its source too.
