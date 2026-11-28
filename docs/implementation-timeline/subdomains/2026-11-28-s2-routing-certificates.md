# Phase 16 S2: subdomains and their certificates on nginx, Caddy and Apache

> Date: 2026-11-28 · Branch: `phase16-s2` · Plan and results: `docs/roadmap/phase-16.md`, "S1 results" and "S2 results" · Decision: D-065

## What shipped

- **`ca.rs`:**
  - `site_names`: a site's certificate names `<host>` and `*.<host>`;
  - `ensure_site_cert` issues it with both names and writes them to `<host>.names` beside the leaf;
  - a leaf whose `.names` file is missing or different is reissued, so every leaf from before D-065 is reissued once.
- **`server.rs`:** a reissued certificate reloads the running web server, even when its config did not change.
- **`nginx.rs`:**
  - each site's `server_name` is its host plus `~^[a-z0-9-]+\.<host>$`, one level only;
  - a proxy site's `Origin` map also rewrites its subdomains' origins, on any port.
- **`caddy.rs`:**
  - each site's HTTPS and HTTP blocks also list `*.<host>`;
  - the `Origin` matcher takes the subdomains.
- **`apache.rs`:**
  - each `.test` vhost has `ServerAlias *.<host>` and answers deeper names with 421;
  - the `Origin` rewrite takes the subdomains;
  - the local port (D-037) is unchanged.
- **`scripts/smoke.sh`:** `acme.shop.test` against Devid's CA, and a deeper name refused, on nginx, Caddy and Apache (macOS) or Caddy (Linux). The release workflow runs it on every target.
- **`scripts/linux-e2e.sh`:** `acme.e2e.test` through the system store in curl and through NSS in headless Chrome, and a deeper name refused, on every pull request.
- **Tests:** the certificate's two SANs and the one-time reissue (`ca.rs`); each web server's names and `Origin` rules (`nginx.rs`, `caddy.rs`, `apache.rs`); the CLI's site test reads the new `server_name` line.

## Checked

See "S2 results" in `docs/roadmap/phase-16.md`.
