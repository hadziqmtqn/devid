# Phase 15 N1 spike: creating projects

The script behind "N1 results" in `docs/roadmap/phase-15.md` (D-063). It runs each framework's
official create tool the way Devid's daemon would: with fixed options, no terminal (stdin is
`/dev/null`), and its own empty Composer, npm and WP-CLI caches, so the times are a first run's.

- `spike.sh <work> [template...]` makes each project in `<work>/proj` and records its exit code, its
  time and its disk use in `<work>/results.txt`. What the tools print goes to `<work>/logs`.
- For the Node templates it also starts `npm run dev` on a port from 4701 up and waits for a 200.
- `wordpress` downloads WP-CLI's phar from its latest GitHub release and checks it against the
  SHA-512 published there. It then downloads WordPress twice, once with `wp core download`'s
  default archive and once from the zip that wordpress.org's version API offers, and runs
  `wp core verify-checksums` on each.

PHP, Composer and Node come from PATH. On the owner's M1 they are Devid's PHP 8.5 and Composer,
and Node 24 from nvm. On the Ubuntu VM they were Devid's PHP 8.5 build from devid-binaries,
`composer.phar` and Node 24's tarball from nodejs.org, all unpacked into a folder in the home
directory and checked against their published checksums, with nothing installed system-wide.
