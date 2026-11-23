# Phase 12 L1 spike: Linux

These scripts answer the L1 questions in `docs/roadmap/phase-12.md` on GitHub's Ubuntu runners, through `.github/workflows/spike-linux.yml`. That workflow runs on `spike/linux-*` branches only and publishes nothing.

| Script | Question | What it does |
|---|---|---|
| `build-php.sh` | 1, 2 | PHP 8.4 from static-php-cli 2.8.5 with `SPC_TARGET=native-native-gnu.2.17` (zig), Xdebug and pcov via `--build-shared`, and apcu built separately with phpize and the same zig |
| `check-php.sh` | 1, 2 | runs that build: cli, php-fpm, the three `.so` files and the compiled-in set. The workflow runs it on the runner and in Ubuntu 22.04, 24.04 and 26.04 and Debian 12 and 13 containers |
| `build-nginx.sh` | 3 | nginx 1.30.7, fully static with musl, OpenSSL and zlib from static-php-cli |
| `system.sh` | 3–9 | Devid's own daemon (musl build) under a systemd user unit; nginx and php-fpm; an nftables redirect for ports 80/443; two ways to send `.test` to Devid's DNS through systemd-resolved; CA trust for curl, PHP, Chrome and Firefox; the `/proc` connection check; idle memory; and an uninstall that puts everything back |
| `established.py` | 8 | a prototype of `ondemand::established()` that reads `/proc` |
| `browser.py` | 6 | opens a URL in headless Chrome or Firefox through Selenium |

The site certificate in `system.sh` is signed with openssl from the Devid CA's key, with the same fields as `ca.rs`: the daemon issues one only for a site it serves, and this run has no runtime to serve with.
