# Phase 16 S4: WordPress Multisite in `devid new`

> Date: 2026-11-28 · Branch: `phase16-s4` · Plan and results: `docs/roadmap/phase-16.md`, "S1 results" and "S4 results" · Decision: D-065

## What shipped

- **`newproject.rs`:**
  - `multisite` checks `--multisite`: WordPress only, `subdomain` or `subdir`;
  - `Options.multisite`;
  - for a network, the WordPress job runs `wp core multisite-install` (`--subdomains` for subdomains) and writes WordPress's `.htaccess` for the kind (`HTACCESS_SUBDIR`, `HTACCESS_SUBDOMAIN`);
  - its step is named "Installing WordPress Multisite", and its notes say how to add a site.
- **`sites.rs`:** `wp_subdir_network` reads `wp-config.php` for `MULTISITE` without `SUBDOMAIN_INSTALL`, and `Site.wp_subdir` is set for a WordPress site in `php` mode.
- **`nginx.rs`:** for such a site, WordPress's subfolder rules come before the locations, with a relative `/wp-admin` redirect so the port stays.
- **`caddy.rs`:** for such a site, the same rules as matchers, before `php_fastcgi`.
- **`protocol.rs` and `server.rs`:**
  - `NewProject` gets `multisite`;
  - `new_project` takes the template's options as `Choices` (`--db`, `--kit`, `--multisite`).
- **CLI:** `devid new … --multisite subdomain|subdir`.
- **The TUI** sends no `multisite` until S5.
- **`scripts/new-e2e.sh`:** `wp-subdir` and `wp-subdomain`, each with a second site, its page and its admin.
- **Tests:**
  - the detection (`sites.rs`);
  - the rules (`nginx.rs`, `caddy.rs`);
  - `--multisite` and the `.htaccess` texts (`newproject.rs`);
  - the CLI's refusals (`crates/cli/tests/new.rs`).

## Checked

See "S4 results" in `docs/roadmap/phase-16.md`.
