# Devid

Devid is a local development environment for Laravel, PHP and Node.js on Macs with Apple silicon (M series) or Intel, and on Linux (Ubuntu and Arch, x86_64). Every project folder you park or link gets its own `https://<name>.test` address with a trusted certificate. Devid ships PHP, the web servers and the databases prebuilt, so it needs no Homebrew, no Docker and no virtual machine. You use it from the command line, from a terminal UI (TUI) or, on macOS, from the menu-bar desktop app.

**Website and guide:** [devid.bekenweb.com](https://devid.bekenweb.com), with the user guide at [devid.bekenweb.com/docs](https://devid.bekenweb.com/docs).

> **Status:** v0.21, used daily on an M1 Mac (an Intel Mac before November 2026, D-052). Apple silicon builds (since v0.14) are tested by hand on the M1 and smoke-tested on GitHub's runners before each release (D-046). Linux builds (Ubuntu since v0.19, D-057; Arch since v0.20, D-059) go through setup, a site over HTTPS and uninstall on Ubuntu and on Arch on GitHub's runners before each release, and were tested by hand on Ubuntu 24.04 Desktop, Arch Linux with KDE Plasma and CachyOS. This repository holds the source, under the [GNU AGPL v3](LICENSE) (D-043). Issues are [here](https://github.com/hadziqmtqn/devid/issues/new/choose) (D-045). The builds and the releases are in [`devid-binaries`](https://github.com/hadziqmtqn/devid-binaries); the website and the guide are in [`devid-site`](https://github.com/hadziqmtqn/devid-site).

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

Homebrew moved Intel Macs to Tier 3: there are no more prebuilt packages, so Valet's PHP and nginx build from source. Devid ships them prebuilt, without a package manager. PHP, nginx, Caddy, Mailpit, MySQL, PostgreSQL and MongoDB come from Devid's releases and are sha256-checked. Everything runs as native processes: at idle, Devid with nginx and two PHP versions uses about 30 MB.

## What you get

- **Sites:** park `~/Projects` or link one folder. Laravel, WordPress, Symfony, CodeIgniter, Vite, Next.js and other projects are detected per folder and served to match, over HTTPS from a local CA limited to `.test`.
- **PHP:** 8.3, 8.4, 8.5 and 8.6 side by side, with a version pinned per site. Xdebug and pcov are included, off by default. 8.5 stays the default for new installs while 8.6's Xdebug is a preview.
- **Node.js:** Vite, Next.js, Nuxt and Node servers are proxied to your dev server, hot reload included. Node is yours, for example from nvm.
- **Dev servers:** save a command per site once, such as `npm run dev` or a queue worker, then start and stop it from Devid. On demand, a site's first visit starts its dev server, and Devid stops it once the site has been idle, so it uses memory only while you work on it (`devid cmd on-demand`). A Laravel site's Vite, started from Devid, serves HTTPS with the site's certificate, so Safari loads its scripts and CSS too.
- **Extensions:** intl, gd, redis and PDO are built in. apcu, igbinary, imagick, ldap, memcached, mongodb, msgpack and swoole come prebuilt from Devid's catalog, with nothing to compile. For PHP 8.6 the catalog has imagick, ldap and mongodb so far; the others follow when their own releases support 8.6.
- **Web server:** nginx by default, or Caddy, or Apache on macOS. One runs at a time; switch with `devid server use`.
- **Share:** `devid share` gives a site a temporary public link through a Cloudflare quick tunnel (it uses your own `cloudflared`, from Homebrew or `~/.local/bin`). `devid app port` serves a site on a local port for a tunnel of your own.
- **Mail:** `devid mail on` catches the mail your apps send, in an inbox on your computer that the desktop app shows. Laravel's SMTP mailer and PHP's `mail()` both work, and nothing reaches a real mailbox. Devid runs Mailpit for it.
- **Databases:** `devid db start mysql` runs MySQL, PostgreSQL or MongoDB on 127.0.0.1, at the usual port or the next free one, so it can sit next to DBngin or Homebrew. `devid db user add mysql shop` makes an account with its own database and a generated password, `devid db env mysql shop` prints its `.env` lines, and a new password is one more command. The TUI and the app's Databases page do the same, and the app opens each client in the terminal app you pick. Each database is off until you start it, and its data stays when you stop or uninstall Devid.
- **Terminal commands:** `php`, `composer`, `php8.3` to `php8.6`, `nginx -t`, `caddy` and `mailpit` run Devid's own copies from any terminal, editor or script, and so do `mysql`, `psql`, `mongosh` and their dump tools once a database is installed. Add to PATH in the app's Settings (or `devid path on`) sets this up.
- **Interfaces:** the CLI, the TUI and, on macOS, the menu-bar app. Every action works the same in all three. `devid doctor` checks the whole setup and tells you how to fix what's wrong.

## Install

With the desktop app:

```sh
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh -s -- --gui
```

The CLI and TUI only:

```sh
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
```

On Linux (x86_64), install curl and certutil first, then use the same command. On Ubuntu 24.04 or 26.04:

```sh
sudo apt install curl libnss3-tools
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
```

On Arch Linux and the distributions built on it, such as CachyOS (nftables too, which plain Arch may lack):

```sh
sudo pacman -S --needed curl nss nftables
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
```

Devid is an open-source personal project, provided as is. The macOS builds are ad-hoc signed and not notarized; the command installs them without a Gatekeeper prompt. Setup asks for your password once for `.test` DNS, Devid's CA and ports 80/443, and `devid uninstall` undoes it. The [guide](https://devid.bekenweb.com/docs) covers the install options, sites, dev servers on demand, Laravel and Vite, PHP and extensions, web servers, Mail, databases, the desktop app, the TUI, terminal commands, HTTPS, updates, troubleshooting and uninstall.

## Requirements

| | |
|---|---|
| macOS | Mac with Apple silicon (M series) or Intel, macOS 12 or newer. Tested by hand on an M1 with macOS 26 Tahoe and on Intel with macOS 14 Sonoma |
| Linux | x86_64 with systemd: Ubuntu 24.04 or 26.04 with `curl` and `libnss3-tools`, or Arch Linux and the distributions built on it with `curl`, `nss` and `nftables`. Where systemd-resolved is off, as on a plain Arch install, setup turns it on and needs NetworkManager. The CLI and the TUI; no desktop app and no Apache yet. Tested by hand on Ubuntu 24.04 Desktop, Arch Linux (KDE Plasma) and CachyOS (KDE Plasma, fish) |
| Setup | Admin password, once: `.test` DNS, Devid's CA and ports 80/443 |
| Ports | 80 and 443 free (stop Valet first) |
| Databases | MySQL, and MongoDB 8.0 and 9.0, need macOS 14 or newer; MongoDB 7.0 runs on macOS 12. MongoDB needs a processor with AVX on x86_64 |
| Not included | Node.js: Devid uses yours |

Everything lives in `~/.devid`. `devid uninstall` undoes setup.

## Roadmap

No dates yet. New macOS versions are added as GitHub's macOS runners make testing possible. The reasons behind each item are in [`docs/structure-of-app/DECISIONS.md`](docs/structure-of-app/DECISIONS.md). Each release and what it added: [devid.bekenweb.com/roadmap](https://devid.bekenweb.com/roadmap).

**Shipped**

- `*.test` sites: park, link, and a PHP version per site.
- PHP 8.3 to 8.6, and the extension catalog (D-009, D-062).
- nginx, Caddy and Apache, with automatic HTTPS.
- Node.js projects and dev servers.
- Sharing a site through a Cloudflare tunnel.
- The CLI, the TUI and the menu-bar app.
- Apple silicon: Devid, PHP, the extensions and the web servers built for arm64, smoke-tested on GitHub's Apple silicon runners before each release (D-046), and in daily use on an M1 (D-052).
- Mail: Devid runs Mailpit to catch the mail your apps send, with the inbox in the desktop app (D-049, D-051).
- Terminal commands: Devid's PHP versions, nginx, Caddy and Mailpit by name from any shell, with Add to PATH in the app (D-053).
- Dev servers on demand: a site's first visit starts its dev server, and Devid stops it when the site has been idle, on nginx, Caddy and Apache (D-054).
- Laravel and Vite: Vite started from Devid serves HTTPS with the site's certificate, so a Laravel page works in Safari too, hot reload included (D-056).
- Linux: Ubuntu 24.04 and 26.04 on x86_64, with the CLI and the TUI. `.test` goes through systemd-resolved and ports 80/443 through nftables, and Devid's CA is trusted by the system, Chrome and Firefox (D-057).
- Arch Linux: Arch and the distributions built on it, such as CachyOS, on x86_64. Setup turns systemd-resolved on where it is off and trusts Devid's CA through p11-kit, and fish gets its own PATH file, as bash and zsh do (D-059).
- Databases: MySQL, PostgreSQL and MongoDB, each off until started, with an account and its `.env` lines in one step, in the CLI, the TUI and the app (D-061).

**Next**

- macOS 15 up to the latest macOS, on Intel, tested as far as GitHub's macOS runners allow.
- A shorter install command, on Devid's own domain.

**Later**

- Windows (D-044). It is the largest effort: it needs its own PHP builds and has no php-fpm.
- More of Linux: the desktop app, arm64, and Debian, Fedora and other distributions (D-057, D-059).
- Developer tools: a dump viewer.

**Out of scope:** Docker- or VM-based setups.

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

CI runs the same checks on every pull request. Every Monday, a workflow also makes each `devid new` template with the real create tools on macOS and Ubuntu (`scripts/new-e2e.sh`). To publish a release, push a `v*` tag. The release workflow then publishes `devid` to `devid-binaries`. A change users can see also updates the guide, in a `devid-site` pull request that is merged when the release ships (D-034).

The plans and decisions are in [`docs/`](docs/README.md). Start with the project context, then read the architecture and the decisions.

To contribute, read [CONTRIBUTING.md](CONTRIBUTING.md): where issues go, running a development copy next to an installed Devid, and what a pull request needs. Report security problems privately, as [SECURITY.md](SECURITY.md) explains.

## License

Devid is free software under the [GNU Affero General Public License v3.0](LICENSE) (`AGPL-3.0-only`). It comes without any warranty. If you distribute Devid, changed or not, you must offer its source under the same license, and a changed version offered to users over a network must offer its source too.
