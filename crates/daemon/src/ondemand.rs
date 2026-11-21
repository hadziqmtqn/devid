//! Dev servers on demand (D-054, Phase 10): a request to a proxy site whose dev server is down
//! starts the command the owner marked for it, and the command stops again once the site has been
//! idle for its `idle_stop` minutes.
//!
//! The web server asks over `run/wake.sock` (see `nginx::site_body`): one HTTP/1.x request per
//! connection, naming the site in `X-Devid-Site`, without a body. The daemon starts the command,
//! waits until its port accepts and answers 418, which the web server turns into the same request
//! to the dev server, method and body included. A page load still starting after 5 s gets a page
//! that reloads itself; any other request waits up to 60 s. The daemon never answers 502: the web
//! server answers that when it cannot reach the wake socket, as when the daemon is not running.

use crate::{
    commands::{self, Commands},
    config::{AppCommand, Config},
    logs,
    paths::Paths,
    ports,
    sites::Site,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
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

/// On-demand works where the web server can ask the daemon: nginx so far (Caddy and Apache follow
/// in Phase 10 O4). Elsewhere the setting stays, and the site waits for its command as before.
pub fn available(config: &Config) -> bool {
    config.server == "nginx"
}

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
    /// A GET that accepts `text/html`: it may get the Starting page.
    pub page: bool,
    /// A HEAD request gets the status and headers only.
    pub head: bool,
}

/// The request line and headers of a wake request; None without `X-Devid-Site`.
pub fn parse(head: &str) -> Option<WakeRequest> {
    let mut lines = head.split("\r\n");
    let method = lines.next()?.split(' ').next()?.to_string();
    let (mut site, mut accept) = (None, String::new());
    for line in lines {
        let Some((key, value)) = line.split_once(':') else { continue };
        match key.trim().to_ascii_lowercase().as_str() {
            "x-devid-site" => site = Some(value.trim().to_string()),
            "accept" => accept = value.trim().to_ascii_lowercase(),
            _ => {}
        }
    }
    Some(WakeRequest { site: site?, page: method == "GET" && accept.contains("text/html"), head: method == "HEAD" })
}

/// Read one wake request's header from the web server; None for anything else.
pub fn read_request(stream: &mut UnixStream) -> io::Result<Option<WakeRequest>> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        if buf.len() > HEADER_MAX {
            return Ok(None);
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(parse(&String::from_utf8_lossy(&buf)))
}

/// The daemon's answer to a wake request.
#[derive(Debug, PartialEq)]
pub struct Reply {
    pub code: u16,
    pub html: bool,
    pub body: String,
}

impl Reply {
    /// 418: the dev server accepts now; the web server sends it the request.
    fn ready() -> Self {
        Self { code: 418, html: false, body: String::new() }
    }

    fn text(code: u16, body: String) -> Self {
        Self { code, html: false, body }
    }

    /// The site is not (or no longer) on demand, or the web server is not one that wakes it.
    pub fn not_on_demand(site: &str) -> Self {
        Self::text(503, format!("{site} is not on demand in Devid; start its dev server (or its command in Devid).\n"))
    }

    /// The HTTP response, without its body for a HEAD request.
    pub fn bytes(&self, head: bool) -> Vec<u8> {
        let reason = match self.code {
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
    page: bool,
    started: impl FnOnce(),
) -> Reply {
    od.mark_used(&site.name, SystemTime::now());
    let note = match site.idle_stop {
        0 => format!("started by a request to {} (on demand)", site.host),
        m => format!("started by a request to {} (on demand; it stops after {m} min idle)", site.host),
    };
    let result = {
        let _one = od.start.lock().unwrap();
        commands.start(&site.name, &command.name, &command.run, &site.path, site.dev_port(), Some(&note))
    };
    if let Err(e) = result {
        return failed(site, command, &format!("could not start: {e}"), "", page);
    }
    started();
    let waited = Instant::now();
    let limit = if page { PAGE_WAIT } else { REQUEST_WAIT };
    loop {
        if ports::in_use(site.upstream) {
            return Reply::ready();
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
    Reply { code: 503, html: true, body: page_html(&format!("Starting {}…", site.host), &paragraphs, tail, true) }
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
    Reply { code: 500, html: true, body: page_html(&format!("{} stopped", command.name), &paragraphs, tail, false) }
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
        if established(pgid, site.upstream) {
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

/// Whether a process of group `pgid` has an established TCP connection on its local port `port`:
/// a request in progress, or an open WebSocket such as a dev server's HMR, which an open tab
/// keeps. Read through libproc, without starting a process.
#[cfg(target_os = "macos")]
pub fn established(pgid: i32, port: u16) -> bool {
    libproc::group_pids(pgid).into_iter().any(|pid| {
        libproc::fds(pid)
            .iter()
            .filter(|fd| fd.proc_fdtype == libc::PROX_FDTYPE_SOCKET as u32)
            .any(|fd| libproc::tcp_established_on(pid, fd.proc_fd, port))
    })
}

/// Other systems have no libproc; nothing counts as an open connection there.
#[cfg(not(target_os = "macos"))]
pub fn established(_pgid: i32, _port: u16) -> bool {
    false
}

#[cfg(target_os = "macos")]
mod libproc {
    use std::{mem::size_of, ptr::null_mut};

    // From <sys/proc_info.h>, the same on arm64 and x86_64 (offsetof checked with clang for both,
    // 2026-11-21). libc has no `socket_fdinfo`, so the few fields needed are read by offset.
    const PROC_PIDFDSOCKETINFO: i32 = 3;
    const SOCKET_FDINFO_SIZE: usize = 792;
    /// psi.soi_kind
    const SOI_KIND: usize = 256;
    /// psi.soi_proto.pri_tcp.tcpsi_ini.insi_lport, in network byte order
    const INSI_LPORT: usize = 268;
    /// psi.soi_proto.pri_tcp.tcpsi_state
    const TCPSI_STATE: usize = 344;
    const SOCKINFO_TCP: i32 = 2;
    const TSI_S_ESTABLISHED: i32 = 4;

    /// The processes of a process group.
    pub fn group_pids(pgid: i32) -> Vec<i32> {
        let mut capacity = 64;
        loop {
            let mut pids = vec![0i32; capacity];
            let bytes = (capacity * size_of::<i32>()) as i32;
            // SAFETY: the buffer holds `bytes` bytes; the call returns how many pids it wrote.
            let n = unsafe { libc::proc_listpgrppids(pgid, pids.as_mut_ptr().cast(), bytes) };
            let Ok(n) = usize::try_from(n) else { return Vec::new() };
            if n < capacity || capacity >= 16_384 {
                pids.truncate(n.min(capacity));
                return pids;
            }
            capacity *= 4;
        }
    }

    /// A process's open file descriptors.
    pub fn fds(pid: i32) -> Vec<libc::proc_fdinfo> {
        // SAFETY: a null buffer asks for the size needed.
        let needed = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, null_mut(), 0) };
        let Ok(needed) = usize::try_from(needed) else { return Vec::new() };
        // Room for a few descriptors opened since.
        let count = needed / size_of::<libc::proc_fdinfo>() + 16;
        // SAFETY: proc_fdinfo is two integers; all zeroes is a valid value.
        let mut fds: Vec<libc::proc_fdinfo> = (0..count).map(|_| unsafe { std::mem::zeroed() }).collect();
        let bytes = (count * size_of::<libc::proc_fdinfo>()) as i32;
        // SAFETY: the buffer holds `bytes` bytes; the call returns how many it wrote.
        let got = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fds.as_mut_ptr().cast(), bytes) };
        let Ok(got) = usize::try_from(got) else { return Vec::new() };
        fds.truncate(got / size_of::<libc::proc_fdinfo>());
        fds
    }

    /// Whether `fd` of `pid` is an established TCP connection on local port `port`.
    pub fn tcp_established_on(pid: i32, fd: i32, port: u16) -> bool {
        let mut info = [0u8; SOCKET_FDINFO_SIZE];
        // SAFETY: the buffer holds a whole socket_fdinfo; the call returns how many bytes it wrote.
        let got = unsafe {
            libc::proc_pidfdinfo(pid, fd, PROC_PIDFDSOCKETINFO, info.as_mut_ptr().cast(), SOCKET_FDINFO_SIZE as i32)
        };
        if usize::try_from(got) != Ok(SOCKET_FDINFO_SIZE) {
            return false;
        }
        let int = |at: usize| i32::from_ne_bytes(info[at..at + 4].try_into().expect("four bytes"));
        int(SOI_KIND) == SOCKINFO_TCP
            && int(TCPSI_STATE) == TSI_S_ESTABLISHED
            && u16::from_be(int(INSI_LPORT) as u16) == port
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_requests() {
        let page = "GET /docs HTTP/1.0\r\nHost: localhost\r\nX-Devid-Site: devid-site\r\n\
                    Accept: text/html,application/xhtml+xml;q=0.9\r\n\r\n";
        assert_eq!(parse(page), Some(WakeRequest { site: "devid-site".into(), page: true, head: false }));
        let fetch = "POST /api HTTP/1.0\r\nx-devid-site: shop\r\naccept: text/html\r\n\r\n";
        assert_eq!(parse(fetch), Some(WakeRequest { site: "shop".into(), page: false, head: false }), "only a GET");
        let asset = "GET /_next/x.js HTTP/1.0\r\nX-Devid-Site: shop\r\nAccept: */*\r\n\r\n";
        assert!(!parse(asset).unwrap().page);
        assert!(parse("HEAD / HTTP/1.0\r\nX-Devid-Site: shop\r\n\r\n").unwrap().head);
        assert_eq!(parse("GET / HTTP/1.0\r\nHost: x\r\n\r\n"), None, "no site");
    }

    #[test]
    fn replies() {
        let ready = String::from_utf8(Reply::ready().bytes(false)).unwrap();
        assert!(
            ready.starts_with("HTTP/1.1 418 I'm a teapot\r\n") && ready.contains("Content-Length: 0\r\n"),
            "{ready}"
        );
        assert!(ready.ends_with("Connection: close\r\n\r\n"));

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

    #[cfg(target_os = "macos")]
    #[test]
    fn an_open_connection_is_seen() {
        use std::net::{TcpListener, TcpStream};
        // SAFETY: getpgrp(2) has no preconditions.
        let group = unsafe { libc::getpgrp() };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!established(group, port), "listening only");
        let client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let (server, _) = listener.accept().unwrap();
        assert!(established(group, port), "the accepted side has local port {port}");
        let other = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(!established(group, other));
        drop((client, server));
        assert!(!established(group, port), "closed");
        assert!(!established(i32::MAX, port), "no such group");
    }
}
