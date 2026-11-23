//! Mail (D-049): the daemon runs the official Mailpit build, so the mail that apps send lands in an
//! inbox on this Mac. Off by default (`[mail]` in config.toml). Mailpit listens on 127.0.0.1 only:
//! SMTP on `port`, its inbox page and API on the next port.

use crate::{config::Mail, paths::Paths, php, supervisor::Spec, webserver};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The supervisor's name for Mailpit, also what `devid stop|start|restart` take.
pub const SERVICE: &str = "mail";

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

pub fn dir(paths: &Paths) -> PathBuf {
    paths.home.join("mailpit")
}

pub fn bin(paths: &Paths) -> PathBuf {
    dir(paths).join("bin/mailpit")
}

/// The version `install` found, so `status` need not run Mailpit to ask.
fn version_file(paths: &Paths) -> PathBuf {
    dir(paths).join("version")
}

/// The mail itself: Mailpit's database.
pub fn data(paths: &Paths) -> PathBuf {
    paths.home.join("mail")
}

pub fn log(paths: &Paths) -> PathBuf {
    paths.log_dir().join("mailpit.log")
}

pub fn installed(paths: &Paths) -> bool {
    bin(paths).is_file()
}

/// The installed Mailpit's version ("1.33.1"), as `install` recorded it.
pub fn version(paths: &Paths) -> Option<String> {
    fs::read_to_string(version_file(paths)).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// "1.33.1" from `mailpit version`, whose first word is the path it was started with:
/// "/Users/x/.devid/mailpit/bin/mailpit v1.33.1 compiled with go1.27.3 on darwin/amd64".
fn parse_version(text: &str) -> Option<String> {
    let mut words = text.split_whitespace();
    if !words.next()?.ends_with("mailpit") {
        return None;
    }
    words.next()?.strip_prefix('v').map(String::from)
}

/// Put a Mailpit binary in place (the release archive's `mailpit`) and record its version.
/// `--no-release-check`: plain `mailpit version` also asks GitHub for Mailpit's latest release, so
/// it needs the network and can outlast the probe's 5 s (a Linux release build failed on it).
pub fn install(paths: &Paths, from: &Path) -> io::Result<String> {
    let out = webserver::probe(from, &["version", "--no-release-check"])?;
    let version = parse_version(&String::from_utf8_lossy(&out.stdout))
        .ok_or_else(|| err(format!("{} does not look like Mailpit", from.display())))?;
    let target = bin(paths);
    fs::create_dir_all(target.parent().expect("bin has a parent"))?;
    let tmp = target.with_extension("new");
    fs::copy(from, &tmp)?;
    fs::rename(tmp, &target)?;
    fs::write(version_file(paths), format!("{version}\n"))?;
    Ok(version)
}

/// The inbox (web page and API) listens on the port after SMTP's.
pub fn inbox_port(mail: &Mail) -> u16 {
    mail.port.saturating_add(1)
}

pub fn inbox_url(mail: &Mail) -> String {
    format!("http://127.0.0.1:{}/", inbox_port(mail))
}

/// How the supervisor runs Mailpit: on 127.0.0.1 only, answering only its own Host header (so a web
/// page cannot read the mail through DNS rebinding), with no update check and no relay.
pub fn spec(paths: &Paths, mail: &Mail) -> Spec {
    let (smtp, inbox) = (mail.port, inbox_port(mail));
    let args: Vec<OsString> = vec![
        "--smtp".into(),
        format!("127.0.0.1:{smtp}").into(),
        "--listen".into(),
        format!("127.0.0.1:{inbox}").into(),
        "--allowed-hosts".into(),
        format!("127.0.0.1:{inbox},localhost:{inbox}").into(),
        "--database".into(),
        data(paths).join("mailpit.db").into(),
        "--max".into(),
        mail.keep.to_string().into(),
        "--log-file".into(),
        log(paths).into(),
        "--disable-version-check".into(),
        "--smtp-disable-rdns".into(),
        // Laravel logs in when MAIL_USERNAME is set: any login is accepted, without TLS.
        "--smtp-auth-accept-any".into(),
        "--smtp-auth-allow-insecure".into(),
    ];
    Spec {
        program: bin(paths),
        args,
        env: Vec::new(),
        pid_file: paths.run_dir().join("mailpit.pid"),
        write_pid: true,
        // Only Devid's copy: the owner may run a Mailpit of their own on other ports (D-049).
        comm: "mailpit/bin/mailpit",
        stop_signal: libc::SIGTERM,
    }
}

/// Why Mail cannot use SMTP port `port` and the inbox port after it: a port below 1024, the last
/// port, or one in `taken` (Devid's own ports and the sites'). A port another program listens on is
/// checked by the caller, which knows whether that program is this Mailpit.
pub fn check_port(port: u16, taken: &[(u16, String)]) -> Result<(), String> {
    if port < 1024 || port == u16::MAX {
        return Err(format!("{port}: use a port from 1024 to 65534 (the inbox takes the next one)"));
    }
    for p in [port, port + 1] {
        if let Some((_, what)) = taken.iter().find(|(t, _)| *t == p) {
            return Err(format!("port {p} is {what}"));
        }
    }
    Ok(())
}

fn php_ini(paths: &Paths, version: &str) -> PathBuf {
    php::etc(paths, version).join("conf.d").join("devid-mail.ini")
}

/// What devid-mail.ini says: PHP's mail() pipes each message to Mailpit's sendmail mode.
fn php_ini_body(paths: &Paths, port: u16) -> String {
    // shortcut: a home folder whose path has a ' breaks this shell quoting; quote properly if one shows up.
    format!(
        "; Generated by Devid while Mail is on (`devid mail php-mail off` removes it)\n\
         sendmail_path = \"'{}' sendmail -S 127.0.0.1:{port}\"\n",
        bin(paths).display()
    )
}

/// Make every installed PHP's devid-mail.ini match `port`: Some while Mail runs with PHP's mail(),
/// None for no file. Returns the versions whose file changed; their php-fpm restarts to read it.
/// The CLI shim reads the same conf.d, so `php artisan tinker` follows at once.
pub fn sync_php(paths: &Paths, port: Option<u16>) -> io::Result<Vec<String>> {
    let want = port.map(|p| php_ini_body(paths, p));
    let mut changed = Vec::new();
    for version in php::installed(paths) {
        let ini = php_ini(paths, &version);
        if fs::read_to_string(&ini).ok() == want {
            continue;
        }
        match &want {
            Some(body) => {
                fs::create_dir_all(ini.parent().expect("conf.d"))?;
                fs::write(&ini, body)?;
            }
            None => match fs::remove_file(&ini) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            },
        }
        changed.push(version);
    }
    Ok(changed)
}

/// One request to Mailpit's API on 127.0.0.1:`port`; the body of a 2xx reply. HTTP/1.0, so the
/// reply is never chunked and ends when Mailpit closes the connection.
fn api(port: u16, method: &str, path: &str, body: &str) -> io::Result<Vec<u8>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    )?;
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply)?;
    reply_body(&reply)
}

fn reply_body(reply: &[u8]) -> io::Result<Vec<u8>> {
    let split =
        reply.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| err("Mailpit sent an incomplete reply"))?;
    let (head, body) = (String::from_utf8_lossy(&reply[..split]), &reply[split + 4..]);
    let status = head.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok());
    match status {
        Some(200..=299) => Ok(body.to_vec()),
        Some(code) => Err(err(format!("Mailpit answered {code}: {}", String::from_utf8_lossy(body).trim()))),
        None => Err(err("Mailpit's reply has no status")),
    }
}

fn api_json(mail: &Mail, method: &str, path: &str, body: &str) -> io::Result<Value> {
    serde_json::from_slice(&api(inbox_port(mail), method, path, body)?)
        .map_err(|e| err(format!("Mailpit's reply: {e}")))
}

/// The message and unread counts and the newest message, from Mailpit's message list; None if it
/// does not answer.
pub fn summary(mail: &Mail) -> Option<Value> {
    api_json(mail, "GET", "/api/v1/messages?limit=1", "").ok()
}

/// The newest message of a `summary` as `status` reports it (D-050 notifies with it): when Mailpit
/// received it, the sender's name (or address) and the subject.
pub fn latest(summary: &Value) -> Option<Value> {
    let m = summary["messages"].get(0)?;
    Some(json!({"created": m["Created"], "from": sender(&m["From"]), "subject": m["Subject"]}))
}

/// A sender's name, or the address when there is none.
fn sender(from: &Value) -> &str {
    from["Name"].as_str().filter(|n| !n.is_empty()).or(from["Address"].as_str()).unwrap_or("")
}

/// Delete every message (no IDs given means all of them).
pub fn clear(mail: &Mail) -> io::Result<()> {
    api(inbox_port(mail), "DELETE", "/api/v1/messages", "{}").map(|_| ())
}

// ---------- the inbox (Phase 9 M6): what the app's inbox and `devid mail list|show` read ----------

/// Mailpit's message IDs (22 letters and digits); anything else could reach another API path.
fn check_id(id: &str) -> io::Result<()> {
    match !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        true => Ok(()),
        false => Err(err(format!("{id:?} is not a message ID"))),
    }
}

/// A query string value: everything but letters, digits and -._~ percent-encoded.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// One line of the list, as the clients show it.
fn listed(m: &Value) -> Value {
    let to: Vec<&str> = m["To"].as_array().into_iter().flatten().filter_map(|t| t["Address"].as_str()).collect();
    json!({"id": m["ID"], "from": sender(&m["From"]), "from_address": m["From"]["Address"], "to": to,
           "subject": m["Subject"], "snippet": m["Snippet"], "created": m["Created"], "read": m["Read"],
           "attachments": m["Attachments"], "size": m["Size"]})
}

/// The newest `limit` messages, or those matching `query` (Mailpit's search: words, from:, to:,
/// subject:, has:attachment…); `unread` keeps only unread ones. `count` is how many match.
pub fn list(mail: &Mail, query: &str, unread: bool, limit: u32) -> io::Result<Value> {
    let query = [if unread { "is:unread" } else { "" }, query.trim()].join(" ").trim().to_string();
    let path = match query.is_empty() {
        true => format!("/api/v1/messages?limit={limit}"),
        false => format!("/api/v1/search?query={}&limit={limit}", encode(&query)),
    };
    let v = api_json(mail, "GET", &path, "")?;
    let messages: Vec<Value> = v["messages"].as_array().into_iter().flatten().map(listed).collect();
    Ok(json!({"total": v["total"], "unread": v["unread"], "count": v["messages_count"], "messages": messages}))
}

/// One message as the clients show it. Mailpit marks it read.
pub fn message(mail: &Mail, id: &str) -> io::Result<Value> {
    check_id(id)?;
    Ok(shown(&api_json(mail, "GET", &format!("/api/v1/message/{id}"), "")?))
}

/// Mailpit's message as the clients show it.
fn shown(m: &Value) -> Value {
    let people = |v: &Value| -> Vec<Value> {
        v.as_array().into_iter().flatten().map(|p| json!({"name": p["Name"], "address": p["Address"]})).collect()
    };
    let attachments: Vec<Value> = m["Attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| json!({"part": a["PartID"], "name": a["FileName"], "type": a["ContentType"], "size": a["Size"]}))
        .collect();
    // Images the HTML shows by `cid:` reference; the app's inbox loads them from Mailpit.
    let inline: Vec<Value> = m["Inline"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| json!({"part": a["PartID"], "cid": a["ContentID"]}))
        .collect();
    json!({"id": m["ID"], "from": {"name": m["From"]["Name"], "address": m["From"]["Address"]},
           "to": people(&m["To"]), "cc": people(&m["Cc"]), "subject": m["Subject"], "date": m["Date"],
           "size": m["Size"], "text": m["Text"], "html": m["HTML"], "attachments": attachments, "inline": inline})
}

/// A message's headers: name -> values.
pub fn headers(mail: &Mail, id: &str) -> io::Result<Value> {
    check_id(id)?;
    api_json(mail, "GET", &format!("/api/v1/message/{id}/headers"), "")
}

/// Shown at most; a raw message with big attachments is cut here.
const RAW_MAX: usize = 512 * 1024;

/// A message's source as it arrived, and whether it was cut at `RAW_MAX`.
pub fn raw(mail: &Mail, id: &str) -> io::Result<(String, bool)> {
    check_id(id)?;
    let bytes = api(inbox_port(mail), "GET", &format!("/api/v1/message/{id}/raw"), "")?;
    let cut = bytes.len() > RAW_MAX;
    Ok((String::from_utf8_lossy(&bytes[..bytes.len().min(RAW_MAX)]).into_owned(), cut))
}

/// Mark messages read or unread; no IDs: all of them.
pub fn mark_read(mail: &Mail, ids: &[String], read: bool) -> io::Result<()> {
    ids.iter().try_for_each(|id| check_id(id))?;
    let body = if ids.is_empty() { json!({"Read": read}) } else { json!({"IDs": ids, "Read": read}) };
    api(inbox_port(mail), "PUT", "/api/v1/messages", &body.to_string()).map(|_| ())
}

/// Delete these messages (`clear` deletes all).
pub fn delete(mail: &Mail, ids: &[String]) -> io::Result<()> {
    if ids.is_empty() {
        return Err(err("no message IDs"));
    }
    ids.iter().try_for_each(|id| check_id(id))?;
    api(inbox_port(mail), "DELETE", "/api/v1/messages", &json!({"IDs": ids}).to_string()).map(|_| ())
}

/// Save attachment `part` of message `id` in `dir` under its own file name, next to a file of the
/// same name rather than over it; returns the path.
pub fn save_part(mail: &Mail, id: &str, part: &str, dir: &Path) -> io::Result<PathBuf> {
    let m = message(mail, id)?;
    let attachment = m["attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["part"] == part)
        .ok_or_else(|| err(format!("message {id} has no attachment {part}")))?;
    let bytes = api(inbox_port(mail), "GET", &format!("/api/v1/message/{id}/part/{part}"), "")?;
    let path = free_path(dir, &file_name(attachment["name"].as_str().unwrap_or("")));
    fs::create_dir_all(dir)?;
    fs::write(&path, bytes)?;
    Ok(path)
}

/// A name a sender chose, made safe as one file name in a folder.
fn file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = base.chars().map(|c| if c.is_control() || c == ':' { '_' } else { c }).collect();
    let clean = clean.trim().trim_start_matches('.').to_string();
    if clean.is_empty() { "attachment".into() } else { clean }
}

/// `dir/name`, or `dir/name (2).ext` and so on when that file exists.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    (1..)
        .map(|n| dir.join(if n == 1 { name.to_string() } else { format!("{stem} ({n}){ext}") }))
        .find(|p| !p.exists())
        .expect("some name is free")
}

/// Send one short message to Mailpit over SMTP, the way an app does.
pub fn send_test(mail: &Mail) -> io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], mail.port));
    // A Mailpit that just started needs a moment (0.2 s in M1) before it listens.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let stream = loop {
        match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100))
            }
            other => break other?,
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    expect(&mut reader, "220")?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for (line, code) in ["EHLO devid.test", "MAIL FROM:<test@devid.test>", "RCPT TO:<test@example.com>", "DATA"]
        .into_iter()
        .zip(["250", "250", "250", "354"])
    {
        write!(writer, "{line}\r\n")?;
        expect(&mut reader, code)?;
    }
    write!(
        writer,
        "From: Devid <test@devid.test>\r\nTo: test@example.com\r\nSubject: Test email from Devid\r\n\
         Date: {date}\r\nMessage-ID: <{now}.{pid}@devid.test>\r\nMIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n\
         If you can read this, Mail works.\r\n\r\n\
         Apps on this computer can send mail to 127.0.0.1:{port}. It lands in this inbox and never reaches a \
         real mailbox.\r\n.\r\n",
        date = rfc2822(now),
        pid = std::process::id(),
        port = mail.port,
    )?;
    expect(&mut reader, "250")?;
    let _ = write!(writer, "QUIT\r\n");
    Ok(())
}

/// Read one SMTP reply, all of its "250-…" lines, and check its code.
fn expect(reader: &mut impl BufRead, code: &str) -> io::Result<()> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(err("Mailpit closed the SMTP connection"));
        }
        if !line.starts_with(code) {
            return Err(err(format!("Mailpit answered {}", line.trim())));
        }
        if line.as_bytes().get(3) != Some(&b'-') {
            return Ok(());
        }
    }
}

/// A mail Date header ("Mon, 16 Nov 2026 12:25:16 +0000") for a Unix time, in UTC.
fn rfc2822(secs: u64) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"]; // 1970-01-01 was a Thursday
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // The civil date of a day count (Howard Hinnant's days_from_civil, inverted).
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{}, {day:02} {} {year} {:02}:{:02}:{:02} +0000",
        DAYS[days.rem_euclid(7) as usize],
        MONTHS[(month - 1) as usize],
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};

    fn temp_paths(name: &str) -> Paths {
        let home = std::env::temp_dir().join(format!("devid-mail-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        Paths { home }
    }

    #[test]
    fn reads_mailpits_version_line() {
        assert_eq!(
            parse_version("mailpit v1.33.1 compiled with go1.27.3 on darwin/amd64\n").as_deref(),
            Some("1.33.1")
        );
        // The real binary starts the line with the path it was run from (found by the smoke test).
        let line = "/tmp/h/downloads/mailpit-1.33.1/mailpit v1.33.1 compiled with go1.27.3 on darwin/amd64\n";
        assert_eq!(parse_version(line).as_deref(), Some("1.33.1"));
        assert_eq!(parse_version("v2.11.9 h1:abc"), None, "Caddy's version line is not Mailpit's");
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn formats_mail_dates() {
        assert_eq!(rfc2822(0), "Thu, 01 Jan 1970 00:00:00 +0000");
        assert_eq!(rfc2822(1_794_831_916), "Mon, 16 Nov 2026 12:25:16 +0000");
        assert_eq!(rfc2822(1_709_251_199), "Thu, 29 Feb 2024 23:59:59 +0000");
        assert_eq!(rfc2822(951_868_800), "Wed, 01 Mar 2000 00:00:00 +0000");
    }

    #[test]
    fn mailpit_listens_on_loopback_and_answers_only_its_own_host() {
        let paths = Paths { home: PathBuf::from("/Users/x/.devid") };
        let spec = spec(&paths, &Mail { port: 2525, keep: 100, ..Mail::default() });
        let args: Vec<String> = spec.args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let value = |flag: &str| args.iter().position(|a| a == flag).map(|i| args[i + 1].as_str());
        assert_eq!(value("--smtp"), Some("127.0.0.1:2525"));
        assert_eq!(value("--listen"), Some("127.0.0.1:2526"));
        assert_eq!(value("--allowed-hosts"), Some("127.0.0.1:2526,localhost:2526"));
        assert_eq!(value("--database"), Some("/Users/x/.devid/mail/mailpit.db"));
        assert_eq!(value("--max"), Some("100"));
        assert!(args.iter().any(|a| a == "--disable-version-check"));
        assert!(spec.write_pid, "Mailpit writes no pid file of its own");
        assert!(spec.program.to_string_lossy().ends_with(spec.comm), "the orphan check matches only Devid's copy");
        assert!(!"/Users/x/.local/bin/mailpit".contains(spec.comm), "the owner's own Mailpit never matches");
        assert_eq!(inbox_url(&Mail::default()), "http://127.0.0.1:2526/");
    }

    #[test]
    fn refuses_ports_it_cannot_have() {
        let taken =
            [(8088, "Devid's web server port".to_string()), (8000, "erdbpro-saas.test's local port".to_string())];
        assert!(check_port(2525, &taken).is_ok());
        assert!(check_port(1023, &taken).unwrap_err().contains("1024 to 65534"));
        assert!(check_port(u16::MAX, &taken).is_err(), "the inbox needs the next port");
        assert_eq!(
            check_port(8087, &taken).unwrap_err(),
            "port 8088 is Devid's web server port",
            "the inbox port counts too"
        );
        assert_eq!(check_port(8000, &taken).unwrap_err(), "port 8000 is erdbpro-saas.test's local port");
    }

    #[test]
    fn php_mail_ini_follows_the_setting() {
        let paths = temp_paths("ini");
        for v in ["8.4", "8.5"] {
            fs::create_dir_all(php::fpm_bin(&paths, v).parent().unwrap()).unwrap();
            fs::write(php::fpm_bin(&paths, v), "").unwrap();
        }
        let installed = php::installed(&paths);
        assert_eq!(installed.len(), 2, "both fake PHP versions count as installed: {installed:?}");
        assert_eq!(sync_php(&paths, Some(2525)).unwrap(), installed, "written for every version");
        let ini = fs::read_to_string(php_ini(&paths, "8.4")).unwrap();
        assert!(
            ini.contains(&format!("sendmail_path = \"'{}' sendmail -S 127.0.0.1:2525\"", bin(&paths).display())),
            "{ini}"
        );
        assert!(sync_php(&paths, Some(2525)).unwrap().is_empty(), "unchanged: no php-fpm restart");
        assert_eq!(sync_php(&paths, Some(2600)).unwrap(), installed, "a new port rewrites it");
        assert_eq!(sync_php(&paths, None).unwrap(), installed);
        assert!(!php_ini(&paths, "8.5").exists());
        assert!(sync_php(&paths, None).unwrap().is_empty());
        fs::remove_dir_all(&paths.home).unwrap();
    }

    #[test]
    fn reports_the_newest_message() {
        let list = serde_json::json!({"total": 2, "unread": 1, "messages": [{"ID": "a", "Created": "2026-11-16T14:00:05Z",
            "From": {"Name": "", "Address": "hello@example.com"}, "Subject": "Reset Password Notification"}]});
        assert_eq!(
            latest(&list).unwrap(),
            serde_json::json!({"created": "2026-11-16T14:00:05Z", "from": "hello@example.com", "subject": "Reset Password Notification"}),
            "no name: the address"
        );
        assert_eq!(latest(&serde_json::json!({"total": 0, "unread": 0, "messages": []})), None);
    }

    #[test]
    fn inbox_ids_queries_and_messages() {
        assert!(check_id("5bH3HIpyY4k9NnbLojFCFc").is_ok());
        for bad in ["", "../info", "a/b", "a?b", "a b", &"x".repeat(65)] {
            assert!(check_id(bad).is_err(), "{bad:?} must not reach the API path");
        }
        assert_eq!(encode("is:unread from:hello@example.com ü"), "is%3Aunread%20from%3Ahello%40example.com%20%C3%BC");
        let item = json!({"ID": "a1", "From": {"Name": "", "Address": "hello@example.com"}, "To": [{"Name": "Budi", "Address": "budi@example.com"}],
            "Subject": "Reset Password Notification", "Snippet": "You are receiving", "Created": "2026-11-16T14:00:05Z", "Read": false,
            "Attachments": 1, "Size": 739});
        let l = listed(&item);
        assert_eq!(
            (l["from"].as_str(), l["to"][0].as_str(), l["read"].as_bool(), l["attachments"].as_u64()),
            (Some("hello@example.com"), Some("budi@example.com"), Some(false), Some(1))
        );
        let full = json!({"ID": "a1", "From": {"Name": "ERD Builder Pro", "Address": "hello@example.com"}, "To": [{"Name": "", "Address": "budi@example.com"}],
            "Cc": null, "Subject": "Invoice", "Date": "2026-11-16T14:00:05+07:00", "Size": 739, "Text": "hi", "HTML": "<p>hi</p>",
            "Attachments": [{"PartID": "2", "FileName": "invoice.pdf", "ContentType": "application/pdf", "Size": 15}],
            "Inline": [{"PartID": "1.2", "FileName": "logo.png", "ContentType": "image/png", "ContentID": "logo@app", "Size": 9}]});
        let m = shown(&full);
        assert_eq!(
            (m["from"]["name"].as_str(), m["to"][0]["address"].as_str(), m["html"].as_str()),
            (Some("ERD Builder Pro"), Some("budi@example.com"), Some("<p>hi</p>"))
        );
        assert_eq!(
            m["attachments"][0],
            json!({"part": "2", "name": "invoice.pdf", "type": "application/pdf", "size": 15})
        );
        assert_eq!(m["cc"], json!([]), "no Cc: an empty list");
        assert_eq!(m["inline"], json!([{"part": "1.2", "cid": "logo@app"}]));
    }

    #[test]
    fn attachments_get_safe_names_and_never_overwrite() {
        assert_eq!(file_name("invoice.pdf"), "invoice.pdf");
        assert_eq!(file_name("../../.ssh/authorized_keys"), "authorized_keys", "only the last part of a path");
        assert_eq!(file_name("..\\evil.bat"), "evil.bat");
        assert_eq!(file_name(".hidden"), "hidden", "no dot files");
        assert_eq!(file_name("a:b\nc.txt"), "a_b_c.txt");
        assert_eq!(file_name(""), "attachment");
        let paths = temp_paths("save");
        let dir = paths.home.join("Downloads");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_path(&dir, "invoice.pdf"), dir.join("invoice.pdf"));
        fs::write(dir.join("invoice.pdf"), "").unwrap();
        assert_eq!(free_path(&dir, "invoice.pdf"), dir.join("invoice (2).pdf"));
        fs::write(dir.join("invoice (2).pdf"), "").unwrap();
        assert_eq!(free_path(&dir, "invoice.pdf"), dir.join("invoice (3).pdf"));
        assert_eq!(free_path(&dir, "README"), dir.join("README"));
        fs::remove_dir_all(&paths.home).unwrap();
    }

    #[test]
    fn reads_only_successful_api_replies() {
        assert_eq!(reply_body(b"HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\n\r\nok").unwrap(), b"ok");
        assert!(reply_body(b"HTTP/1.0 403 Forbidden\r\n\r\nInvalid Host").unwrap_err().to_string().contains("403"));
        assert!(reply_body(b"HTTP/1.0 200 OK\r\n").is_err(), "no blank line: incomplete");
        assert_eq!(reply_body(b"HTTP/1.0 200 OK\r\n\r\n%PDF\xff\x00").unwrap(), b"%PDF\xff\x00", "bytes stay bytes");
    }

    #[test]
    fn sends_a_test_message_over_smtp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            let mut seen = String::new();
            writer.write_all(b"220 fake ESMTP\r\n").unwrap();
            let mut line = String::new();
            let mut in_data = false;
            while reader.read_line(&mut line).unwrap() > 0 {
                seen.push_str(&line);
                let reply: &[u8] = match (in_data, line.trim_end()) {
                    (true, ".") => {
                        in_data = false;
                        b"250 2.0.0 Ok: queued\r\n"
                    }
                    (true, _) => b"",
                    (false, l) if l.starts_with("EHLO") => b"250-fake\r\n250 AUTH PLAIN LOGIN\r\n",
                    (false, "DATA") => {
                        in_data = true;
                        b"354 go ahead\r\n"
                    }
                    (false, "QUIT") => break,
                    _ => b"250 ok\r\n",
                };
                writer.write_all(reply).unwrap();
                line.clear();
            }
            seen
        });
        send_test(&Mail { port, ..Mail::default() }).unwrap();
        let seen = server.join().unwrap();
        assert!(seen.contains("RCPT TO:<test@example.com>\r\n"), "{seen}");
        assert!(seen.contains("Subject: Test email from Devid\r\n") && seen.contains("\r\n.\r\n"), "{seen}");
    }

    #[test]
    fn reports_an_smtp_refusal() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(b"554 no service\r\n").unwrap();
        });
        let e = send_test(&Mail { port, ..Mail::default() }).unwrap_err();
        assert!(e.to_string().contains("554 no service"), "{e}");
    }
}
