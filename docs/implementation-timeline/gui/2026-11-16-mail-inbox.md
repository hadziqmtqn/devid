# The inbox in the app (Phase 9 M6b, D-051)

> Date: 2026-11-16 · Branch: `feat/mail-inbox-app`

## What changed

- **The Mail page is the inbox** while Mail runs, as on the boards "Mail · Inbox" and "Mail · Inbox, empty":
  - **Left:** the messages, newest first, with an unread dot, the time, the subject, the recipients and a paperclip; a search field (Mailpit's search) and All or Unread. A message opened in the Unread view stays listed after it is read, as in Mail.
  - **Right:** the open message with Mark as read or unread, Delete, From, To, Cc, Date and size, and the tabs Preview, Text, Headers and Raw. Open in Mailpit opens it in the browser.
  - **Attachments** sit below the message, always in view, since the email scrolls in its frame; each one saves to ~/Downloads. The owner moved them there from the header after a first look.
  - **Empty:** "No mail yet" with the `.env` lines, Send test email and Mail settings.
  - **Toolbar:** Send test email, and ⋯ with Mark all as read, Open inbox in Mailpit, Mail settings… and Delete all….
  - **Keys:** ↑ and ↓ move through the list; Delete or Backspace deletes the open message.
- **Mail settings** is the stage 1 page, behind ⋯ (with a back button) and as the only view while Mail is stopped.
- **`crates/gui/src/mailview.rs`:** the `mailview://localhost/<id>` scheme serves the email's HTML for a sandboxed frame, with its own CSP (no scripts), `cid:` images pointed at Mailpit, `http-equiv` and `color-scheme` switched off, and `<base target="_blank">` appended. `navigate`, the window's navigation handler, opens http, https and mailto links in the browser and keeps the window on the app's pages.
- **Redraws:** `PAGES.mail.patch` redraws only the list while the open message stays, so its frame does not reload every 2 s.
- **Daemon:** `mail_message` also lists the inline parts (`inline`: part and Content-ID).
- **Menu bar and quick menu:** Open inbox opens the Mail page instead of the browser.
- **Page API:** `mail_list`, `mail_headers` and `mail_raw` count as reads (no menu-bar redraw); the `inbox` open target takes a message ID.

## Checks

- A debug build on a test daemon in `/tmp` with the real Mailpit 1.33.1 on 28525/28526, driven through the test app's accessibility elements only (presses and one value; no keystrokes), with one pid-checked click.
- A hostile email: styles, the `cid:` image and a remote image showed. Its script (which would have replaced the page and called a logger on 127.0.0.1:28599) did not run; its `<meta refresh>` did not fire.
- Links: Laravel's button and a plain link both opened in the browser; the frame stayed on the email. The plain link needed the `<base>`: WebKit blocked the frame's own navigation to http:// as mixed content before asking the app.
- Mark as unread, then Text and Preview again: still unread.
- Unread view, search, Delete (the next message opens), Mail settings and back, Delete all (the empty view), and the first message after it.
- Saving an attachment, with the test daemon's HOME in `/tmp`.
- At 760×520 the tab row wraps.
- Backgrounds, in dark mode, after the owner asked whether the page follows each email's template: a dark template shows dark, Laravel's shows its gray and white, an email without colors shows white. Two emails with dark-mode rules (with and without a `color-scheme` meta) showed white text on white until `color-scheme` was switched off; now they show their light design.
- Tests: `email_html_loses_its_pragmas_and_gets_its_inline_images`, `only_message_ids_reach_the_daemon`, `the_web_view_stays_on_the_app_and_email_pages`, `only_changes_redraw_the_menu_bar`, and the inline parts in `inbox_ids_queries_and_messages`.
- `cargo fmt`, `cargo clippy --all-targets -D warnings` and `cargo test` for the workspace and for `-p devid-gui`.

## Found and fixed while testing

- An email with dark-mode rules showed white text on the white page in dark mode. Each email now shows its light design (D-051).
- While the search field had focus, a redraw waited (so typing is never undone), and the old view's Delete button deleted the newly picked message. The view's buttons now act on the message they show, and picking a message or a tab leaves the search field.

## Left

- Saving to ~/Downloads from the daemon may make macOS ask whether `devid` may use the Downloads folder, and again after an update (D-051). Not seen on the test setup.
- The arrow keys and Delete in the list were not tried: GUI tests send no keystrokes.
- The menu-bar item was not pressed in the test (its menu does not open through accessibility); it calls the same `open_window(…, "mail")` as a notification's Show.
