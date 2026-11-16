# Mail in the daemon and the CLI (Phase 9 M3, D-049)

> Date: 2026-11-16 · Branch: `feat/mail-daemon`

## What changed

- **Config:** `[mail]` with `enabled` (off), `port` (2525), `keep` (500), `php_mail` (on) and `notify` (off). A config without the table gets these defaults.
- **`daemon::mail`:**
  - `install` copies the release's `mailpit` to `~/.devid/mailpit/bin/mailpit` after `mailpit version` answers, and records the version.
  - `spec` runs it on 127.0.0.1 only: SMTP on `port`, the inbox on `port + 1`, a Host allowlist, no update check, no reverse DNS, any SMTP login accepted, the database in `~/.devid/mail/`, the log in `~/.devid/log/mailpit.log`.
  - `sync_php` writes or removes `conf.d/devid-mail.ini` (`sendmail_path` into Mailpit's sendmail mode) for every installed PHP version and reports the versions whose file changed.
  - `info` and `clear` call Mailpit's API over HTTP/1.0 on a `TcpStream`; `send_test` speaks SMTP; `rfc2822` formats the test mail's Date.
- **Supervisor:** `Spec.write_pid`, so the supervisor writes the pid of a service that writes none (Mailpit). The orphan check matches `mailpit/bin/mailpit`, never a Mailpit the user runs from elsewhere.
- **Server:**
  - `apply` syncs the PHP ini, wants the `mail` service while Mail is on, and restarts each running php-fpm whose ini changed.
  - `services()` lists `mail`, so stop/start/Stop all/Start all and `autostart` cover it.
  - `mail_set` downloads Mailpit the first time it is turned on, clears a `stop mail`, and refuses ports below 1024 or 65535, Devid's own ports, a site's ports, and busy ports (naming their holder). A refused change leaves the config as it was.
  - `mail_test` and `mail_clear` need a running Mailpit.
  - `status` gains `mail`; while Mailpit runs, its version and counts come from `/api/v1/info`. A failed Mailpit reports the holder of its port.
  - `plan_updates` and `apply_updates` update Mailpit like nginx and Caddy.
- **CLI:** `devid mail [on|off|port|keep|php-mail|notify|test|open|clear]`, a `mail:` line in `devid status`, and a doctor check while Mail is on.
- **Smoke test:** `scripts/smoke.sh` turns Mail on with the real Mailpit when the runtime release has it, sends a test mail and one from PHP's `mail()`, expects both in the inbox, and expects 403 for a foreign Host header. Until the runtime release has Mailpit, it says it skipped Mail.

## Checks

- Unit tests (`daemon::mail`): the version line, mail dates (1970, 2000-03-01, 2024-02-29, 2026), Mailpit's flags, refused ports, the PHP ini for every version and its removal, API replies (2xx only), the SMTP exchange against a fake server, and an SMTP refusal. `config`: `[mail]` defaults.
- Integration test `crates/cli/tests/mail.rs`, with a fake Mailpit from a `file://` release: off by default; on downloads, runs it with the expected flags and writes the PHP ini and the pid file; stop and start; refused ports (Devid's HTTP port, a busy port) and `keep 0` leave the config as it was; `keep` restarts it with the new `--max`; `php-mail off` removes the ini; a newer Mailpit in the release is updated; off stops it, removes the ini and keeps the binary and the mail.
- End to end on the owner's Mac (x86_64), in a throwaway home on spare ports:
  - `scripts/smoke.sh` ran with a local `file://` release: runtime-latest's PHP 8.5.12 and nginx 1.30.6 archives, plus the Mailpit 1.33.1 archive from M2's mirror script.
  - PHP served the site, Mail came on, the test mail and a mail from PHP's `mail()` both arrived (2 messages), and a foreign Host header got 403.
  - Afterwards no Mailpit of Devid's was left running.
- The end-to-end run found one bug, which the fake had hidden: `mailpit version` starts its line with the path Mailpit was run from, not with "mailpit". The first install refused the real binary. The parser now accepts a path ending in `mailpit`, and a unit test has the real line.

## Not checked yet

- arm64: `scripts/smoke.sh` runs on GitHub's Apple silicon runner at the next devid release, once the runtime release has Mailpit.
- The owner's own daemon. M3's "done when" (a password reset mail from erdbpro-saas) needs the runtime release with Mailpit and a devid build with this change installed.
