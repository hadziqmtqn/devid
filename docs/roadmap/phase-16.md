# Phase 16 Plan: Subdomains of a site

> Scope: every site also answers on its subdomains, `https://<sub>.<name>.test`, with a certificate for them; accounts that may create databases; WordPress Multisite in `devid new` (D-065)
> Verified against: 2026-11-28 (Devid 0.22.0, main c11bebd; `dns.rs`, `ca.rs`, `nginx.rs`, `caddy.rs`, `apache.rs`, `db.rs`)
> Status: accepted 2026-11-28 (D-065): the owner chose this phase over a Linux desktop app and Windows ("Subdomain dulu") and took every recommendation ("Ya, ikuti semua"). Its answers are in "Owner's answers". **S1 done 2026-11-28** on the M1 and the Ubuntu VM (see "S1 results"). **S2 built 2026-11-28:** subdomains and their certificates on nginx, Caddy and Apache, checked on the M1 (see "S2 results").

On 2026-11-26, Phase 15 left two things for later: WordPress Multisite (the owner said "ya nanti kamu catat") and a multi-tenant option for Laravel ("Ya, catat"). Both need the same serving, which Devid lacks: subdomains of a site and their certificates. Multi-tenant apps with one database per tenant also need an account that may create databases (`phase-15.md`, "Not in this phase"). On 2026-11-28 the owner chose this work as Phase 16. They asked for suggestions before any documentation, and this plan records the ones they accepted.

## What exists

- **DNS:** Devid's responder answers every name under the TLD, at any depth, with 127.0.0.1 (`dns.rs`; its test resolves `a.b.test`). On macOS `/etc/resolver/test` sends `.test` to it, and on Linux a systemd-resolved drop-in does. **Subdomains need no DNS change.**
- **Web servers** match each site's exact host:
  - nginx: `server_name <name>.test`. Unknown hosts get a 404 on HTTP, and the HTTPS handshake is refused (`ssl_reject_handshake`), so no wrong certificate is ever served.
  - Caddy: one `https://<name>.test:<port>` and one `http://` block per site.
  - Apache: `ServerName <name>.test`. The first HTTPS vhost also answers unknown hosts, a known shortcut (`apache.rs`).
- **Certificates** (D-008): one leaf per site host, with a single SAN, signed by Devid's CA. The CA is name-constrained to the TLD, so it may sign `*.<name>.test` as it is. Leaves last 397 days and renew 30 days before they expire.
- **Site names** are DNS labels: lowercase letters, digits and `-`, with no dots (`sites::valid_name`). `acme.shop.test` can therefore never be another site's host.
- **Proxy sites** (D-035): nginx sends `X-Forwarded-Host $host`, and Apache sends it as well. Unless `pass_host` is on, `Host` becomes `localhost:<port>`, and the site's own `Origin` is rewritten for the dev server, matched on the exact host (`own_origin_map`).
- **Laravel and Vite** (D-056): laravel-vite-plugin allows CORS from `*.test` origins by default (`phase-8.md`, `phase-11.md`). A Laravel site's commands get the site's certificate for Vite.
- **Database accounts** (D-061): MySQL accounts get `GRANT ALL ON <db>.*`, a PostgreSQL account owns its database, and a MongoDB account reads and writes its own database. The main accounts (`root`, `postgres`) can create databases, without a password from this computer.
- **WordPress** in `devid new` (D-063): WP-CLI's `wp core install`, a Devid MySQL account and database.

## What others do

- **Valet** routes unknown subdomains of a site to that site, and its `valet secure` certificate names `<name>.test` and `*.<name>.test`.
- **Herd** follows Valet here.
- **Tenancy packages** such as `stancl/tenancy` create a database per tenant with the app's own database account. Its database names are a prefix plus the tenant's ID (`tenancy.database.prefix`, `tenant` by default).

## Proposal (D-065)

### Subdomains

- **On for every site.** No setting and no command. Nothing runs for it, so it costs no memory at idle. Since site names have no dots, it cannot clash with another site.
- **One level:** `acme.shop.test` reaches `shop`. A certificate's wildcard covers one level only, so deeper names (`a.b.shop.test`) are refused the way unknown hosts are now, and never get a wrong certificate.
  - **nginx** matches `*.shop.test` at any depth, so it needs a regular expression for one level, or another way that S1 settles.
  - **Caddy's** `*` matches one label.
  - **Apache's** `ServerAlias *.shop.test` may match deeper names too. S1 checks it, and what the handshake does then.
- **The app sees the name the visitor used:** PHP gets it in `HTTP_HOST` and `SERVER_NAME`, as Laravel's `Route::domain()` and WordPress's subdomain networks need.
- **Proxy sites** get their subdomains too:
  - `X-Forwarded-Host` already carries the subdomain while `Host` is rewritten;
  - the rewrite of the site's own `Origin` grows to its subdomains, so Vite and Next.js accept those pages;
  - with `pass_host` on, the dev server sees the subdomain in `Host` and must allow it itself, as it must allow `<name>.test` now.
- **Local ports and Share** (D-037) stay on the site's own host. Subdomains through a tunnel are not in this phase.

### Certificates

- **One certificate per site** names `<name>.test` and `*.<name>.test`.
- **Existing certificates** are reissued once, on the first start of the new version, by the daemon's usual check. A marker beside each leaf (like its `.expires` file) says it has the wildcard.
- **Same CA:** no sudo, no new trust step, and nothing changes in the system or browser trust stores.
- **Vite over HTTPS** (D-056) uses the same certificate, so a page at `acme.shop.test` can load Vite's assets over HTTPS.

### Accounts that may create databases

- **New option:** `devid db user add <engine> <name> --create-databases`; the TUI and the app get a checkbox. An existing account can get it or lose it too, with a command whose name S3 settles.
- **MySQL:** the account gets every right on the databases whose names start with `<name>_` (`GRANT ALL ON \`<name>\_%\`.*`), not a global `CREATE`. It can make and drop its tenants' databases, and still cannot touch anyone else's. The app sets its tenancy prefix to `<name>_`; the guide shows how.
- **PostgreSQL:** `CREATEDB`. The account owns the databases it makes, and gets no rights on other databases from it.
- **MongoDB:** not in this phase. The tenancy packages that make a database per tenant target SQL databases.
- **Accounts without the option** are unchanged.

### WordPress Multisite

- **New option:** `devid new wordpress <name> --multisite subdomain|subdir`, also in the TUI's wizard and the app's sheet. Without it nothing changes.
- **WP-CLI** runs `wp core multisite-install`, with `--subdomains` for a subdomain network, instead of `wp core install`.
- **Subdomain networks** (`shop2.blog.test`) work through the subdomains above.
- **Subdirectory networks** (`blog.test/shop2`) need WordPress's multisite rewrite rules:
  - Apache reads WordPress's own `.htaccess` (`AllowOverride All`) and needs nothing;
  - nginx and Caddy get the rules from WordPress's documentation, only for a WordPress site whose `wp-config.php` turns `MULTISITE` on without `SUBDOMAIN_INSTALL`. Devid only reads that file, as detection does now.
- **A site turned into a network by hand** (`wp core multisite-convert`) is served the same way.

### Clients and the guide

- **The CLI, the TUI and the app** say once, in a site's details, that it also answers on `*.<name>.test`, with the explanation in an (i) tooltip (D-055). The change is this one line and the two options above, so no design board is needed.
- **Laravel** gets no tenancy option in `devid new`, as the owner answered on 2026-11-26.
- **The guide** gets `/docs/subdomains`, in English and Indonesian in the same PR:
  - subdomains of a site, their certificate, and the one-level limit;
  - Laravel: `Route::domain()`, `SESSION_DOMAIN=.<name>.test` to share the login across subdomains, and a tenancy package's database prefix with a `--create-databases` account;
  - WordPress Multisite, both kinds.

## S1 results (2026-11-28)

Run on the owner's M1 (macOS 26.6.1, Devid 0.22.0) beside the owner's own Devid, which was not touched:
- **Web servers:** Devid's nginx 1.30.8, Caddy 2.11.9 (from the runtime release, checked against its SHA-256) and the system's Apache 2.4.67, each on spare ports: 18088/18443, 18089/18444 and 18090/18445.
- **php-fpm:** Devid's PHP 8.5 on a socket of its own.
- **Dev servers:** Vite 8.4.0 (React) and Next.js 16.4.6 on ports 15300 and 15301.
- **Certificates:** `spike/subdomains/cert.sh` signed leaves for `<name>.test` and `*.<name>.test`, shaped like `ca.rs`'s, with the owner's Devid CA. The CA key was read in place and never copied, and the leaves and every spike file were removed afterwards.
- **Names:** the owner's Devid DNS already resolved every name used.

### Certificates in the browsers

| Browser (macOS) | `acme.shop.test` (one level) | `a.b.shopw.test` (two levels, wildcard leaf) |
|---|---|---|
| Safari's stack (`nscurl`, NSURLSession) | loads | refused: "The certificate for this server is invalid" |
| Chromium 156 (the app's built-in browser) | loads | refused: `ERR_CERT_COMMON_NAME_INVALID` |
| Firefox 160.0.1, a fresh profile | loads | the error page; the headless screenshot never finishes |
| curl with the CA (LibreSSL) | loads | refused (exit 60) |

- **The name constraint holds:** the CA permits `test`, and every client accepted `*.shop.test` under it.
- **No browser treats a wildcard under a two-label name differently.**
- **Brave headless** hung on macOS and was dropped; Chromium stands in for it, as Brave did for Chrome in Phase 12.
- **Linux:** see "On Ubuntu" below.

### On Ubuntu

The owner turned the Ubuntu 24.04.5 VM on, and allowed Devid 0.22.0 to be installed there for the check and removed afterwards. It answered at 192.168.1.7. 192.168.1.5, its address before, now offered an unknown host key, so nothing was sent there.

- **Setup:** `devid setup` put the CA in the system store, `~/.pki/nssdb` and the one Firefox profile, and `devid doctor` said "all good".
- **The leaves:** signed with openssl from that Devid's CA and served by its nginx on 18443, as on the M1.

| Client | `acme.shop.test` | `a.b.shop.test` (regex site) | `a.b.shopw.test` (wildcard site) |
|---|---|---|---|
| curl, the system store | loads | handshake refused (exit 35) | refused (exit 60) |
| Firefox 160.0.1 (`/usr/bin/firefox`), a fresh profile with the CA added the way setup adds it | loads (screenshot) | | the error page; the screenshot never finishes |
| Brave 158 headless | no certificate error logged | | `net_error -200` (`ERR_CERT_COMMON_NAME_INVALID`) |

- **Brave headless** never finished a page on the VM, not even `https://example.com/` or the exact `shop.test`, in either headless mode, so it shows only which certificate errors it logged. It logged none for `acme.shop.test`, and `-200` for the two-level name.
- **So S2 adds a subdomain** to `scripts/linux-e2e.sh`, which already opens a site in headless Chrome on GitHub's Ubuntu runners. That checks Chrome on Linux on every pull request.
- **Cleanup:** `devid uninstall --purge` removed the CA from the system store and from NSS, the PATH lines, the resolver drop-in and `~/.devid`. `shop.test` no longer resolves, and the spike's files are gone.

### One level on each web server

| Host | nginx `~^[a-z0-9-]+\.shop\.test$` | nginx `*.shopw.test` | Caddy `*.shop.test` | Apache `ServerAlias *.shop.test` |
|---|---|---|---|---|
| `shop.test` | the site | the site | the site | the site |
| `acme.shop.test` | the site | the site | the site | the site |
| `Acme.Shop.Test` | the site (`$host` is lowercased) | the site | the site (`{host}` keeps the case) | the site |
| `a.b.shop.test`, HTTP | "no site" (404) | **the site** | "no site" (404) | **the site** |
| `a.b.shop.test`, HTTPS | handshake refused | **the wrong certificate** | handshake refused | **the wrong certificate** |

- **nginx:** a wildcard `server_name` matches every depth, so a site gets its exact name plus a regular expression for one level: `server_name shop.test ~^[a-z0-9-]+\.shop\.test$;`. nginx checks exact names first, then wildcards, then expressions in order, so a site's own name never reaches the expressions, and SNI picks the server the same way. Deeper names fall to the default servers, so the handshake is refused as for unknown hosts.
- **Caddy:** `https://shop.test:<port>, https://*.shop.test:<port>` in one block. Caddy's `*` is one label, so deeper names find no site and no certificate.
- **Apache:** `ServerAlias *.shop.test` matches every depth. On HTTP a guard in the vhost answers deeper names with 421 (`RewriteCond %{HTTP_HOST} ^[^.]+\.[^.]+\.shop\.test(:[0-9]+)?$ [NC]`, then `RewriteRule ^ - [R=421,L]`). On HTTPS the handshake still serves the site's certificate, which does not cover the name, so the browser warns. This is Apache's existing shortcut for unknown hosts (`apache.rs`) and stays as it is.

### Proxy sites: Vite and Next.js from a subdomain

Each site sat behind a copy of Devid's proxy block (`Host` rewritten, `Origin` mapped, `X-Forwarded-Host $host`), with `pass_host` off.

| Dev server | Today's `Origin` map (exact host) | Widened to subdomains |
|---|---|---|
| Vite 8.4.0 | page loads, `[vite] connected`, an edit hot-updated without a reload | the same |
| Next.js 16.4.6 | page loads, but `/_next/hmr` is refused: "Blocked cross-origin request to Next.js dev resource /_next/hmr from "acme.next.test"" | `[HMR] connected`, and an edit to `page.tsx` refreshed through Fast Refresh |

- **Vite** does not check the WebSocket's `Origin`, so it needs nothing.
- **Next.js** does, so the map needs the subdomains. One regular-expression entry per site does it, after the exact ones: `"~^https?://[a-z0-9-]+\.next\.test(:[0-9]+)?$" "http://localhost:15301";`.
- **Laravel's Vite:** laravel-vite-plugin 3.2.1 (in the owner's `test-laravel`) allows CORS from `/^https?:\/\/.*\.test(:\d+)?$/`, which takes subdomains. Vite's host stays `<name>.test`, which the certificate still names. A full Laravel page from a subdomain is checked in S2.

### WordPress Multisite, subdirectory network

A tree shaped like WordPress's (`index.php`, `wp-login.php`, `wp-admin/`, `wp-includes/`, `wp-content/uploads/sites/2/`) answered the same on all three servers:

| Path | Served by |
|---|---|
| `/`, `/hello-world/`, `/shop2/`, `/shop2/hello/` | `index.php`, with the original `REQUEST_URI` |
| `/shop2/wp-admin` | 301 to `/shop2/wp-admin/` |
| `/shop2/wp-admin/`, `/shop2/wp-admin/admin-ajax.php` | the files in `wp-admin/` |
| `/shop2/wp-login.php` | `wp-login.php` |
| `/shop2/wp-includes/js/x.js` | the file |
| `/wp-content/uploads/sites/2/a.txt` | the file |

- **Apache** used WordPress's own `.htaccess` for subdirectory networks, unchanged.
- **nginx** used WordPress's rules at the server level, with a relative redirect so the port stays:

  ```nginx
  if (!-e $request_filename) {
      rewrite /wp-admin$ $uri/ permanent;
      rewrite ^(/[^/]+)?(/wp-.*) $2 last;
      rewrite ^(/[^/]+)?(/.*\.php) $2 last;
  }
  ```

- **Caddy** needs the same rules written for it, before `php_fastcgi`:

  ```caddy
  @wpms_admin path_regexp ^/[^/]+/wp-admin$
  redir @wpms_admin {path}/ 301
  @wpms {
      not file
      path_regexp wpms ^/[^/]+(/wp-.*|/.*\.php)$
  }
  rewrite @wpms {re.wpms.1}
  ```

- **Hidden files:** `/shop2/.htaccess` gets 403 on nginx. On Caddy it reaches `index.php`, because Devid's Caddy rule for hidden paths matches only the top level. No file is served either way, as before this phase.
- **A real network** (`wp core multisite-install`, a second site, its admin) comes in S4, in the weekly template check.

### What S2 builds from this

- nginx: `server_name <host> ~^[a-z0-9-]+\.<name>\.<tld>$` per site, the site's name escaped; the `Origin` map gets one expression for the subdomains.
- Caddy: both addresses in each block.
- Apache: `ServerAlias *.<host>` and the 421 guard on every vhost of the site.
- `ca.rs`: the leaf names `<host>` and `*.<host>`, with a marker that the daemon's check reads to reissue older leaves once.
- `scripts/linux-e2e.sh`: a subdomain of its site, in curl and headless Chrome.

## S2 results (2026-11-28)

What changed is in the timeline record `docs/implementation-timeline/subdomains/2026-11-28-s2-routing-certificates.md`. In short:
- **The certificate:** a site's leaf names `<host>` and `*.<host>`. A `<host>.names` file beside it records them, so leaves from before D-065 are reissued once, and the web server reloads when one is.
- **The web servers:**
  - nginx gets the one-level expression from S1;
  - Caddy lists `*.<host>` in each block;
  - Apache gets the alias and the 421 guard, except on the local port.
- **Proxy sites:** the `Origin` rewrite covers the subdomains on all three servers.

**Checks on the M1 (Devid 0.22.0 built from `phase16-s2`; the owner's Devid and DBngin were not touched):**
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass: 184 tests with a fake HOME, and `passwd_home_matches_home` on its own with the real one.
- `scripts/smoke.sh` passed with the new subdomain step: a throwaway Devid on ports 28xxx, `acme.shop.test` against Devid's CA, and `a.b.shop.test` refused.

  | Web server | `acme.shop.test` (HTTPS, Devid's CA) | `a.b.shop.test` over HTTPS | `a.b.shop.test` over HTTP |
  |---|---|---|---|
  | nginx | the site | no valid certificate | 404 |
  | Caddy 2.11.9 | the site | no valid certificate | 404 |
  | Apache 2.4.67 (macOS) | the site | no valid certificate | 421 |

- **A Next.js 16 app and a Laravel 13 app,** on a throwaway Devid on ports 38xxx (dev servers on 38301 and 38173, away from the owner's 3xxx ports and 5173):

  | Check | nginx | Caddy | Apache |
  |---|---|---|---|
  | `https://acme.nx.test/` (proxy site) | 200 | 200 | 200 |
  | `/_next/hmr` upgrade with `Origin: https://acme.nx.test` | 101 | 101 | 101 |
  | the same with `Origin: https://other.test` | refused (000) | refused (502) | refused (502) |
  | `a.b.nx.test` over HTTP / HTTPS | 404 / handshake refused | 404 / handshake refused | 421 / no valid certificate |
  | `https://acme.lara.test/` (Laravel, `devid new laravel`) | the page, loading `https://lara.test:38173/@vite/client` | the same | the same |
  | Vite over HTTPS (D-056) with `Origin: https://acme.lara.test` | `Access-Control-Allow-Origin` echoes it | the same | the same |

  - **Apache's guard runs before `ProxyPass`:** a deeper name of a proxy site gets 421 and never reaches the dev server.
  - **Vite over HTTPS needs `APP_URL` without a port,** as D-056 says. On the spare ports `devid new` wrote `https://lara.test:38443`, so Vite ran on plain HTTP until the check set `https://lara.test`, which a real install, serving 443, writes.
- **Linux:** `scripts/linux-e2e.sh` now opens `https://acme.e2e.test/` in curl and in headless Chrome and refuses `a.b.e2e.test`. It runs on this pull request's CI.
- **Not checked by hand:** a browser on the M1 against the S2 build. Its daemon has its own CA, which no browser trusts, and S1 already ran the same configuration lines in Safari's stack, Chromium and Firefox. The owner's own try comes with the rc.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| S1 | **Spike:** a `*.<name>.test` certificate in Safari, Chrome and Firefox on macOS, and in Chrome and Firefox on Ubuntu; one-level matching on nginx, Caddy and Apache, and what each does with deeper names; Vite's HMR and Next.js's dev resources from a subdomain page; a subdirectory network's rewrite rules on nginx and Caddy | results in "S1 results" here and an S1 bullet in D-065; every open point below has an answer |
| S2 | **Routing and certificates** on nginx, Caddy and Apache; the one-time reissue; the wider `Origin` rewrite for proxy sites | unit tests for each web server's config; a subdomain of a PHP site and of a Vite site opens over HTTPS on the M1, through the spare-port daemon |
| S3 | **Accounts that may create databases** on MySQL and PostgreSQL, in the CLI | the account creates and drops `<name>_t1` and cannot create `other`; tests in `tests/` |
| S4 | **WordPress Multisite** in `devid new`: subdomain and subdirectory networks | both kinds added to the weekly template check (`scripts/new-e2e.sh`); a second site of each network opens and its admin loads |
| S5 | **The TUI, the app, the guide and 0.23.0:** the site line and the two options, `/docs/subdomains` in both languages, rc on the M1 and the Linux VMs | 0.23.0 released |

## Where it is tested

- **CI** on macOS and Ubuntu: unit tests for the config and the certificate's SANs, and the weekly template check for Multisite.
- **The owner's M1:** a second daemon on spare ports, as in Phase 15 (`DEVID_HOME` under the worktrees, ports 18088/18443). DBngin's servers are never touched.
- **The Ubuntu, Arch and CachyOS VMs** for the rc: a site's subdomain over 443 with the system CA, in a browser where the VM has one.

## Open points for S1

All answered in "S1 results" on 2026-11-28.

- How nginx limits a site to one level without slowing down the match for other sites, and whether Apache needs a check of its own for deeper names.
- Whether any browser treats a wildcard directly under a two-label name (`*.shop.test`) differently.
- Whether Vite's HMR websocket and Next.js's dev resources work from a subdomain with `pass_host` off, and which `Origin` they send.
- Which subdirectory-network rules Caddy needs. WordPress documents nginx's and Apache's.

## Owner's answers (2026-11-28)

- **Which phase:** "Subdomain dulu", after Claude compared a Linux desktop app, Windows 10/11 and subdomains. Claude recommended subdomains first (small, no idle cost, wanted by Phase 15's notes), then the Linux app, then Windows, starting with a spike; Windows 10 is past its end of support, so Windows 11 would be the target.
- **Every recommendation:** "Ya, ikuti semua":
  1. on for every site, no setting;
  2. one level;
  3. one certificate with `<name>.test` and `*.<name>.test`, reissued once, the same CA;
  4. proxy sites too, with the `Origin` rewrite widened;
  5. `--create-databases`: MySQL by the `<name>_` prefix, PostgreSQL `CREATEDB`, no MongoDB;
  6. WordPress Multisite in `devid new`, subdomain and subdirectory;
  7. one line and an (i) in the TUI and the app, no design board;
  8. no tenancy option for Laravel, and a guide page;
  9. subdomains through Share later;
  10. milestones S1 to S5, released as 0.23.0.

## Not in this phase

- **More than one level** of subdomains (`a.b.shop.test`).
- **Subdomains on a site's local port and through Share** (D-037).
- **A tenancy option** in `devid new laravel`.
- **MongoDB accounts that may create databases.**
- **Aliases:** a site answering on another name altogether, such as `shop-admin.test`.
