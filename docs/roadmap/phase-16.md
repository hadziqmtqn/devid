# Phase 16 Plan: Subdomains of a site

> Scope: every site also answers on its subdomains, `https://<sub>.<name>.test`, with a certificate for them; accounts that may create databases; WordPress Multisite in `devid new` (D-065)
> Verified against: 2026-11-28 (Devid 0.22.0, main c11bebd; `dns.rs`, `ca.rs`, `nginx.rs`, `caddy.rs`, `apache.rs`, `db.rs`)
> Status: accepted 2026-11-28 (D-065): the owner chose this phase over a Linux desktop app and Windows ("Subdomain dulu") and took every recommendation ("Ya, ikuti semua"). Its answers are in "Owner's answers".

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
