//! Dev servers on demand (D-054, Phase 10): a request to a proxy site whose dev server is down
//! starts the command the owner marked for it, and the command stops again once the site has been
//! idle for its `idle_stop` minutes.
//!
//! The web server asks over `run/wake.sock` (see `site_body` in `nginx`, `caddy` and `apache`): one
//! HTTP/1.x request per connection, naming the site in `X-Devid-Site`. The daemon starts the
//! command and waits until its port accepts. A page load still starting after 5 s gets a page that
//! reloads itself; any other request waits up to 60 s. The daemon never answers 502, which nginx
//! and Caddy give when they cannot reach the wake socket, as when the daemon is not running.
//!
//! - **nginx and Caddy** send no body, and Caddy sends the original method in `X-Devid-Method`.
//!   The daemon answers 418 once the port accepts, and the web server sends the same request,
//!   method and body included, to the dev server.
//! - **Apache** reaches the socket through `ErrorDocument 503`, which turns the request into a
//!   GET and may still pass its body. It sends the original request line in `X-Devid-Request`,
//!   and the daemon answers 307 to the same URL, which the client follows with its method and
//!   body. A body Apache passes is read and dropped first.

use crate::{
    commands::{self, Commands},
    config::AppCommand,
    logs,
    paths::Paths,
    ports,
    sites::Site,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::net::UnixStream,
    sync::Mutex,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// How long a browser page load waits before it gets the Starting page instead.
const PAGE_WAIT: Duration = Duration::from_secs(5);
/// How long any other request waits for the dev server.
const REQUEST_WAIT: Duration = Duration::from_secs(60);
/// How often the idle check runs while an on-demand command runs.
pub const IDLE_CHECK: Duration = Duration::from_secs(30);
/// The request file is emptied past this size; only its modification time matters.
const REQUESTS_MAX: u64 = 64 * 1024;
/// Header bytes a wake request may have.
const HEADER_MAX: usize = 16 * 1024;
/// Log lines on the Starting and failure pages.
const TAIL_LINES: usize = 20;

#[derive(Default)]
pub struct OnDemand {
    /// Site -> when a request woke its command or the idle check last saw a connection to it.
    used: Mutex<BTreeMap<String, SystemTime>>,
    /// Wake-ups start a command one at a time, so requests that arrive together start it once.
    start: Mutex<()>,
    /// Whether the idle check's thread runs; it runs only while an on-demand command does.
    pub watching: Mutex<bool>,
    /// Whether the wake socket is bound: from the first on-demand site until the daemon stops.
    pub listening: Mutex<bool>,
}

impl OnDemand {
    fn mark_used(&self, site: &str, at: SystemTime) {
        self.used.lock().unwrap().insert(site.to_string(), at);
    }

    /// The last sign that the site was used: a request in the web server's file, a wake-up or an
    /// open connection the idle check saw, or the start (Unix seconds) of its command.
    pub fn last_use(&self, paths: &Paths, site: &str, since: u64) -> SystemTime {
        let started = UNIX_EPOCH + Duration::from_secs(since);
        let request = fs::metadata(paths.last_request(site)).and_then(|m| m.modified()).ok();
        let used = self.used.lock().unwrap().get(site).copied();
        [request, used].into_iter().flatten().fold(started, SystemTime::max)
    }
}

/// What the web server asks: which site, and whether the request is a browser's page load.
#[derive(Debug, PartialEq)]
pub struct WakeRequest {
    pub site: String,
    /// The original request is a GET that accepts `text/html`: it may get the Starting page.
    pub page: bool,
    /// The wake request itself is a HEAD (nginx passes the method on), so the answer has no body.
    /// Caddy and Apache ask with a GET whatever the method, and read the body of the answer.
    pub head: bool,
    /// Apache's original request target: once the port accepts, the client is sent there (307).
    pub redirect: Option<String>,
    /// The body the web server sends with the wake request, which the daemon reads and drops.
    pub body: Body,
}

/// How a wake request's body is framed.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Body {
    None,
    Length(u64),
    Chunked,
}

/// The request line and headers of a wake request; None without `X-Devid-Site`.
pub fn parse(head: &str) -> Option<WakeRequest> {
    let mut lines = head.split("\r\n");
    let wire = lines.next()?.split(' ').next()?.to_string();
    let mut method = wire.clone();
    let (mut site, mut accept, mut redirect, mut body) = (None, String::new(), None, Body::None);
    for line in lines {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "x-devid-site" => site = Some(value.to_string()),
            "accept" => accept = value.to_ascii_lowercase(),
            // Caddy changes the wake request to a GET, so that it sends no body.
            "x-devid-method" if !value.is_empty() => method = value.to_string(),
            // Apache's original request line, e.g. "POST /login?next=%2F HTTP/1.1".
            "x-devid-request" => {
                let mut parts = value.split(' ');
                method = parts.next().unwrap_or("GET").to_string();
                redirect = Some(redirect_target(parts.next().unwrap_or("/")));
            }
            "content-length" if body != Body::Chunked => body = value.parse().map_or(Body::None, Body::Length),
            "transfer-encoding" if value.to_ascii_lowercase().contains("chunked") => body = Body::Chunked,
            _ => {}
        }
    }
    Some(WakeRequest {
        site: site?,
        page: method == "GET" && accept.contains("text/html"),
        head: wire == "HEAD",
        redirect,
        body,
    })
}

/// A request target that is safe in a `Location` header: a path on the same site, else "/".
fn redirect_target(target: &str) -> String {
    let safe = target.starts_with('/') && !target.starts_with("//") && target.bytes().all(|b| b.is_ascii_graphic());
    if safe { target.to_string() } else { "/".to_string() }
}

/// Read one wake request from the web server, its body read and dropped; None for anything else.
pub fn read_request(stream: &mut UnixStream) -> io::Result<Option<WakeRequest>> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > HEADER_MAX {
            return Ok(None);
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let Some(req) = parse(&String::from_utf8_lossy(&buf[..end])) else { return Ok(None) };
    // The web server sends the whole body before it reads the answer.
    let mut rest = BufReader::new((&buf[end + 4..]).chain(stream));
    match req.body {
        Body::None => {}
        Body::Length(n) => {
            if io::copy(&mut (&mut rest).take(n), &mut io::sink())? < n {
                return Ok(None);
            }
        }
        Body::Chunked => drop_chunked(&mut rest)?,
    }
    Ok(Some(req))
}

/// Read a chunked body to its end and drop it.
fn drop_chunked(reader: &mut impl BufRead) -> io::Result<()> {
    let line = |reader: &mut dyn BufRead| -> io::Result<String> {
        let mut line = String::new();
        if Read::take(&mut *reader, 4096).read_line(&mut line)? == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        Ok(line)
    };
    loop {
        let size_line = line(&mut *reader)?;
        let size = size_line.split(';').next().unwrap_or("").trim();
        let size = u64::from_str_radix(size, 16).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        if size == 0 {
            // Trailers, if any, end with an empty line.
            while !line(&mut *reader)?.trim().is_empty() {}
            return Ok(());
        }
        // The chunk and its CRLF.
        if io::copy(&mut Read::take(&mut *reader, size + 2), &mut io::sink())? < size + 2 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
    }
}

/// The daemon's answer to a wake request.
#[derive(Debug, PartialEq)]
pub struct Reply {
    pub code: u16,
    pub html: bool,
    pub body: String,
    /// The `Location` of a 307.
    pub location: Option<String>,
}

impl Reply {
    /// The dev server accepts now. 418: the web server sends it the request; 307 (Apache): the
    /// client sends it again to the same URL.
    fn ready(req: &WakeRequest) -> Self {
        match &req.redirect {
            Some(target) => Self { code: 307, html: false, body: String::new(), location: Some(target.clone()) },
            None => Self::text(418, String::new()),
        }
    }

    fn text(code: u16, body: String) -> Self {
        Self { code, html: false, body, location: None }
    }

    fn html(code: u16, body: String) -> Self {
        Self { code, html: true, body, location: None }
    }

    /// The site is not (or no longer) on demand, or the web server is not one that wakes it.
    pub fn not_on_demand(site: &str) -> Self {
        Self::text(503, format!("{site} is not on demand in Devid; start its dev server (or its command in Devid).\n"))
    }

    /// The HTTP response, without its body for a HEAD request.
    pub fn bytes(&self, head: bool) -> Vec<u8> {
        let reason = match self.code {
            307 => "Temporary Redirect",
            418 => "I'm a teapot",
            500 => "Internal Server Error",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "",
        };
        let kind = if self.html { "text/html" } else { "text/plain" };
        let mut out = format!(
            "HTTP/1.1 {} {reason}\r\nContent-Type: {kind}; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n",
            self.code,
            self.body.len()
        );
        if self.code == 503 {
            out.push_str("Retry-After: 1\r\n");
        }
        if let Some(location) = &self.location {
            out.push_str(&format!("Location: {location}\r\n"));
        }
        out.push_str("Connection: close\r\n\r\n");
        let mut bytes = out.into_bytes();
        if !head {
            bytes.extend_from_slice(self.body.as_bytes());
        }
        bytes
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Start `command` for `site` unless it runs, and wait until its port accepts (`Reply::ready`),
/// it exits, or the wait for this kind of request is over. `started` runs once the command runs.
pub fn wake(
    od: &OnDemand,
    commands: &Commands,
    paths: &Paths,
    site: &Site,
    command: &AppCommand,
    req: &WakeRequest,
    started: impl FnOnce(),
) -> Reply {
    let page = req.page;
    od.mark_used(&site.name, SystemTime::now());
    let note = match site.idle_stop {
        0 => format!("started by a request to {} (on demand)", site.host),
        m => format!("started by a request to {} (on demand; it stops after {m} min idle)", site.host),
    };
    let result = {
        let _one = od.start.lock().unwrap();
        commands.start(
            &site.name,
            &command.name,
            &command.run,
            &site.path,
            &commands::site_vars(paths, site),
            Some(&note),
        )
    };
    if let Err(e) = result {
        return failed(site, command, &format!("could not start: {e}"), "", page);
    }
    started();
    let waited = Instant::now();
    let limit = if page { PAGE_WAIT } else { REQUEST_WAIT };
    loop {
        if ports::in_use(site.upstream) {
            return Reply::ready(req);
        }
        let state = commands.state(&site.name, &command.name);
        match state["state"].as_str() {
            Some("exited") => return failed(site, command, &exit_text(&state), &log_tail(paths, site, command), page),
            Some("stopped") => {
                return Reply::text(
                    503,
                    format!("{} · {} was stopped while a request waited for it.\n", site.name, command.name),
                );
            }
            _ => {}
        }
        if waited.elapsed() >= limit {
            let since = state["since"].as_u64().unwrap_or_else(now_secs);
            return starting(site, command, now_secs().saturating_sub(since), &log_tail(paths, site, command), page);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// "exited with code 1" or "was ended by signal 9".
fn exit_text(state: &Value) -> String {
    match (state["code"].as_i64(), state["signal"].as_i64()) {
        (Some(code), _) => format!("exited with code {code}"),
        (None, Some(signal)) => format!("was ended by signal {signal}"),
        _ => "exited".into(),
    }
}

/// The last lines of a command's log, without terminal escapes, each line as its last redraw.
fn log_tail(paths: &Paths, site: &Site, command: &AppCommand) -> String {
    let path = paths.log_dir().join(commands::log_name(&site.name, &command.name));
    let mut text = Vec::new();
    if let Ok(mut file) = fs::File::open(&path) {
        let len = file.metadata().map_or(0, |m| m.len());
        let _ = file.seek(SeekFrom::Start(len.saturating_sub(8 * 1024)));
        let _ = file.read_to_end(&mut text);
    }
    let text = logs::strip_escapes(&String::from_utf8_lossy(&text));
    let lines: Vec<&str> = text.lines().map(|l| l.rsplit('\r').next().unwrap_or(l)).collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}

/// Text that is safe in HTML.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// A small page in the system font, light or dark, that reloads itself every second if `reload`.
fn page_html(title: &str, paragraphs: &[String], tail: &str, reload: bool) -> String {
    let refresh = if reload { "<meta http-equiv=\"refresh\" content=\"1\">\n" } else { "" };
    let mut body = paragraphs.iter().map(|p| format!("<p>{p}</p>\n")).collect::<String>();
    if !tail.is_empty() {
        body.push_str(&format!("<pre>{}</pre>\n", esc(tail)));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n{refresh}\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n\
         <style>\n\
         body {{ font: 15px/1.5 -apple-system, system-ui, sans-serif; max-width: 46rem; margin: 2.5rem auto; padding: 0 1rem; color: #1d1d1f; background: #fff; }}\n\
         h1 {{ font-size: 1.3rem; font-weight: 600; }}\n\
         code {{ font: 13px ui-monospace, Menlo, monospace; }}\n\
         pre {{ font: 12px/1.45 ui-monospace, Menlo, monospace; background: #f2f2f4; padding: .75rem 1rem; border-radius: 8px; white-space: pre-wrap; word-break: break-word; }}\n\
         @media (prefers-color-scheme: dark) {{ body {{ color: #e8e8ed; background: #1c1c1e; }} pre {{ background: #2c2c2e; }} }}\n\
         </style>\n</head>\n<body>\n<h1>{title}</h1>\n{body}</body>\n</html>\n",
        title = esc(title),
    )
}

/// The command still starts: a page that reloads itself (503) for a page load, else a 504.
fn starting(site: &Site, command: &AppCommand, secs: u64, tail: &str, page: bool) -> Reply {
    let log = commands::log_name(&site.name, &command.name);
    if !page {
        return Reply::text(
            504,
            format!(
                "{}: {} has been starting for {secs} s and does not answer on port {} yet. Its log: `devid logs {log} -f`.\n\n{tail}\n",
                site.host, command.name, site.upstream
            ),
        );
    }
    let paragraphs = [
        format!(
            "Devid started <code>{}</code> (<code>{}</code>) because this site was opened. It has been starting for {secs} s; this page reloads until the dev server answers on port {}.",
            esc(&command.name),
            esc(&command.run),
            site.upstream
        ),
        format!("The end of its log (<code>devid logs {}</code>):", esc(&log)),
    ];
    Reply::html(503, page_html(&format!("Starting {}…", site.host), &paragraphs, tail, true))
}

/// The command could not start or exited before its port accepted: 500, with its log's end.
fn failed(site: &Site, command: &AppCommand, what: &str, tail: &str, page: bool) -> Reply {
    let log = commands::log_name(&site.name, &command.name);
    if !page {
        return Reply::text(
            500,
            format!(
                "{}: {} {what} before it answered on port {}. Its log: `devid logs {log}`.\n\n{tail}\n",
                site.host, command.name, site.upstream
            ),
        );
    }
    let paragraphs = [
        format!(
            "Devid started <code>{}</code> (<code>{}</code>) for this site, and it {} before it answered on port {}. Reload this page to start it again.",
            esc(&command.name),
            esc(&command.run),
            esc(what),
            site.upstream
        ),
        format!("The end of its log (<code>devid logs {}</code>):", esc(&log)),
    ];
    Reply::html(500, page_html(&format!("{} stopped", command.name), &paragraphs, tail, false))
}

/// One idle check, every `IDLE_CHECK`: an on-demand command with an open connection counts as
/// used, and one whose site has been idle for its `idle_stop` minutes is stopped (it sleeps until
/// the next request). Returns whether an on-demand command still runs.
pub fn tick(od: &OnDemand, commands: &Commands, paths: &Paths, sites: &[Site], now: SystemTime) -> bool {
    let mut running = false;
    for site in sites {
        let Some(name) = &site.on_demand else { continue };
        let Some((pgid, since)) = commands.running_group(&site.name, name) else { continue };
        trim_requests(paths, &site.name);
        if crate::platform::established(pgid, site.upstream) {
            od.mark_used(&site.name, now);
            running = true;
            continue;
        }
        let limit = Duration::from_secs(u64::from(site.idle_stop) * 60);
        let idle = |at: SystemTime| at.duration_since(od.last_use(paths, &site.name, since)).unwrap_or_default();
        if site.idle_stop == 0 || idle(now) < limit {
            running = true;
            continue;
        }
        // Wake-ups wait while it stops, then start it afresh: one that came in since the check
        // above (it marks the site used first) keeps it running instead, so a request is never
        // told the dev server accepts just before it is stopped.
        let _one = od.start.lock().unwrap();
        if idle(SystemTime::now()) < limit {
            running = true;
            continue;
        }
        let line = format!(
            "stopped after {} min without requests or open connections; the next request to {} starts it again",
            site.idle_stop, site.host
        );
        let log = paths.log_dir().join(commands::log_name(&site.name, name));
        if let Ok(mut f) = OpenOptions::new().append(true).open(log) {
            let _ = writeln!(f, "devid: {line}");
        }
        eprintln!("devid: {} · {name} {line}", site.name);
        commands.stop(&site.name, Some(name));
    }
    running
}

/// Empty a site's request file past REQUESTS_MAX, keeping its modification time (the last
/// request). The web server appends (O_APPEND), so it carries on at the new end.
fn trim_requests(paths: &Paths, site: &str) {
    let path = paths.last_request(site);
    let Ok(meta) = fs::metadata(&path) else { return };
    if meta.len() <= REQUESTS_MAX {
        return;
    }
    if let (Ok(file), Ok(modified)) = (OpenOptions::new().write(true).open(&path), meta.modified()) {
        let _ = file.set_len(0).and_then(|_| file.set_modified(modified));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wake_request(site: &str, page: bool, head: bool) -> WakeRequest {
        WakeRequest { site: site.into(), page, head, redirect: None, body: Body::None }
    }

    #[test]
    fn wake_requests() {
        let page = "GET /docs HTTP/1.0\r\nHost: localhost\r\nX-Devid-Site: devid-site\r\n\
                    Accept: text/html,application/xhtml+xml;q=0.9\r\n\r\n";
        assert_eq!(parse(page), Some(wake_request("devid-site", true, false)));
        let fetch = "POST /api HTTP/1.0\r\nx-devid-site: shop\r\naccept: text/html\r\n\r\n";
        assert_eq!(parse(fetch), Some(wake_request("shop", false, false)), "only a GET");
        let asset = "GET /_next/x.js HTTP/1.0\r\nX-Devid-Site: shop\r\nAccept: */*\r\n\r\n";
        assert!(!parse(asset).unwrap().page);
        assert!(parse("HEAD / HTTP/1.0\r\nX-Devid-Site: shop\r\n\r\n").unwrap().head);
        assert_eq!(parse("GET / HTTP/1.0\r\nHost: x\r\n\r\n"), None, "no site");

        // Caddy: a GET, with the original method beside it.
        let caddy = "GET /login HTTP/1.1\r\nX-Devid-Site: shop\r\nX-Devid-Method: POST\r\nAccept: text/html\r\n\r\n";
        assert_eq!(parse(caddy), Some(wake_request("shop", false, false)), "a form post is not a page load");
        // A HEAD through Caddy still asks with a GET, which expects the answer's body.
        let head = parse("GET / HTTP/1.1\r\nX-Devid-Site: shop\r\nX-Devid-Method: HEAD\r\nAccept: text/html\r\n\r\n");
        assert_eq!(head, Some(wake_request("shop", false, false)), "neither a page load nor a bodiless answer");

        // Apache: the original request line, and maybe a body.
        let apache = "GET / HTTP/1.1\r\nX-Devid-Site: shop\r\nAccept: text/html\r\n\
                      X-Devid-Request: GET /a/b%20c?x=1&next=%2F HTTP/2.0\r\nContent-Length: 12\r\n\r\n";
        let got = parse(apache).unwrap();
        assert!(got.page);
        assert_eq!(got.redirect.as_deref(), Some("/a/b%20c?x=1&next=%2F"));
        assert_eq!(got.body, Body::Length(12));
        let post = "GET / HTTP/1.1\r\nX-Devid-Site: shop\r\nX-Devid-Request: POST /form HTTP/1.1\r\n\
                    Content-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n";
        let got = parse(post).unwrap();
        assert_eq!((got.page, got.redirect.as_deref(), got.body), (false, Some("/form"), Body::Chunked));
        for odd in ["//evil.example/", "http://evil.example/", "*", "/a\u{7f}b"] {
            assert_eq!(redirect_target(odd), "/", "{odd}");
        }
    }

    /// Send `raw` as the web server would and read it as the daemon does; then send "NEXT", which
    /// must be all that is left: the request was read to its end, and no further.
    fn read_raw(raw: &[u8]) -> (Option<WakeRequest>, Vec<u8>) {
        let (mut server, mut daemon) = UnixStream::pair().unwrap();
        let (done, read) = std::sync::mpsc::channel::<()>();
        let raw = raw.to_vec();
        let writer = thread::spawn(move || {
            server.write_all(&raw).unwrap();
            read.recv().unwrap();
            server.write_all(b"NEXT").unwrap();
        });
        let req = read_request(&mut daemon).unwrap();
        done.send(()).unwrap();
        writer.join().unwrap();
        let mut rest = Vec::new();
        daemon.read_to_end(&mut rest).unwrap();
        (req, rest)
    }

    #[test]
    fn bodies_are_read_and_dropped() {
        let head = "GET / HTTP/1.1\r\nX-Devid-Site: shop\r\nX-Devid-Request: POST /f HTTP/1.1\r\n";
        let body = vec![b'x'; 50_000];
        let mut raw = format!("{head}Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        raw.extend_from_slice(&body);
        let (req, rest) = read_raw(&raw);
        assert_eq!(req.unwrap().redirect.as_deref(), Some("/f"));
        assert_eq!(rest, b"NEXT", "the whole body, nothing more");

        let chunked = format!(
            "{head}Transfer-Encoding: chunked\r\n\r\n5;ext=1\r\nhello\r\n1A\r\n{}\r\n0\r\nX-T: 1\r\n\r\n",
            "y".repeat(26)
        );
        let (req, rest) = read_raw(chunked.as_bytes());
        assert_eq!(req.unwrap().body, Body::Chunked);
        assert_eq!(rest, b"NEXT");

        let (req, rest) = read_raw(b"GET / HTTP/1.1\r\nX-Devid-Site: shop\r\n\r\n");
        assert_eq!(req.unwrap().body, Body::None);
        assert_eq!(rest, b"NEXT", "no body, nothing read");
    }

    #[test]
    fn replies() {
        let ready = String::from_utf8(Reply::ready(&wake_request("shop", true, false)).bytes(false)).unwrap();
        assert!(
            ready.starts_with("HTTP/1.1 418 I'm a teapot\r\n") && ready.contains("Content-Length: 0\r\n"),
            "{ready}"
        );
        assert!(ready.ends_with("Connection: close\r\n\r\n") && !ready.contains("Location"));
        let apache = WakeRequest { redirect: Some("/f?x=1".into()), ..wake_request("shop", false, false) };
        let again = String::from_utf8(Reply::ready(&apache).bytes(false)).unwrap();
        assert!(again.starts_with("HTTP/1.1 307 Temporary Redirect\r\n"), "{again}");
        assert!(again.contains("Location: /f?x=1\r\n") && again.contains("Cache-Control: no-store\r\n"));

        let site =
            Site { name: "shop".into(), host: "shop.test".into(), upstream: 4321, idle_stop: 10, ..Site::default() };
        let command = AppCommand { name: "dev".into(), run: "npm run dev -- --port \"$PORT\" <x>".into() };
        let page = starting(&site, &command, 7, "ready <in> 2s", true);
        assert_eq!((page.code, page.html), (503, true));
        assert!(page.body.contains("<meta http-equiv=\"refresh\" content=\"1\">"), "reloads itself");
        assert!(page.body.contains("<title>Starting shop.test…</title>") && page.body.contains("starting for 7 s"));
        assert!(
            page.body.contains("--port &quot;$PORT&quot; &lt;x&gt;") && page.body.contains("ready &lt;in&gt; 2s"),
            "escaped"
        );
        let raw = String::from_utf8(page.bytes(false)).unwrap();
        assert!(raw.contains("Retry-After: 1\r\n") && raw.contains("Cache-Control: no-store\r\n"));
        assert!(String::from_utf8(page.bytes(true)).unwrap().ends_with("\r\n\r\n"), "HEAD: no body");

        let api = starting(&site, &command, 60, "", false);
        assert_eq!((api.code, api.html), (504, false));
        assert!(api.body.contains("does not answer on port 4321 yet"), "{}", api.body);

        let crashed = json_exit(Some(1), None);
        let gone = failed(&site, &command, &exit_text(&crashed), "Error: EADDRINUSE", true);
        assert_eq!(gone.code, 500);
        assert!(gone.body.contains("exited with code 1") && gone.body.contains("EADDRINUSE"));
        assert!(!gone.body.contains("http-equiv"), "a failure does not reload by itself");
        assert_eq!(exit_text(&json_exit(None, Some(9))), "was ended by signal 9");
        assert_eq!(Reply::not_on_demand("shop").code, 503, "the daemon never answers 502");
    }

    fn json_exit(code: Option<i32>, signal: Option<i32>) -> Value {
        serde_json::json!({"state": "exited", "code": code, "signal": signal, "at": 1})
    }

    #[test]
    fn log_tail_and_request_file() {
        let home = std::env::temp_dir().join(format!("devid-ondemand-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let paths = Paths { home: home.clone() };
        fs::create_dir_all(paths.log_dir()).unwrap();
        fs::create_dir_all(paths.run_dir()).unwrap();
        let site = Site { name: "shop".into(), ..Site::default() };
        let command = AppCommand { name: "dev".into(), run: "x".into() };
        let lines: String = (1..=30).map(|i| format!("\x1b[32mline {i}\x1b[0m\n")).collect();
        fs::write(paths.log_dir().join("app-shop-dev.log"), lines + "50%\r75%\r100%\n").unwrap();
        let tail = log_tail(&paths, &site, &command);
        assert_eq!(tail.lines().count(), TAIL_LINES);
        assert!(tail.starts_with("line 12\n") && tail.ends_with("line 30\n100%"), "{tail:?}");

        let od = OnDemand::default();
        let requests = paths.last_request("shop");
        fs::write(&requests, "1.0\n".repeat(20_000)).unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(&requests).unwrap().set_modified(old).unwrap();
        trim_requests(&paths, "shop");
        assert_eq!(fs::metadata(&requests).unwrap().len(), 0, "emptied past 64 KB");
        assert_eq!(od.last_use(&paths, "shop", 0), fs::metadata(&requests).unwrap().modified().unwrap());
        assert!(od.last_use(&paths, "shop", 0) < SystemTime::now() - Duration::from_secs(3000), "keeps its time");
        let now = SystemTime::now();
        od.mark_used("shop", now);
        assert_eq!(od.last_use(&paths, "shop", 0), now, "a wake-up counts");
        fs::remove_dir_all(&home).unwrap();
    }
}
