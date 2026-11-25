# Phase 15 Plan: Creating projects

> Scope: `devid new` creates a PHP, Laravel, WordPress, Symfony, CodeIgniter 4, static or Node.js project and serves it at `https://<name>.test` right away, through the CLI, the TUI and the app (D-063)
> Verified against: 2026-11-25 (Devid 0.20.0, main 1340d22; Laravel 13 docs; WordPress 7.1; create-vite 9; WP-CLI's handbook)
> Status: accepted 2026-11-25 (D-063): the owner took every recommendation ("saya ikut rekomendasi kamu") and the official create tools for Node.js ("Ya, tulis rencananya"). It starts after Phase 14 (0.21.0). Its answers are in "Owner's answers".

On 2026-11-25 the owner asked for a feature that creates projects: "buatkan fitur baru untuk create project PHP, Laravel, Wordpress, NodeJs, dll". They asked for suggestions before any documentation. This plan records the suggestions they accepted.

## What exists

- **Sites** (D-011, D-029):
  - parked folders and linked folders become `<name>.test`, with HTTPS from Devid's CA;
  - `devid isolate` pins a site to a PHP version.
- **App types** (D-035): Devid detects 11 presets (Laravel, CodeIgniter 4 and 3, Symfony, WordPress, PHP, Next.js, Nuxt, Node.js, Vite, static). Each sets the serving mode: `php`, `static` or `proxy`, the last to a dev server's port.
  - The proxy rewrites `Host` and `Origin` for the dev server unless `pass_host` is on (Phase 8). Vite's `allowedHosts` and Next.js's `allowedDevOrigins` therefore accept `.test` without changes to the project.
- **Commands** (D-036, D-040, D-054): a site keeps named commands, such as `npm run dev`. A proxy site can name one that its first request starts and that stops when idle.
  - Commands run with the login shell's environment, plus Node from nvm (the folder's `.nvmrc`, else nvm's default alias).
- **Vite over HTTPS** (D-056): a Laravel site's commands get the site's certificate.
- **PHP and Composer:** Devid's own. `composer` is a terminal command (D-053), and `composer.phar` comes from getcomposer.org with its published SHA-256.
- **Node.js:** the user's own (D-035). D-060 leaves Node and Bun runtimes to a later phase of their own.
- **Databases** (D-061, Phase 14): MySQL, PostgreSQL and MongoDB under the daemon, with `devid db user add` and the `.env` lines from `devid db env`.
- **Mail** (D-049): Mailpit catches what apps send, PHP's `mail()` included.
- **Long jobs** run in the daemon and report their progress, which the CLI and the TUI read: `devid update` (`UpdateProgress`) and catalog extensions (`PhpExtProgress`).

## What others do

- **Herd:** its site wizard creates Laravel apps, with or without a starter kit. Its WordPress guide is manual: create the database, download WordPress, extract it into a parked folder.
- **Local:** WordPress only.
- **The frameworks** publish their own create tools, kept up to date with each release:
  - Laravel's installer (`laravel new --react|--vue|--livewire`), on top of `composer create-project`;
  - `npm create vite`, `create-next-app`, `create nuxt`, `create astro`, `sv create`.
- **React** retired Create React App on 2025-02-14. Its docs point new apps to a framework (Next.js, React Router), or to a build tool such as Vite.

## Proposal (D-063)

### Templates

| Template | How Devid creates it | What Devid sets up |
|---|---|---|
| **PHP** (blank) | a folder with `index.php` | the site |
| **Laravel**: plain, or the React, Vue, Svelte or Livewire starter kit | `composer create-project laravel/laravel`, or the kit's package, such as `laravel/livewire-starter-kit`; no Laravel installer | `APP_URL`, the database, Mail, the migration, the kit's front end, and `npm run dev` as a command |
| **WordPress** | WP-CLI: `wp core download`, `wp config create`, `wp core install` | a MySQL account and database from Devid, `wp-config.php` with fresh salts, an admin account |
| **Symfony** | `composer create-project symfony/skeleton`, then `composer require webapp` for Symfony's full web app (`--kit skeleton` keeps the bare skeleton) | the site, served from `public/` |
| **CodeIgniter 4** | `composer create-project codeigniter4/appstarter` | `.env` with `CI_ENVIRONMENT = development` and `app.baseURL` |
| **Static** | a folder with `index.html` | a static site |
| **React, Vue, Svelte or vanilla (Vite)** | `npm create vite@latest <name> -- --template react-ts` (or `vue-ts`, `svelte-ts`, `vanilla-ts`) | a proxy site, and `npm run dev` on its port, started on demand |
| **Next.js** | `npx create-next-app@latest` | the same |
| **Nuxt** | `npm create nuxt@latest` | the same |
| **SvelteKit** | `npx sv create` | the same |
| **Astro** | `npm create astro@latest` | the same |
| **Node.js API** (Express) | no official tool exists, so Devid writes a small one: `package.json`, `server.js` that reads `PORT`, and `npm install express` | the same |

- **Official create tools only:** for every framework that has one, Devid runs it rather than a copy of its own, which would date with each release. The project then looks the way the framework's docs and tutorials show it.
- **No changes to the project's code** beyond the settings files the table names (`.env`, `wp-config.php`).

### How `devid new` makes a project

1. **Checks before anything is written:**
   - **The name:** lowercase letters, digits and `-`, so it works as a host name. No site may already use it.
   - **The folder:** it must not exist, or be empty.
   - **What the template needs:** a PHP version it supports; Node at the version its tool needs (Vite: 20.19+ or 22.12+); MySQL for WordPress, which Devid installs and starts if it is off; the network.
2. **Where:**
   - `--path` names the folder.
   - Otherwise the project goes into the first parked folder, so it is served as soon as it exists.
   - With no parked folder, Devid offers to create `~/Sites` and park it.
3. **The daemon runs the job,** like `devid update`:
   - one step at a time, each reported to the CLI, the TUI and the app;
   - what the tools print goes to `~/.devid/log/new-<name>.log`.
4. **The official tool runs** with fixed options, so it asks nothing: Composer and npm as the user, with Devid's PHP and nvm's Node.
5. **Devid's part:** the site (its preset, and `devid isolate` when `--php` is given), the database account and its lines, Mail's settings, the commands and on-demand start.
6. **The end:**
   - Devid prints the URL, and opens it with `--open`. For WordPress it also prints the admin login.
   - The TUI and the app show an "Open" button.
7. **A failure** stops the job and keeps what was made. Devid then says what is there (the folder, a database and its account) and how to remove it.

### Laravel

- **Database:**
  - SQLite by default (Laravel's own default), so no server runs.
  - `--db mysql` or `--db pgsql` makes an account and a database named after the project through Devid (D-061), and writes its `.env` lines.
- **Settings:** `APP_URL=https://<name>.test`. When Mail is on, `MAIL_MAILER=smtp`, `MAIL_HOST=127.0.0.1` and Mail's port.
- **Migration:** `php artisan migrate`.
- **Starter kits:** they need Node, for `npm install` and `npm run build`. `npm run dev` becomes the site's command, with Vite over HTTPS (D-056).
- **Plain Laravel** needs no Node.

### WordPress

- **WP-CLI:** it joins Devid like `composer.phar`. Devid downloads the phar from WP-CLI's GitHub release and checks it against the SHA-512 published there. `wp` becomes a terminal command (D-053).
- **Steps:**
  1. `devid db user add mysql <name>` (D-061). MySQL starts first if it is off.
  2. `wp core download`.
  3. `wp config create` with the account, `127.0.0.1` and MySQL's port. WP-CLI fetches fresh salts.
  4. `wp core install` with `--url=https://<name>.test`, an admin account `admin` and a generated password.
- **The admin password** is shown once, and Devid does not keep it. `wp user update admin --user_pass=…` resets it, and a lost-password email reaches Mail when Mail is on.
- **PHP:** WordPress 7.x recommends PHP 8.4 or 8.5, which Devid's default meets.

### Node.js

- **Node:** the user's own, through Devid's nvm lookup (D-035). With no Node, or an older one than the tool needs, Devid says which version to install.
- **The package manager:** npm by default, since it comes with Node. `--pm pnpm|yarn|bun` uses another one that is already installed; Devid installs none of them.
- **Each tool's fixed options** are found and checked in N1. Examples: `--template` for Vite, `--yes` for Next.js and Astro, and no Git or install prompts.
- **The site:**
  - the preset that matches the framework (`vite`, `next`, `nuxt` or `node`);
  - a free port from the framework's usual one;
  - `npm run dev` with that port, as the command that starts on demand.
- **Packages:** Devid runs only the official packages, by their exact names, at `@latest`.

### Interfaces

- **CLI:**
  - `devid new <template> <name> [--path <dir>] [--php <version>] [--db sqlite|mysql|pgsql] [--kit <kit>] [--pm npm|pnpm|yarn|bun] [--open]`;
  - the templates: `php`, `laravel`, `wordpress`, `symfony`, `codeigniter`, `static`, `react`, `vue`, `svelte`, `vite` (vanilla), `next`, `nuxt`, `sveltekit`, `astro` and `express`;
  - `--kit`: Laravel's `react`, `vue`, `svelte` or `livewire`; Symfony's `skeleton`;
  - `--db` applies to Laravel; WordPress always uses MySQL;
  - `devid new --list` lists the templates, and says why one cannot run here (no Node, Node too old, no MySQL on this macOS).
- **TUI:** `n` on the Sites page opens a wizard: template, name, the template's options, then the progress, and Open.
- **The app:** a New project page, built from a design board the owner approves first (D-026), as for Phase 14's Databases page.
- **The daemon:** `NewTemplates`, `NewProject { template, name, path?, options }` and `NewProjectProgress`.

### Limits shown, not hit

- **Node templates and Laravel's starter kits** need Node, at the version their tool asks for.
- **WordPress** needs MySQL, which needs macOS 14 or later (D-058).
- **The network:** every template but the blank PHP and static ones downloads from the official sources (Packagist, wordpress.org, GitHub, the npm registry).

## Milestones

| # | Milestone | Done when |
|---|---|---|
| N1 | **Spike:** each official tool without prompts, on macOS and Ubuntu | the fixed options, the time, the disk use and the Node versions of every template are in this doc. That includes the starter kits' package names, WP-CLI's release files, and whether `wp core download` checks its archive |
| N2 | **Daemon and CLI:** the templates, the job and its progress, the PHP templates, Laravel with the database and Mail, WordPress with WP-CLI and `wp` | on the M1, `devid new` makes a Laravel project with each database and a WordPress site. Each opens at `https://<name>.test`, next to DBngin |
| N3 | **Node.js templates and the TUI wizard** | every Node template opens through the proxy with hot reload, and the TUI makes a Laravel project and a Vite project |
| N4 | **The app's New project page** (macOS) | the owner approves a design board, then makes a project from the page on the M1 |
| N5 | **CI, docs and release** | a weekly CI job makes every template with the real tools on macOS and Ubuntu, so a changed option shows within a week. The guide gets a `/docs/new-project` page. 0.22.0 ships after an rc on the M1 and the Linux VMs |

## Where it is tested

- **GitHub's runners:**
  - unit tests;
  - CLI tests with fake `composer`, `wp` and `npm` from a local test setup, as Phase 14's tests fake the servers;
  - the weekly job with the real tools.
- **The owner's M1:** each template once, Devid's MySQL next to DBngin, and the projects in a scratch folder.
- **The Ubuntu, Arch and CachyOS VMs,** with the rc.

## Owner's answers (2026-11-25)

- **The request:** "buatkan fitur baru untuk create project PHP, Laravel, Wordpress, NodeJs, dll. berikan saran dulu sebelum kamu menambahkan dokumentasi baru".
- **The eight recommendations:** "saya ikut rekomendasi kamu".
  1. The templates in the table above.
  2. The first parked folder, else `~/Sites`, which Devid offers to create and park; `--path` for another folder.
  3. SQLite for Laravel by default, with MySQL and PostgreSQL through Devid's accounts; MySQL always for WordPress.
  4. The user's own Node (nvm); Node and Bun runtimes stay in a later phase (D-060).
  5. WP-CLI as the terminal command `wp`.
  6. The CLI and the TUI first; the app's page after a design board.
  7. Templates from a Git repository later, not in this phase.
  8. Phase 15, after Phase 14, since WordPress needs Phase 14's MySQL.
- **Node.js:** asked "untuk aplikasi nodeJS perlukah mengikuti template bawaan misal reactJS, dll?". Claude recommended running each framework's official create tool, plus Devid's small Express template, rather than Devid's own copies. The owner agreed: "Ya, tulis rencananya".

## Not in this phase

- **Templates from a Git repository** (`devid new --from <url>`). They run another party's scripts.
- **Devid's own Node and Bun** (D-060).
- **More frameworks:**
  - Statamic, Drupal, Craft CMS, Magento, Angular and React Router;
  - each is one more row in the template table later.
- **Deploying projects.**
