# Phase 9 Plan: Mail

> Scope: a mail catcher. Devid runs Mailpit, so the mail that apps send lands in an inbox on this Mac instead of real mailboxes (D-049)
> Verified against: 2026-11-16 (v0.14.1 installed on the owner's Mac; Mailpit 1.33.1)
> Status: accepted 2026-11-16. M1 (spike) done, see "M1 results".

Apps on the owner's Mac send their mail to a Mailpit the owner set up by hand: `~/.local/bin/mailpit` 1.33.0, started by the LaunchAgent `com.axllent.mailpit`, with SMTP on 127.0.0.1:1025 and its inbox on 127.0.0.1:8025. Laravel's `.env.example` (erdbpro-saas has it) points at 127.0.0.1:2525.

Phase 9 lets Devid download, run and show a mail catcher from the CLI, the TUI and the desktop app, in two stages:

- **Stage 1, 0.15.0:** Mail in the daemon, `devid mail`, the TUI and the app: on/off, status, settings, the `.env` lines, and a Mail section in the quick menu. "Open inbox" opens Mailpit's own page in the browser.
- **Stage 2, a later 0.15.x:** the inbox inside the app, over Mailpit's API.

The design is on the canvas page "0.15.x" (`https://claude.ai/artifact/TS8URDyBeQ4D5PaozQsDUa`), approved by the owner on 2026-11-16:

- stage 1: "Mail · Off, turn on", "Mail · Setup and status" and "Quick menu · Mail";
- stage 2: "Mail · Inbox" and "Mail · Inbox, empty".

## Ground rules

- **Client rule (D-012):** Mail's state and actions live in the daemon. The CLI, the TUI and the app render it and send requests, and every action is a CLI subcommand.
- **Nothing runs by itself:**
  - Mail is off by default, and Mailpit is downloaded only when the user turns Mail on.
  - With Mail off, the daemon's idle RAM and CPU stay as they are now, and no Mail port is open.
- **Local only:**
  - Mailpit's SMTP server and its inbox listen on 127.0.0.1, never on every interface (Mailpit's default).
  - The inbox only answers requests whose Host header is its own address, so a web page cannot read mail through DNS rebinding.
  - Mailpit does not check for updates and never relays mail.
- **Owner's machine:** the owner's own Mailpit (1025 and 8025, its LaunchAgent and database) is never touched. Devid's defaults use other ports, and removing the owner's Mailpit is the owner's call.
- **Compatibility:** with Mail off, sites and PHP behave exactly as today.

## Design

### Mailpit in the runtime release (M2, `devid-binaries`)

- `scripts/mirror-mailpit.sh`, like `mirror-caddy.sh`:
  - downloads the official `mailpit-darwin-amd64.tar.gz` or `-arm64.tar.gz`;
  - checks it against the sha256 digest that GitHub's releases API gives for that asset, since Mailpit publishes no checksums file;
  - repackages `mailpit`, `LICENSES/mailpit.txt` and `BUILDINFO` as `dist/mailpit-<ver>-darwin-<arch>.tar.gz`.
- `manifest.py` accepts the `mailpit` tool (its artifact name pattern lists php, nginx and caddy today) and writes an entry shaped like Caddy's: version, then per arch the URL, sha256 and size.
- `release.yml` gets a `mailpit` job next to the `caddy` one (one version for both arches), and the release job waits for it.
- `upstream.py` learns Mailpit's latest release, so the weekly `check-upstream.yml` starts a runtime release when Mailpit moves.

### The Mail service (M3, daemon)

- **Install:** the first `mail on` downloads the `mailpit` artifact with `releases::download` (sha256 checked) to `~/.devid/mailpit/bin/mailpit`, the way `install_server` installs Caddy. `mailpit version` gives the installed version.
- **Config:** a `[mail]` table in `config.toml`, with `enabled = false`, `port = 2525`, `keep = 500`, `php_mail = true` and `notify = false`. The inbox listens on `port + 1`.
- **Run:**
  - `State::apply` wants a `mail` service while `enabled` is true and Mailpit is installed.
  - Its Spec runs `mailpit --smtp 127.0.0.1:<port> --listen 127.0.0.1:<port+1> --database ~/.devid/mail/mailpit.db --max <keep> --log-file ~/.devid/log/mailpit.log --disable-version-check --smtp-disable-rdns --smtp-auth-accept-any --smtp-auth-allow-insecure --allowed-hosts 127.0.0.1:<port+1>,localhost:<port+1>`, stopped with SIGTERM.
  - A settings change changes the Spec, so `reconcile` restarts Mailpit. The mail stays in its database.
  - `mail` joins `services()`, so `devid stop|start|restart mail` work, and Stop all and Start all cover it.
  - Mailpit writes no pid file, like Caddy, whose orphan cleanup does nothing today. M3 lets the supervisor write the pid itself, so a Mailpit left over from a crashed daemon is stopped instead of holding the port.
- **Ports:** both must be 1024 or higher, not one of Devid's own (http, https, DNS), not a site's local port, and free. A busy port is reported with its holder, for example "Port 1025 is in use by mailpit (pid 439)", as `check_port` does for local ports.
- **PHP's `mail()`:**
  - While Mail runs with `php_mail` on, Devid writes `etc/conf.d/devid-mail.ini` for each installed PHP version, with `sendmail_path = "'<home>/mailpit/bin/mailpit' sendmail -S 127.0.0.1:<port>"`, and restarts its php-fpm.
  - Turning either off removes the file.
  - php-fpm and the CLI shim read the same `conf.d`, so mail from `php artisan tinker` lands there too.
- **Counts:** while Mail runs, `status` asks Mailpit's `/api/v1/info` on 127.0.0.1 for the version and the message and unread counts, with a short timeout. The daemon has no HTTP client, so this is a few lines of HTTP/1.1 over a `TcpStream`.
- **Test mail:** `mail_test` sends one short message to Mailpit over SMTP, the path apps use.
- **Clear:** `mail_clear` deletes every message through Mailpit's API.
- **Doctor:** with Mail on, one line for Mailpit and its port.
- **Updates:** `plan_updates` treats Mailpit like nginx and Caddy (version compare), and `devid update` restarts it.
- **Smoke test:** `scripts/smoke.sh`, which every devid release runs on both arches, turns Mail on, sends a test mail and checks the count. That covers the arm64 build, which the owner's Mac cannot run.

### Daemon API

- `mail_set {enabled?, port?, keep?, php_mail?, notify?}` changes `[mail]` through `State::mutate`, so a Mailpit that cannot start rolls back like a server change.
- `mail_test` and `mail_clear`.
- `status` gains `mail`: `{enabled, installed, version, state, port, inbox_url, total, unread, keep, php_mail, notify, error}`, where `state` is `off`, `starting`, `running`, `paused` or `failed`.

### Clients, stage 1 (M4)

- **CLI:**
  - `devid mail` shows the status.
  - `devid mail on|off`, `devid mail port <n>`, `devid mail keep <n>`, `devid mail php-mail on|off` and `devid mail notify on|off` change settings.
  - `devid mail test` sends a test mail, `devid mail open` opens the inbox in the browser, and `devid mail clear` deletes all mail.
  - `devid status` prints a `mail:` line.
- **TUI:** a Mail page after Server, as in the app's sidebar, with the status, on/off, test, open and clear. Logs and Settings move to keys 5 and 6.
- **App:**
  - Mail in the sidebar between Web Server and Logs, with the unread count, and `--page mail`.
  - The Mail page as on the boards "Mail · Off, turn on" and "Mail · Setup and status".
  - "Open inbox" opens the inbox URL from `status`, through a new `Open::Inbox` target, since `api.rs` only opens fixed kinds of targets.
  - Both quick menus, the window's status card menu and the tray, get a Mail section: "N new", "Open inbox" and "Send test email", or "Turn on Mail".
  - Logs: `mailpit.log` in a Mail group.
  - Notifications: the tray compares the unread count at each refresh (every 15 s, and after each request) and posts a notification when it grows. M4 picks the way to post it (Tauri's notification plugin, or the system API) that works for the ad-hoc signed app. As built: the newest message instead of the count, checked every 5 s, through `NSUserNotificationCenter`, because UserNotifications refuses ad-hoc signed apps (D-050).

### Stage 2 (M6)

- The app reads Mailpit's API through a Tauri command limited to `/api/v1/` and `/view/` at the inbox URL from `status`, so the window's CSP stays as it is.
- A message's HTML is shown in a sandboxed frame with scripts off, and its links open in the browser.
- New mail arrives over Mailpit's `/api/events`.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| M1 | **Spike** | the numbers and answers are in this doc (done 2026-11-16, see "M1 results") |
| M2 | **Mailpit in the runtime release** (`devid-binaries`) | a runtime release lists Mailpit for both arches in `manifest.json`, checked against GitHub's digest of the official build |
| M3 | **Daemon and CLI:** the Mail service, its config, status, `devid mail`, PHP's `mail()`, doctor, updates | on the owner's Mac, `devid mail on` downloads and starts Mailpit on 2525, a password reset mail from erdbpro-saas lands in it, and `devid mail off` stops it |
| M4 | **App and TUI, stage 1:** the Mail page, the quick menu, notifications | the owner turns Mail on in Devid.app and opens the inbox from the quick menu |
| M5 | **Docs and release 0.15.0:** the docs on `devid-site`, README, the timeline record | released and installed on the owner's Mac (the release waits for the owner's go-ahead) |
| M6 | **Stage 2: the inbox in the app** | the "Mail · Inbox" boards work in Devid.app on the owner's Mac; released as a 0.15.x |

Each milestone is one worktree and one PR. M2 goes first: M3's end-to-end check needs Mailpit in a runtime release.

## M1 results (2026-11-16)

Mailpit 1.33.1, `mailpit-darwin-amd64.tar.gz`, run from a scratch folder on the owner's Mac on 127.0.0.1:2526 (SMTP) and 8026 (inbox).

- **Download:** 11.1 MB, a 29 MB `x86_64` binary. Its sha256 matches the digest GitHub records for the release asset. Mailpit publishes no checksums file, unlike Caddy.
- **Start:** both ports accept connections 0.17 s after launch.
- **Cost:**

  | State | RSS | CPU in total |
  |---|---|---|
  | started, then 1 min idle | 16.3 MB | 0.05 s |
  | after 200 messages, then 5 min idle | 24.5 MB | 0.49 s, the same as 1 min after them |

  - It runs 9 threads, and its only sockets are its two listeners. `/api/v1/info` reports `LatestVersion: disabled`, so it makes no update check.
  - The database is 0.3 MB with 203 messages (plus a 4 MB write-ahead log until SQLite checkpoints it). Messages survive a restart.
- **Sending:** all three ways an app sends mail work.
  - Plain SMTP.
  - SMTP with a login (AUTH PLAIN, as Laravel does when `MAIL_USERNAME` is set): any user name and password is accepted, and Mailpit stores the user name with the message. A site could be told apart by it later.
  - PHP's `mail()`, with Devid's PHP 8.4.27 and `sendmail_path = "<mailpit> sendmail -S 127.0.0.1:2526"`: `mail()` returns true and the message arrives.
- **API** (everything stage 2 needs):
  - The list gives `total` and `unread`, and for each message its ID, From, To, Subject, Snippet, Read, Tags, the number of attachments, Size and Username.
  - One message gives its HTML, its text and its attachments with their part IDs. Reading it through the API marks it read.
  - Also: `/view/<ID>.html`, the raw source, the headers, an attachment by part, setting messages read or unread (`PUT /api/v1/messages`), and live events over a WebSocket (`/api/events` upgrades).
- **Security:**
  - By default Mailpit listens on every interface (`[::]:1025` and `[::]:8025`) and answers any Host header.
  - With 127.0.0.1 addresses and `--allowed-hosts 127.0.0.1:<port>,localhost:<port>`, a request with another Host header gets 403.
- **Flags Devid passes:** `--smtp 127.0.0.1:<port>`, `--listen 127.0.0.1:<inbox port>`, `--database <file>`, `--max <messages>`, `--disable-version-check`, `--smtp-disable-rdns`, `--smtp-auth-accept-any`, `--smtp-auth-allow-insecure`, `--allowed-hosts …`, `--log-file <file>`.

Not measured yet: idle CPU over hours (M3, on the owner's Mac) and the arm64 build (M2, on GitHub's Apple silicon runner).
