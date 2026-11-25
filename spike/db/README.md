# Phase 14 D1 spike: databases

The scripts behind "D1: what the spike must answer" in `docs/roadmap/phase-14.md` (D-061). They run
devid-binaries' `db-latest` builds the way Devid would, without Devid:

- `fetch.py <work>` downloads every database series built for this computer, mongosh, Devid's PHP 8.5
  and its mongodb extension, each checked against its sha256 in the manifest.
- `spike.py <work>` runs each server on 127.0.0.1, with the open-files limit Devid's daemon passes on,
  and measures setup, readiness, idle memory, stop, and a restart after SIGKILL. With Devid's
  settings it also creates an account, resets its password, checks what each engine lets in from
  Devid's PHP, and with `--laravel` runs a Laravel migration with that account.

`.github/workflows/spike-db.yml` runs them on GitHub's macOS (Apple silicon and Intel) and Ubuntu
runners, and on Arch in its VM. On the owner's Mac they run with `--ports usual`, which takes the
usual port or the next free one as Devid will, next to DBngin's servers and without touching them.
