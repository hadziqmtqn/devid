//! The inbox shows an email's HTML through the app's own `mailview:` URL scheme (Phase 9 M6b,
//! D-051), never Mailpit's `/view/<id>.html`, which runs the email's scripts:
//! - the reply carries its own CSP: no scripts, frames, forms or plugins; styles, images and fonts load;
//! - the page shows it in a sandboxed iframe;
//! - `<meta http-equiv>` is switched off, so an email cannot move anywhere on its own;
//! - every link opens a new window, which `navigate` sends to the browser for web and mail links.

use devid_daemon::{
    paths::Paths,
    protocol::{Request, request},
};
use serde_json::Value;
use std::{process::Command, sync::Mutex};
use tauri::{
    Url,
    http::{Response, StatusCode, header},
};

const CSP: &str =
    "default-src 'none'; img-src * data:; style-src * 'unsafe-inline'; font-src * data:; form-action 'none'";

/// The message the page last opened, as the daemon sent it: the frame shows it without asking
/// Mailpit again, which would mark it read again after "Mark as unread".
static OPENED: Mutex<Option<Value>> = Mutex::new(None);

/// Called with every `mail_message` reply the page gets.
pub fn opened(reply: &Value) {
    *OPENED.lock().unwrap() = Some(reply["message"].clone());
}

/// `mailview://localhost/<id>`: that message's HTML, ready for the frame.
pub fn reply(path: &str) -> Response<Vec<u8>> {
    let (status, body) = match page(path) {
        Ok(html) => (StatusCode::OK, html),
        Err(e) => (
            StatusCode::NOT_FOUND,
            format!(
                "<p style=\"font: 13px -apple-system, sans-serif; color: #6E6E73; margin: 20px\">{}</p>",
                escape(&e)
            ),
        ),
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, CSP)
        .body(body.into_bytes())
        .expect("a valid response")
}

fn page(path: &str) -> Result<String, String> {
    let id = path.strip_prefix('/').filter(|id| is_id(id)).ok_or("Not a message")?;
    let paths = Paths::from_env();
    let kept = OPENED.lock().unwrap().clone().filter(|m| m["id"] == id);
    let message = match kept {
        Some(m) => m,
        None => request(&paths, &Request::MailMessage { id: id.into() })?["message"].take(),
    };
    let html = message["html"].as_str().filter(|h| !h.is_empty()).ok_or("This message has no HTML")?;
    let inline = message["inline"].as_array().filter(|a| !a.is_empty());
    let inbox = inline.and_then(|_| crate::api::inbox_url(&paths).ok());
    Ok(prepare(html, id, inline.map(Vec::as_slice).unwrap_or_default(), inbox.as_deref()))
}

/// The email's HTML, ready for the frame:
/// - its inline images (`cid:`) load from Mailpit;
/// - every `http-equiv` is renamed, so no `<meta http-equiv="refresh">` moves the frame without a click;
/// - every `color-scheme` is renamed too, so the email shows its light design, as in Gmail and Mailpit.
///   In dark mode, its dark rules could otherwise put light text on the white page behind it;
/// - a `<base>` makes links without a target open a new window. In the frame itself, WebKit blocks
///   a link to http:// (mixed content) before `navigate` is asked.
fn prepare(html: &str, id: &str, inline: &[Value], inbox: Option<&str>) -> String {
    let mut html = html.to_string();
    if let Some(inbox) = inbox {
        for p in inline {
            let (Some(cid), Some(part)) = (p["cid"].as_str(), p["part"].as_str()) else { continue };
            if !cid.is_empty() && !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
                html = html.replace(&format!("cid:{cid}"), &format!("{inbox}api/v1/message/{id}/part/{part}"));
            }
        }
    }
    let mut out = switch_off(&switch_off(&html, "http-equiv"), "color-scheme");
    // At the end, so the doctype stays first; the first `<base target>` in the page counts, so an
    // email's own still wins.
    out.push_str("<base target=\"_blank\">");
    out
}

/// Every `name`, in any case, renamed `data-off-name`: an attribute, CSS property or media feature
/// that nothing knows, so it does nothing.
// shortcut: the name changes in the email's visible text too, which only an email about HTML or CSS
// would show; parse the HTML if that ever matters.
fn switch_off(html: &str, name: &str) -> String {
    // ASCII lowercase keeps every byte offset, so the matches index `html` too.
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len() + 64);
    let mut last = 0;
    for (at, _) in lower.match_indices(name) {
        out.push_str(&html[last..at]);
        out.push_str("data-off-");
        last = at;
    }
    out.push_str(&html[last..]);
    out
}

/// Mailpit's message IDs: letters and digits.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Where the window's web view may go: the app's own pages and email pages. A web or mail link
/// clicked in an email opens in the browser instead. WebKit asks this before a link with
/// `target="_blank"` makes a new window, so those links come here too.
pub fn navigate(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "mailview" | "about" => true,
        "http" | "https" | "mailto" => {
            // Full path, as in api::open; off the main thread, which asks this.
            let url = url.to_string();
            std::thread::spawn(move || {
                let _ = Command::new("/usr/bin/open").arg(url).status();
            });
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn email_html_loses_its_pragmas_and_gets_its_inline_images() {
        let html = "<!DOCTYPE html><html><head><META HTTP-EQUIV=\"refresh\" content=\"0;url=https://x.test\">\
                    <meta http-equiv=Content-Type content=\"text/html\"></head><body><img src=\"cid:logo@app\">\
                    <img src=\"cid:evil\"></body></html>";
        let inline = [json!({"part": "1.2", "cid": "logo@app"}), json!({"part": "../x", "cid": "evil"})];
        let out = prepare(html, "AbC123", &inline, Some("http://127.0.0.1:2526/"));
        assert!(!out.to_ascii_lowercase().contains(" http-equiv"), "{out}");
        assert!(out.contains("<META data-off-HTTP-EQUIV=\"refresh\""), "{out}");
        assert!(out.contains("src=\"http://127.0.0.1:2526/api/v1/message/AbC123/part/1.2\""), "{out}");
        assert!(out.contains("src=\"cid:evil\""), "a part ID that is not one stays as it was");
        assert!(out.starts_with("<!DOCTYPE html><html><head>"), "the doctype stays first");
        let dark = "<meta name=\"color-scheme\" content=\"light dark\"><style>@media (prefers-color-scheme: dark) { body { color: #fff } }</style>";
        let out = prepare(dark, "a", &[], None);
        assert!(!out.contains(" color-scheme") && !out.contains("(prefers-color-scheme"), "{out}");
        assert!(
            out.contains("name=\"data-off-color-scheme\"") && out.contains("(prefers-data-off-color-scheme: dark)"),
            "{out}"
        );
        assert_eq!(prepare("<p>é</p>", "a", &[], None), "<p>é</p><base target=\"_blank\">");
    }

    #[test]
    fn only_message_ids_reach_the_daemon() {
        assert!(is_id("Rx8nHpVfW2zG7kTq9aLm3c"));
        for bad in ["", "../api", "a/b", "a%2Fb", "a?b", &"a".repeat(65)] {
            assert!(!is_id(bad), "{bad}");
        }
        assert_eq!(escape("<b>\"x\" & y</b>"), "&lt;b&gt;&quot;x&quot; &amp; y&lt;/b&gt;");
    }

    #[test]
    fn the_web_view_stays_on_the_app_and_email_pages() {
        let url = |u: &str| Url::parse(u).unwrap();
        assert!(navigate(&url("tauri://localhost/index.html")));
        assert!(navigate(&url("mailview://localhost/AbC123")));
        assert!(navigate(&url("about:blank")));
        for other in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,x",
            "ssh://host",
            "x-apple.systempreferences:",
        ] {
            assert!(!navigate(&url(other)), "{other}");
        }
    }
}
