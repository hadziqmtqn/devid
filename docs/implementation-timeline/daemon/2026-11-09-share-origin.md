# Fix: Share reaches the site on its local port or :80, not on 8088 (v0.12.1)

> Date: 2026-11-09 · Branch: `fix/share-origin`

## Problem

The owner shared erdbpro-saas.test from Devid.app. The trycloudflare.com link answered "host down", and `share-erdbpro-saas.log` showed every request failing with `dial tcp 127.0.0.1:8088: connect: operation timed out`. An earlier share of phpmyadmin.test, through the same port, had worked.

## Findings

All measured on the owner's Mac, over ten seconds each:

| Path to the same nginx | Connected |
|---|---|
| `127.0.0.1:8000` (erdbpro-saas's local port, also the owner's own tunnel) | 20/20, about 0.4 ms |
| `:80` and `:443` (pf redirects them to 8088 and 8443) | every time |
| `127.0.0.1:8088` direct | 0/20 |
| `127.0.0.1:8443` direct | 1/20 (the first, after 100 ms) |

- **Not the listener:** new listeners on 8089, 8444 and 18089 accepted at once. Hanging attempts sat in `SYN_RCVD` on nginx's socket, so the handshake never finished.
- **Not pf state:** `pfctl -s info` showed an empty state table, and the `com.apple` anchors were empty. Devid's anchor only has the two `rdr` rules.
- **Not nginx's age:** `devid restart nginx` left the pattern unchanged.
- **Not the owner's tunnel:** its port, 8000, was the one path that always answered.
- **Open:** the exact cause. Two things on the Mac could act at this level: the macOS application firewall is on and does not list Devid's nginx, and a Cloudflare WARP daemon still runs. Proving it would mean switching pf or those off.

## Fix

- `share_origin` picks the port cloudflared reaches the site on: the site's local port (D-037) when it has one that is not busy, else `:80` when pf forwards it (`standard_ports`), else `http_port`.
- Unit test `share_avoids_pfs_redirect_target`.
- `share_runs_cloudflared_and_reads_its_url` now also checks that a site with a local port is shared through that port.
- After the change, on the owner's Mac: 5/5 requests through `127.0.0.1:8000` (erdbpro-saas) and 5/5 through `127.0.0.1:80` (phpmyadmin) answered 200. The old path, `127.0.0.1:8088`, answered 1/3.
