# M2: PHP manager and php-fpm supervision

> Date: 2026-11-02 · Branch: `phase2/m2-php` · Plan: [Phase 2](../../roadmap/phase-2.md)

## Outcome

- `devid php install <ver> --from <dir>` copies `php`, `php-fpm` and `modules/*.so` into `~/.devid/php/<ver>/`.
  - It checks the version against D-009 and against what the binary reports.
  - It installs through a temp dir and a rename, so a failed copy never leaves a half-installed version.
  - On reinstall it keeps the owner's `etc/`.
- Per version, Devid writes:
  - `etc/php.ini` once. It contains the base settings, `curl.cainfo`/`openssl.cafile` pointing at `~/.devid/ca/cacert.pem`, and JIT disabled.
  - `etc/php-fpm.conf` on every install. It sets `pm = ondemand`, the socket at `~/.devid/run/php<NN>.sock` with mode 0600, and the log at `~/.devid/log/`.
- The daemon supervises one php-fpm master for the default version.
  - It restarts the master on a crash and gives up after 5 crashes in 60 s.
  - It stops an orphaned master left by a dead daemon, checked by pid file and process name.
  - `devid php use <ver>` switches the default, saves `config.toml` atomically, and starts and stops masters to match.
- `devid status` and `devid php list` show each installed version as running (with pid), stopped or failed.

The `--from <dir>` install is a shortcut. Downloading release assets comes with the Phase 5 build pipeline.

## Problems found and fixed

- **Stop hang.** php-fpm drops signals that arrive during its startup. A `daemon stop` about 1 s after `php use` left the daemon waiting forever. The stop sequence now repeats SIGQUIT every second and sends SIGKILL after 10 s. A regression test uses a fake php-fpm that ignores signals for its first 1.5 s.
- **JIT setting.** `opcache.jit = off` parses as an empty boolean, which is ambiguous. Devid now writes `opcache.jit = disable` with `opcache.jit_buffer_size = 0`; this was verified as `disable` and `0` on 8.3, 8.4 and 8.5.

## Verification

- `cargo test`:
  - `php_install_use_supervise`: validation errors, install, the default starting, switching versions, crash restart, and stop ending PHP.
  - `stop_during_php_startup_does_not_hang`.
  - The unit tests.
  - These tests use fake php/php-fpm scripts, so they run without PHP builds.
- Manual check with the real static builds (`spike/php/out/8.3–8.5`) in a temp `DEVID_HOME`:
  - install takes about 0.2 s per version, and `php-fpm -t` passes on the generated config;
  - `php use` switches masters;
  - an immediate `daemon stop` completes in 2.2 s with no leftover processes.
- `cargo clippy --all-targets -- -D warnings` is clean.

## Dependencies added

`libc` (to send SIGQUIT/SIGKILL; std can only SIGKILL).
