# Phase 15 Plan: Creating projects

> Scope: `devid new` creates a PHP, Laravel, WordPress, Symfony, CodeIgniter 4, static or Node.js project and serves it at `https://<name>.test` right away, through the CLI, the TUI and the app (D-063)
> Verified against: 2026-11-26 (Devid 0.21.0, main e6d9537; N1's runs on the owner's M1 and the Ubuntu VM: Laravel 13.44, its starter kits' `dev-main`, Symfony 8.1, CodeIgniter 4.7, create-vite 9.3, create-next-app 16.4, create-nuxt 4.1, sv 1.7, create-astro 5.3, WP-CLI 2.12.0, WordPress 7.1.5)
> Status: accepted 2026-11-25 (D-063): the owner took every recommendation ("saya ikut rekomendasi kamu") and the official create tools for Node.js ("Ya, tulis rencananya"). It starts after Phase 14 (0.21.0). Its answers are in "Owner's answers". **N1 done 2026-11-26:** every official tool ran without prompts on the M1 and the Ubuntu VM (see "N1 results"). **N2 done 2026-11-26:** `devid new` makes the PHP templates, Laravel with each database and its kits, Symfony, CodeIgniter 4 and WordPress, checked on the M1 (see "N2 results"). **N3 done 2026-11-26:** the nine Node.js templates, proxied with their dev server on demand, and the TUI's wizard (see "N3 results"). **N4 built 2026-11-26:** the app's New project sheet, from the board the owner approved; the owner's own try comes with the rc (see "N4 results"). **N5 in progress 2026-11-26:** a weekly workflow makes every template with the real tools on macOS and Ubuntu, and the guide's page is written; 0.22.0-rc.1 is next (see "N5 results").

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
| **Symfony** | `composer create-project symfony/skeleton`, then `composer require webapp` for Symfony's full web app (`--kit skeleton` keeps the bare skeleton) | the site, served from `public/`; the database, as for Laravel; Mail |
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
  - `--db` applies to Laravel and Symfony (the owner's answer, 2026-11-26); WordPress always uses MySQL;
  - `devid new --list` lists the templates, and says why one cannot run here (no Node, Node too old, no MySQL on this macOS).
- **TUI:** `n` on the Sites page opens a wizard: template, name, the template's options, then the progress, and Open.
- **The app:** a New project page, built from a design board the owner approves first (D-026), as for Phase 14's Databases page.
- **The daemon:** `NewTemplates`, `NewProject { template, name, path?, options }` and `NewProjectProgress`.

### Limits shown, not hit

- **Node templates and Laravel's starter kits** need Node, at the version their tool asks for.
- **WordPress** needs MySQL, which needs macOS 14 or later (D-058).
- **The network:** every template but the blank PHP and static ones downloads from the official sources (Packagist, wordpress.org, GitHub, the npm registry).

## N1 results (2026-11-26)

**Where it ran:**
- **The owner's M1** (8 GB, macOS 26): Devid 0.21.0's PHP 8.5.13 and Composer 2.11.2, Node 24.22.3 from nvm, npm 11.19.1.
- **The Ubuntu 24.04 VM** (x86_64, 2 cores, 2 GB):
  - PHP: Devid's 8.5.13 build from devid-binaries (`2026.11.25-22`);
  - Composer 2.11.3, and Node 24.22.3 from nodejs.org;
  - each checked against its published checksum and unpacked into a folder in the home directory, with nothing installed system-wide;
  - the folder was removed afterwards. Git is not installed there.
- **Every run** had no terminal (stdin was `/dev/null`) and its own empty Composer, npm and WP-CLI caches, so the times are those of a first run.
- **The script** is `spike/new/spike.sh`.

### What each template runs, and what it costs

| Template | The command, with the options that keep it from asking | M1 | Ubuntu VM | Disk | Node |
|---|---|---|---|---|---|
| Laravel | `composer create-project laravel/laravel <name> --prefer-dist --no-interaction` | 18 s | 26 s | 71 MB | — |
| Laravel, React kit | `composer create-project laravel/react-starter-kit <name> --stability=dev --prefer-dist --no-interaction` | 87 s | 200 s | 462 MB | 22.18+ or 24.11+ |
| Laravel, Vue kit | the same with `laravel/vue-starter-kit` | 94 s | 186 s | 479 MB | the same |
| Laravel, Svelte kit | the same with `laravel/svelte-starter-kit` | 73 s | 194 s | 456 MB | the same |
| Laravel, Livewire kit | the same with `laravel/livewire-starter-kit` | 69 s | 139 s | 377 MB | the same |
| Symfony (web app) | `composer create-project symfony/skeleton <name> --no-interaction`, then `composer config --json extra.symfony.docker false` and `composer require webapp --no-interaction` | 28 s | 43 s | 96 MB | — |
| Symfony (`--kit skeleton`) | `composer create-project symfony/skeleton <name> --no-interaction` | 10 s | 16 s | 12 MB | — |
| CodeIgniter 4 | `composer create-project codeigniter4/appstarter <name> --no-interaction` | 5 s | 9 s | 27 MB | — |
| React (Vite) | `npm create vite@latest <name> -- --template react-ts --no-interactive --no-immediate`, then `npm install` | 18 s | 36 s | 89 MB | 20.19+ or 22.12+ |
| Vue (Vite) | the same with `vue-ts` | 14 s | 41 s | 78 MB | the same |
| Svelte (Vite) | the same with `svelte-ts` | 31 s | 42 s | 72 MB | the same |
| Vanilla (Vite) | the same with `vanilla-ts` | 11 s | 25 s | 62 MB | the same |
| Next.js | `npx --yes create-next-app@latest <name> --yes --use-npm` | 61 s | 139 s | 470 MB | 20.9+ |
| Nuxt | `npm create nuxt@latest <name> -- --template minimal --packageManager npm --no-gitInit --modules=` | 89 s | 173 s | 171 MB | 22.22.3+ or 24.15+ |
| SvelteKit | `npx --yes sv@latest create <name> --template minimal --types ts --no-add-ons --install npm` | 23 s | 35 s | 71 MB | 22.17+ |
| Astro | `npm create astro@latest <name> -- --yes --skip-houston` | 30 s | 105 s | 175 MB | 22.23.4+ |
| Express API | `npm init -y`, `npm install express`, and Devid's `server.js` | 3 s | 9 s | 3 MB | 18+ |
| WordPress | `wp core download <zip URL>`, `wp config create`, `wp core install` | 18 s | 31 s, the download only | 114 MB | — |

- **Disk use** is the M1's. Ubuntu's was within 8 % of it.
- **The starter kits' times** include an `npm install` and `npm run build` that the spike ran after Composer. The kits had already done both (see "Laravel"), so Devid needs neither.
- **Node** is the framework's own `engines.node`, which can be stricter than its create tool: `create-nuxt` accepts 22.21, Nuxt 4.7 needs 22.22.3. The kits' minimum comes from Vite+ (`vite-plus` 1.8).
- **What they made:**
  - Laravel 13.44.1; Symfony 8.1.12; CodeIgniter 4.7.6;
  - from create-vite 9.3.4, Vite 8.3; from create-next-app 16.4.6, Next.js 16.4.6; from create-nuxt 4.1.0, Nuxt 4.7.0;
  - from sv 1.7.1, SvelteKit 3; from create-astro 5.3.2, Astro 7.4; Express 5.3;
  - WordPress 7.1.5, with WP-CLI 2.12.0.
- **Dev servers:** every Node template answered 200 on the port the spike gave it:
  - `npm run dev -- --port <port>`, Next.js's `-p <port>`, and the Express template's `PORT`;
  - 2 to 3 s after starting on the M1, 1 to 8 s on the VM.

### Answers

1. **No tool asked anything** with the options above and no terminal.
   - **Vite:** `--no-immediate` keeps it from installing and starting the dev server itself.
   - **Next.js:** `--yes` takes the user's saved preferences when there are any. With none saved it chose TypeScript, ESLint, Tailwind 4, the App Router, no `src/`, and an `AGENTS.md`. N2 passes each choice explicitly, so a user's saved preferences cannot change what Devid makes.
   - **Nuxt:** an empty `--modules=` skips its question about modules.
   - **Astro:** `--yes` takes the `basics` template.
2. **Git repositories:**
   - Next.js and Astro make one by default when Git is installed. On the Ubuntu VM, which has no Git, both went on without one.
   - The other tools, Laravel's installer included, make none.
3. **A tool's own install can fail and still exit 0.**
   - On the VM, create-astro's `npm install` failed ("Dependencies failed to install"). create-astro still printed "Project initialized!" and exited 0, with no `node_modules`.
   - A second `npm install` in that folder passed in 25 s, and its dev server answered.
   - The cause is unknown. The VM had memory to spare, and it did not happen again.
4. **Laravel:**
   - **`create-project` runs Laravel's own scripts:** `.env` from `.env.example`, `key:generate`, an SQLite file, and `migrate`. That suits `--db sqlite`.
   - **Laravel's installer** runs `create-project` with `--no-scripts` instead, then runs `post-root-package-install` and `key:generate` itself, and migrates once the database is chosen.
   - **The `.env` it writes** has `APP_URL=http://localhost:8000` and `MAIL_MAILER=log`.
   - **The starter kits' packages:**
     - `laravel/react-starter-kit`, `laravel/vue-starter-kit`, `laravel/svelte-starter-kit` and `laravel/livewire-starter-kit`.
     - Their tagged releases are v1.0.x from February 2025, for Laravel 12. The Svelte kit has no tagged release.
     - Laravel's installer takes each kit's `dev-main` with `--stability=dev`. That branch is the Laravel 13 kit (`laravel/framework ^13.17`), updated 2026-11-25.
     - The branches `dev-teams`, `dev-workos` and `dev-workos-teams` are the installer's `--teams` and `--workos`.
   - **Inside `create-project`,** every kit runs `php artisan install:features`. That step runs `npm install`, generates Wayfinder's files and builds the assets, so a kit needs Node before Composer starts.
   - **Vite+:** the kits build with Vite+ (`vite-plus` 1.8; `"dev": "vp dev"`). They keep `laravel-vite-plugin` 3, through which D-056 gives Vite its HTTPS certificate.
5. **Symfony:**
   - **Docker:** without a terminal, `composer require webapp` added Docker Compose files (`compose.yaml`, `compose.override.yaml`). With `composer config --json extra.symfony.docker false` before it, it added none (checked on the M1).
   - **The database:** `.env` points `DATABASE_URL` at PostgreSQL on 127.0.0.1:5432, with the account `app` and the password `!ChangeMe!`. On the owner's M1 that port is DBngin's.
   - **Mail:** `MAILER_DSN` is `null://null`.
6. **CodeIgniter 4** ships an `env` file, not `.env`. Inside it, `CI_ENVIRONMENT` and `app.baseURL` are commented out.
7. **WP-CLI:**
   - **The release:** its latest is 2.12.0, from 2025-05-07. Its files are `wp-cli-2.12.0.phar`, with `.md5`, `.sha256`, `.sha512`, `.asc` and `.gpg` beside it. The phar matched its SHA-512 on both machines. The `wp-cli.phar` in WP-CLI's builds repository is the same file.
   - **On PHP 8.5** the phar prints a deprecation notice (from `react/promise`, inside it) before every command's output. With `error_reporting = E_ALL & ~E_DEPRECATED` it prints none.
8. **WordPress:**
   - **`wp core download` checks the archive** against the MD5 that wordpress.org publishes ("md5 hash verified").
   - **Its default archive breaks WordPress 7.1:**
     - The default is the `.tar.gz`, which WP-CLI unpacks with PHP's `PharData`.
     - On the M1 that cut 46 file names in `wp-includes/php-ai-client` at 100 characters, and `wp core verify-checksums` failed.
     - On the VM, `PharData` ran out of PHP's 128 MB memory limit. The VM's PHP ran without Devid's `php.ini`, which sets 512 MB.
   - **The zip works:** wordpress.org's version API (`api.wordpress.org/core/version-check/1.7/`) offers a zip as `packages.full`. It downloaded, matched its MD5, and passed `verify-checksums` on both machines.
   - **`wp config create`** used a Devid MySQL account with `--dbhost=127.0.0.1:3307`, next to DBngin. It checked the connection and wrote fresh salts. `wp core install` then took about 1 s.
   - **`siteurl` depends on where the phar is:**
     - With the phar in a parent folder of the site, WordPress took the folder between them as part of the URL (`https://n1wp.test/site-zip`), even with `wp` run from the site's folder.
     - With the phar elsewhere, `siteurl` and `home` were `https://n1wp.test`.
   - **Clean-up:** the test account and its database were removed afterwards.
9. **Time and disk:**
   - On the M1, a project took 3 to 94 s, and 18 to 30 s for most templates. The VM was 1.5 to 3.5 times slower.
   - The starter kits and Next.js take 380 to 480 MB each, most of it `node_modules`.

### What this changes in the plan

D-063 stands. N1 settles these details for N2 and N3:
- **Node templates:**
  - Devid runs each tool without its own install step: Vite's `--no-immediate`, Next.js's `--skip-install`, and `--no-install` for Nuxt, SvelteKit and Astro.
  - Then Devid runs the package manager itself, so a failed install stops the job with the package manager's error.
- **No Git repository:** Next.js gets `--disable-git`, Astro `--no-git`, and Nuxt `--no-gitInit`, as no other template makes one.
- **Node minimums:** each template's minimum is its framework's `engines.node`, read from the npm registry when `devid new` runs, so it follows each release. The starter kits use Vite+'s.
- **Laravel:**
  - **SQLite:** `create-project` with Laravel's scripts.
  - **MySQL or PostgreSQL:** `--no-scripts`, then Laravel's `post-root-package-install` and `key:generate`, then Devid's `.env` lines, then `migrate`.
  - **Every database:** Devid then sets `APP_URL` and Mail's lines.
  - **The kits** come from `dev-main` with `--stability=dev`, as Laravel's installer takes them. Node is checked before Composer starts.
  - **N3** checks D-056's Vite over HTTPS with `vp dev`.
- **Symfony:**
  - `extra.symfony.docker` is set to `false` before `composer require webapp`.
  - Devid writes `DATABASE_URL` and `MAILER_DSN` to `.env.local`.
  - **The database:** Symfony gets `--db` as Laravel does: SQLite by default, `--db mysql` or `--db pgsql` through a Devid account. The owner chose this on 2026-11-26: "Ya, seperti Laravel".
- **CodeIgniter 4:** Devid copies `env` to `.env`, with `CI_ENVIRONMENT = development` and `app.baseURL = 'https://<name>.test/'`.
- **WordPress:**
  - `wp core download` gets the zip URL from wordpress.org's version API.
  - `wp core verify-checksums` runs after it.
  - `wp` runs with `error_reporting = E_ALL & ~E_DEPRECATED`. The phar stays under `~/.devid`, outside every parked folder.
  - After `wp core install`, Devid checks that `siteurl` and `home` are `https://<name>.test`.

## N2 results (2026-11-26)

### What shipped

- **`devid new <template> <name>`** in the CLI, for `php`, `laravel` (with `--kit react|vue|svelte|livewire`), `wordpress`, `symfony` (with `--kit skeleton`), `codeigniter` and `static`.
  - **Options:** `--db sqlite|mysql|pgsql` for Laravel and Symfony, `--php`, `--path` and `--open`.
  - **`devid new --list`** (or no template) lists the templates, with what this computer lacks for each: no PHP, or Node.js too old or missing for the starter kits.
  - **The Node.js templates** answer "comes in a later version" (N3).
  - **No parked folder and no `--path`:** in a terminal, the CLI offers to create `~/Sites` and park it.
- **The daemon** (`newproject.rs`, and `NewTemplates`, `NewProject` and `NewProjectProgress` in the protocol):
  - **Checks before anything is written:**
    - the name: a site name that starts with a letter, at most 32 characters, so `-` → `_` gives the database and account name;
    - no site with that name;
    - a new or empty folder whose parent exists; in a parked folder, the folder takes the project's name;
    - the template's kit and `--db`;
    - the PHP version installed, and `composer.phar` present;
    - one job at a time.
  - **The job:**
    - **Each step** is reported to `NewProjectProgress`, and the CLI prints it as it starts.
    - **The tools' output** goes to `~/.devid/log/new-<name>.log`. Passwords in arguments show as `--dbpass=…`.
    - **The tools run** as Devid's commands do (D-036): the login shell's environment and nvm's Node, with Devid's PHP and its ini for Composer, artisan and WP-CLI, and no stdin.
  - **At the end:**
    - **The site:** a project outside a parked folder is linked; `--php` pins the site.
    - **The kits** get `vite: npm run dev` as a saved command.
    - **The reply** has the URL, the folder, the database account with its password, WordPress's admin password (shown once, not kept), the commands and the log.
  - **A failure** stops at the failed step, marks it failed in the progress, and names what stays: the folder, and the database account.
- **Laravel:**
  - **`APP_URL`;**
  - **Mail's lines** while Mail is on;
  - **with `--db mysql|pgsql`:** a Devid account and database named after the project, the `.env` lines, and `artisan migrate`. The database's engine starts first if it is off.
  - **The kits** check Node.js (22.18+, 24.11+ or 26+) before Composer starts.
- **Symfony:**
  - the web app without Docker files;
  - `DATABASE_URL` in `.env.local`: SQLite, or a Devid account (the owner's answer, 2026-11-26);
  - `MAILER_DSN` while Mail is on.
- **CodeIgniter 4:** `.env` from its `env`, with `CI_ENVIRONMENT = development` and `app.baseURL`.
- **WordPress:**
  - **WP-CLI:** downloaded on first use from its newest GitHub release, checked against its SHA-512, and kept at `~/.devid/wp-cli/wp-cli.phar`.
  - **Its steps:** MySQL (started if off), the account, `wp core download` of the zip, `wp core verify-checksums`, `wp config create`, `wp core install` with a generated admin password, and a check of `siteurl` and `home`.
- **`wp`** is a terminal command (D-053): WP-CLI on the current folder's PHP, without PHP 8.5's deprecation notices. An install gets it with `devid setup`, which `self-update` runs.

### Different from the plan

- **Laravel with MySQL or PostgreSQL** keeps Laravel's own scripts, rather than `--no-scripts` as N1 proposed:
  - The starter kits' `npm install` and build run from those scripts (`install:features`).
  - So `create-project` runs as for SQLite, then Devid writes the database's lines, removes the SQLite file Laravel made, and migrates again on the account's database.

### Checked

- **Unit tests** (`newproject.rs`):
  - names, kits and databases per template;
  - Node versions for the kits;
  - `.env` editing: replace, uncomment, append, and CodeIgniter's spacing;
  - folder checks, steps, progress;
  - secrets kept out of the log, and Doctrine's URL escaping.
- **`crates/cli/tests/new.rs`:**
  - a fake PHP plays Composer and artisan, and every PHP template runs end to end, in a parked folder and elsewhere;
  - the checks fail before anything runs;
  - the kit path runs when Node.js is there;
  - a failed `create-project` keeps the folder and says so.
- **On the owner's M1:**
  - **The setup:** the N2 build ran as a second daemon with its own `DEVID_HOME` and ports (18088 and 18443, Mail on 12525). The owner's Devid 0.21.0 and its sites were left alone.
  - **The databases:** that daemon's MySQL took 3308, next to DBngin's MySQL on 3306 and Devid 0.21.0's on 3307. Its PostgreSQL took 5432, which was free because DBngin's PostgreSQL was off.
  - **Every template made a project,** and each site answered over HTTPS:

    | Project | Time | The page |
    |---|---|---|
    | `php` | 0 s | 200 |
    | `static` | 1 s | 200 |
    | `laravel` (SQLite) | 11 s | 200 |
    | `laravel --db mysql` (MySQL's first start included) | 28 s | 200; 9 tables in its database |
    | `laravel --db pgsql` (PostgreSQL's download included) | 20 s | 200 |
    | `laravel --kit react` | 55 s | 200, the built Inertia app |
    | `symfony` | 21 s | "Welcome to Symfony!" (with a 404, as Symfony's dev welcome page has) |
    | `symfony --db pgsql` | 12 s | "Welcome to Symfony!" (the same 404) |
    | `codeigniter` | 9 s | 200 |
    | `wordpress` (WP-CLI's download included) | 19 s | 200, and `/wp-login.php` |

  - **WordPress:**
    - `siteurl` and `home` were the site's URL;
    - `wp core verify-checksums` passed;
    - the log has no deprecation notices and no passwords;
    - `wp option get home`, `wp core version` (7.1.5) and `wp user list` worked through the `wp` command.
  - **A second `devid new wordpress`** with the same name was refused: "a site named n2wp exists already".
  - **Clean-up:** the second daemon was stopped, and its home and projects were removed.

### Not yet

- **N3:** the Node.js templates, and the TUI's wizard.
- **Not run:** `--open`, and the offer to park `~/Sites`. Both need a terminal and a browser.
- **The kits' Vite over HTTPS** (D-056) with `vp dev` is N3's check.

## N3 results (2026-11-26)

### What shipped

- **Nine Node.js templates:**
  - `react`, `vue`, `svelte` and `vite` (vanilla), through `npm create vite`;
  - `next`, `nuxt`, `sveltekit` (`sv create`) and `astro`;
  - `express`, whose `package.json` and `server.js` Devid writes, since Express has no create tool.
- **Each Node.js template's steps:**
  1. **Checking Node.js:** the framework's `engines.node`, read with `npm view` when the job runs, against the user's Node. A Node too old stops the job before anything is made.
  2. **The create tool,** with N1's options, without its own install step and without a Git repository.
  3. **`npm install`,** so a failed install stops the job with npm's own error.
- **The site:**
  - **Preset:** `vite` (Vite's templates and SvelteKit), `next`, `nuxt`, or `node` (Astro and Express).
  - **Dev port:** a free one from the framework's usual port, skipping ports a program listens on and ports a site, Mail, a database or Devid itself uses.
  - **The `dev` command:** `npm run dev` with that port, through `$PORT`, which Devid's commands get. It is marked on demand (D-054), so the site's first visit starts it, and it stops when idle.
  - **Astro's command** adds `--ignore-lock` (see "Found").
- **The TUI's wizard:** `n` on the Sites page.
  - **The form:** a template (←→), a name, and the template's kit and database.
  - **While it runs:** each step shows on the message line.
  - **When it ends:** what was made, with `o` to open the site and `c` to copy the details.
  - **A failure** brings the wizard back with the error.
- **The CLI:** `devid new --list` lists the Node.js templates. A project's details show its dev port. The CLI and the TUI share the progress loop (`new_with_progress`).
- **Not done:** `--pm pnpm|yarn|bun`. None of them is installed on the M1 to check it with, so npm is the only package manager for now.

### Checked

- **Unit tests:**
  - `engines.node` ranges as frameworks write them: `^20.19.0 || >=22.12.0`, Nuxt's three ranges, `>= 18`, `>=22.17`, `~18.3` and `>=18 <21`;
  - each Node.js template's steps;
  - the wizard's keys, fields and request, and its drawing.
- **`tests/new.rs`:** a second test with a fake `node`, `npm` and `npx` in `~/.devid/bin`, first on the tools' PATH:
  - the Vite and Next.js options;
  - `npm install` after the tool;
  - the proxy site's preset, port and on-demand `dev`, and two sites on different ports;
  - Express's own files;
  - a Node too old for Vite refused before anything is made.
- **On the owner's M1:** as for N2, a second daemon with its own home and ports.
  - **Ports:** before its first visit, each site moved to a 153xx port (`devid app set --upstream`), so no dev server ran on 3xxx, where the owner's sites and tunnel are, or on 4321. That also checked that `dev` follows the site's port through `$PORT`.
  - **The results:**

    | Template | Made in | First visit through the proxy | Hot reload |
    |---|---|---|---|
    | `react` | 16 s | 502 once, then 200 (see "Found") | Vite's HMR socket: 101 |
    | `vue` | 32 s | 502 once, then 200 | 101 |
    | `svelte` | 21 s | 200 in 1 s | 101 |
    | `vite` | 3 s | 200 in 1 s | 101 |
    | `next` | 22 s | 200 in 5 s | `/_next/hmr`: 101, with the `.test` Origin |
    | `nuxt` | 38 s | Nuxt's own "Starting Nuxt…" page (503, which reloads itself), then 200 | its Vite client answered; its socket not checked |
    | `sveltekit` | 7 s | 502 once, then 200 in 1.1 s | 101 |
    | `astro` | 13 s | 200 in 2 s | 101 |
    | `express` | 2 s | 200 | `node --watch`: a change to `server.js` was served 2 s later |

  - **The TUI,** driven in a pseudo-terminal, made a Laravel project (9 s) and a React one (4 s); both opened with 200.
  - **Vite over HTTPS (D-056) with Vite+:** with `APP_URL=https://<name>.test`, as on the owner's ports, a React kit's `vp dev` served HTTPS with the site's certificate. `public/hot` was `https://n3kit.test:5173`, and `@vite/client` answered 200 over HTTPS.
  - **Clean-up:** the second daemon was stopped, and its home and projects were removed.

### Found

- **Astro 7 moves `astro dev` to the background** when it detects an AI agent (`am-i-vibing`: `AI_AGENT`, `CLAUDECODE` and the like).
  - **What happened:** the test daemon started from Claude Code's shell, and its commands got that environment, so Astro detached its server. That server kept running after its command and the daemon had stopped.
  - **The owner's daemon** runs under launchd and has no such variables.
  - **The fix:** `--ignore-lock`, Astro's flag for a foreground server that `astro dev stop` does not track, keeps it in the foreground in any environment. It was checked in this one.
- **The first visit after creation** gave 502 once for three of the Vite templates in the run that made all nine one after another.
  - A fresh React project made alone answered 200 at its first visit (0.9 s), and the three answered 200 when their commands were stopped and woken again.
  - The cause is not known; a reload shows the page.
  - **Found in N5:** the test caused it. It moved each site to a 153xx port right before the first visit, and nginx's old worker still sent that visit to the old port, since nginx reloads by SIGHUP (see "N5 results"). `devid new` moves no port.
- **Nuxt, SvelteKit and Astro listen on `[::1]` only.** Devid's proxy reaches them, since nginx's retry tries `127.0.0.1` and `[::1]`.
- **Next.js 16's HMR socket is `/_next/hmr`.** The proxy's rewritten Origin passes Next's cross-origin check.

## N4 results (2026-11-26)

### The board

- **Where:** the Devid Redesign canvas, page "Fase 15: new project".
- **Its eight boards:**
  - the + menu on Sites;
  - the form, for Laravel and for Next.js;
  - the job running;
  - the job ready, for Laravel with MySQL and a starter kit, and for WordPress;
  - WordPress during MySQL's first download;
  - a failure.
- **The owner's questions** (2026-11-26):
  - **How WordPress makes its database:** the answer added three things to the board, all in this build:
    - MySQL's first download, shown under "Making the database account";
    - a taken account name refused before Create;
    - "Show in Databases" in place of "Copy URL" for WordPress.
  - **WordPress Multisite and multi-tenant Laravel:** left for later; see "Not in this phase".
- **Approved** on 2026-11-26: "Board N4 sudah oke, lanjut bangun".

### What shipped

- **Sites' +** is a menu: New project… (⌘N, also from other pages) and Link a folder….
- **The New project sheet:**
  - **The template list** (PHP; JavaScript · Node.js) comes from `new_templates`. A template this Mac lacks something for has a dot and the daemon's reason.
  - **Name:**
    - a hint shows the URL and the folder;
    - the checks: the name rule, the sites, and the accounts of a MySQL or PostgreSQL that runs (`db_user_list`). An engine that is off is checked when the job starts, and the sheet says so.
  - **Folder:**
    - the parked folders;
    - with none, `~/Sites`, which the page makes (`sites_folder`) and parks before the job;
    - or another folder from the picker, which links the project.
  - **The template's options:**
    - PHP, for the PHP templates;
    - the starter kit, whose buttons are off, with the reason, when Node is too old or missing;
    - Symfony's kind;
    - the database.
  - **One line each on what Devid sets up,** from the daemon's state:
    - the database server: running, off and started first, or not installed and downloaded first;
    - the account;
    - Mail;
    - the Node the job uses (`new_templates` now reports it);
    - a Node template's dev port.
  - **A note** blocks Create when this Mac cannot make the template, with the daemon's reason.
- **The job:**
  - **The steps:** `new_project` runs, and the page reads `new_project_progress` every 0.5 s. It shows each step with the time it took, and the last lines of the job's log (`log_read`).
  - **A first download:** during "Making the database account", the engine's download from `status` shows as a bar.
  - **Hide** leaves the job running. A toast says how it ended, and a failure while hidden shows again from the + menu.
- **Ready:**
  - the URL;
  - the folder, with Show in Finder and Open in Terminal;
  - what serves it, and a Node project's dev port;
  - the kits' `vite` command, with Start;
  - **the database account:**
    - Copy password;
    - Copy .env for Laravel;
    - Show in Databases for the others;
  - **WordPress's admin login,** marked "Shown once":
    - Copy, through the new `copy_text`;
    - Open, which opens `/wp-admin/` through `open` with a page path, checked to be a path on the site only;
  - Done and Open in browser.
- **Failed:**
  - the steps, with the failed one marked;
  - the daemon's message, which names what stays;
  - Show log and Show in Finder.

### Different from the board

- **The name field starts empty,** with a placeholder, rather than with a suggested name.
- **Symfony's "Copy URL"** became "Show in Databases": the page gets an account's password from Devid, not its Doctrine URL.
- **"What stays" on a failure** is the daemon's own message, with Show in Finder, rather than a separate card.
- **The new project** shows in the Sites list after the refresh, without a "New" chip.

### Checked

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` (the app included) and `cargo test` pass.
  - **New unit test:** `site_page`'s paths.
  - **`tests/new.rs`:** asserts `new_templates`' `node`.
- `node --check` passes on `app.js`.
- **In a browser,** with the real `app.js` and `app.css` and a fake `invoke` that answers like the N3 daemon. Its status and sites came from the owner's daemon, read only; the owner has no parked folder.
  - **The entry points:** the + menu and ⌘N.
  - **WordPress with MySQL not installed:**
    - `sites_folder`, `park`, then `new_project`;
    - the download bar under the account step;
    - Ready, with the admin password copied through `copy_text` and `/wp-admin/` opened.
  - **Laravel with MySQL:**
    - `shop`, a taken account name, refused before Create;
    - the React kit's tool line and steps;
    - Hide, then the toast when the job ended.
  - **A failed `create-project`:** the failed step, the message, Show in Finder and Close.
  - **Next.js:** Ready, with its dev port.
  - **No Node:**
    - the Node.js templates marked;
    - the kits off;
    - Create blocked, with the reason.
  - **The rest:** the list's arrow keys, Escape, and the light and dark themes.

### Not yet

- **The real app against a real daemon:** the owner's own try on the M1 comes with the rc in N5, as Phase 14's D4 did.
- **N5:** the weekly CI job, the guide's `/docs/new-project` page, and 0.22.0.

## N5 results (2026-11-26)

### The weekly check

- **`scripts/new-e2e.sh`** makes 24 projects with the real tools, one after another:
  - every template;
  - Laravel with each database and each starter kit;
  - Symfony's web app with each database, and its skeleton.
- **How it runs:**
  - a daemon in a throwaway `DEVID_HOME` under `/tmp`, on spare ports (18088, 18443 and 15353);
  - the binary, the `php`, `composer` and `wp` shims, and `composer.phar` (sha256-checked), as `devid setup` installs them, so the projects' own scripts run Devid's PHP;
  - a parked folder for the projects;
  - the daemon without `CI` and `GITHUB_ACTIONS`, since some tools skip their prompts in CI, which could hide a new prompt.
- **Each project must:**
  - finish `devid new`;
  - answer over HTTPS with its own page:
    - Laravel's, CodeIgniter's and WordPress's home page, and the blank and static pages;
    - Symfony's welcome page, with the 404 it has in development;
    - each Node.js project's page from its dev server, through the proxy, which the first visit starts;
  - pass its own checks:
    - the starter kits' built assets;
    - `DB_CONNECTION` in Laravel's `.env`;
    - Symfony's database, through `bin/console dbal:run-sql`;
    - WordPress's `siteurl`, and its login page.
- **The dev servers** stop after their check, so one runs at a time.
- **The report:** the script runs every project even after a failure. It prints a table of each project's time, first visit and result, then the logs of the failed ones, and exits 1 if any failed.
- **`SPARE_PORTS=1`,** for the owner's M1, keeps the test away from DBngin's ports and the owner's on-demand sites:
  - MySQL on 13306 and PostgreSQL on 15432;
  - each dev server on its own port from 15300, set a second before its first visit.
- **`.github/workflows/new-templates.yml`:**
  - **When:** Mondays at 03:00 UTC (10:00 WIB), two hours after devid-binaries' weekly check; by hand; and on a pull request that changes `newproject.rs`, the script or the workflow.
  - **Where:** macOS 15 on Apple silicon and Ubuntu 24.04, each with a debug build.
  - **Node:** the newest LTS from `actions/setup-node`, made nvm's default, since Devid's commands put nvm's Node first (D-036). A runner's login files may put an older Node first on `PATH`.
  - **A failed scheduled run:** GitHub emails the workflow file's last editor. The run's summary has the table, and the logs are an artifact.

### Checked

- **On the owner's M1,** with `SPARE_PORTS=1`, next to the owner's Devid 0.21.0 and DBngin, and with Composer's and npm's caches warm from N1 to N3: all 24 projects were made and answered.

  | Project | Made in | First visit |
  |---|---|---|
  | `php`, `static` | 0 s | 200 |
  | `laravel` | 9 s | 200 |
  | `codeigniter` | 2 s | 200 |
  | `symfony` | 14 s | 404, Symfony's welcome page |
  | `symfony --kit skeleton` | 3 s | 404, the same page |
  | `wordpress` (MySQL's download and first start included) | 28 s | 200; `siteurl` and the login page checked |
  | `laravel --db mysql` | 8 s | 200 |
  | `symfony --db mysql` | 11 s | 404; `dbal:run-sql` reached MySQL |
  | `laravel --db pgsql` (PostgreSQL's download included) | 12 s | 200 |
  | `symfony --db pgsql` | 12 s | 404; `dbal:run-sql` reached PostgreSQL |
  | `laravel --kit react`, `vue`, `svelte`, `livewire` | 32, 22, 24 and 15 s | 200, with the built assets |
  | `react`, `vue`, `svelte`, `vite`, `sveltekit` | 2 to 5 s | 200 within 2 s |
  | `next` | 13 s | 200 in 3 s |
  | `nuxt` | 22 s | Nuxt's own Starting page (503), then 200 in 3 s |
  | `astro` | 11 s | 200 in 1 s |
  | `express` | 1 s | 200 |

- **N3's first-visit 502, explained.** The first run gave React, Vue and SvelteKit a 502 once, as in N3.
  - **The cause:** the test moved each Node site's port right before its first visit. nginx reloads by SIGHUP, and its old worker still sent that visit to the old port: the error log shows `[::1]:5173` after the move to 15300. `smoke.sh` notes the same race.
  - N3's test moved the ports the same way.
  - With a second's pause after the move, all three answered 200 at their first visit.
  - `devid new` itself moves no port, and the weekly job on GitHub's runners moves none.
- **Clean-up:** the script stops the daemon, which stops the databases and the dev servers, and removes its folder. Nothing of it was left running.
- **The first run on GitHub's runners found a bug.**
  - **What failed:** the 15 PHP projects passed on macOS 15 and Ubuntu 24.04, but none of the 9 Node.js projects woke at its first visit. Each answered "Nothing answers on localhost:<port>" for 240 s, and no dev server was started.
  - **The cause:** the daemon listens for on-demand visits (`run/wake.sock`) only once some site is on demand. It checks that when it starts and after `app set`, `cmd on-demand` and `cmd start`, but not after `devid new`. On a computer with no on-demand site yet, the first Node.js project from `devid new` could not wake until something else turned the listener on, or the daemon restarted.
  - **Why N3 and the M1 run passed:** both moved each site's port with `devid app set` before the first visit, which turned the listener on. The owner's daemon already listens, for its on-demand sites.
  - **The fix:** `new_project` checks the on-demand sites once the project is made. `crates/cli/tests/new.rs` now checks that `run/wake.sock` appears with the first Node.js project. The test fails without the fix.
  - **Also seen:** a few PHP sites answered nothing (`000`) for a second right after `devid new`, then their page, as nginx reloaded. The check waits that out.

### The rc

- **Version 0.22.0-rc.1.** The release notes name `devid new`.
- **What is left:**
  1. The owner merges this and creates the pre-release v0.22.0-rc.1.
  2. The rc on the M1, once the owner agrees to update Devid there, with the owner's own try of the app's New project sheet (N4's done-when).
  3. The rc on the Ubuntu, Arch and CachyOS VMs: the PHP templates, WordPress, a starter kit and the Node.js templates.
  4. A last pull request, "Version 0.22.0": the README, PROJECT_CONTEXT, and D-063's status. Then the owner tags v0.22.0.
- **The guide:** devid-site#25 (a draft) adds `/docs/new-project` in English and Indonesian, and mentions `devid new` and `wp` on the related pages and in the roadmap. It merges when 0.22.0 ships.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| N1 | **Spike:** each official tool without prompts, on macOS and Ubuntu | the fixed options, the time, the disk use and the Node versions of every template are in this doc. That includes the starter kits' package names, WP-CLI's release files, and whether `wp core download` checks its archive (done 2026-11-26, see "N1 results") |
| N2 | **Daemon and CLI:** the templates, the job and its progress, the PHP templates, Laravel with the database and Mail, WordPress with WP-CLI and `wp` | on the M1, `devid new` makes a Laravel project with each database and a WordPress site. Each opens at `https://<name>.test`, next to DBngin (done 2026-11-26, see "N2 results") |
| N3 | **Node.js templates and the TUI wizard** | every Node template opens through the proxy with hot reload, and the TUI makes a Laravel project and a Vite project (done 2026-11-26, see "N3 results") |
| N4 | **The app's New project page** (macOS) | the owner approves a design board, then makes a project from the page on the M1 (board approved and page built 2026-11-26, see "N4 results"; the owner's own try comes with the rc) |
| N5 | **CI, docs and release** | a weekly CI job makes every template with the real tools on macOS and Ubuntu, so a changed option shows within a week. The guide gets a `/docs/new-project` page. 0.22.0 ships after an rc on the M1 and the Linux VMs (the weekly job and the guide's page done 2026-11-26, see "N5 results"; the rc next) |

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

## Owner's answers (2026-11-26)

- **Symfony's database,** asked after N1: should Symfony get `--db` like Laravel? The answer was "Ya, seperti Laravel".
  - The default is SQLite.
  - `--db mysql` or `--db pgsql` makes a Devid account and its database (D-061).
  - Devid writes `DATABASE_URL` to `.env.local`.

## Not in this phase

- **Templates from a Git repository** (`devid new --from <url>`). They run another party's scripts.
- **Devid's own Node and Bun** (D-060).
- **More frameworks:**
  - Statamic, Drupal, Craft CMS, Magento, Angular and React Router;
  - each is one more row in the template table later.
- **Deploying projects.**
- **WordPress Multisite** (`devid new wordpress --multisite subdir|subdomain`). Later, as the owner answered on 2026-11-26 ("ya nanti kamu catat"), since Devid cannot serve a network yet:
  - **Subdirectory networks** (`name.test/shop2`) need WordPress's multisite rewrite rules. Apache reads WordPress's own `.htaccess`, because sites get `AllowOverride All`. nginx and Caddy have no such rules, so a sub-site's `wp-admin` and files break there.
  - **Subdomain networks** (`shop2.name.test`) need `*.name.test` routed to the site, and certificates for those names. nginx matches each site's exact host, and Devid makes one certificate per site host.
  - **Once Devid serves both,** `devid new` runs `wp core multisite-install` instead of `wp core install`. Until then, `wp core multisite-convert` turns a site into a network by hand; a subdirectory network then works on Apache.
- **A multi-tenant option for Laravel.** It is not a template option, as the owner answered on 2026-11-26 ("Ya, catat"):
  - **Why:** tenancy comes from a package (`stancl/tenancy`, `spatie/laravel-multitenancy`) and from design choices inside the app, which no official create tool makes.
  - **What Devid can add later** is the serving that such apps need:
    - **Subdomains of a site** (`acme.shop.test`): `*.name.test` routed to the site, with certificates for those names. WordPress's subdomain networks need the same, and so does Laravel's `Route::domain()`.
    - **An account that may create databases,** for a database per tenant. A Devid account has rights on its own database only. Until then, MySQL's `root` and PostgreSQL's `postgres`, which need no password from this computer (D-061), can create them.
  - **What works today:** tenants told apart by path, and tenants sharing one database.
