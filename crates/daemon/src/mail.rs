//! Mail (D-049): the daemon runs the official Mailpit build, so the mail that apps send lands in an
//! inbox on this Mac. Off by default (`[mail]` in config.toml). Mailpit listens on 127.0.0.1 only:
//! SMTP on `port`, its inbox page and API on the next port.

use crate::{config::Mail, paths::Paths, php, supervisor::Spec, webserver};
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
pub fn install(paths: &Paths, from: &Path) -> io::Result<String> {
    let out = webserver::probe(from, &["version"])?;
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
fn api(port: u16, method: &str, path: &str, body: &str) -> io::Result<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    )?;
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply)?;
    reply_body(&String::from_utf8_lossy(&reply))
}

fn reply_body(reply: &str) -> io::Result<String> {
    let (head, body) = reply.split_once("\r\n\r\n").ok_or_else(|| err("Mailpit sent an incomplete reply"))?;
    let status = head.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok());
    match status {
        Some(200..=299) => Ok(body.to_string()),
        Some(code) => Err(err(format!("Mailpit answered {code}: {}", body.trim()))),
        None => Err(err("Mailpit's reply has no status")),
    }
}

/// Mailpit's `/api/v1/info` (its version, and the message and unread counts); None if it does not answer.
pub fn info(mail: &Mail) -> Option<serde_json::Value> {
    serde_json::from_str(&api(inbox_port(mail), "GET", "/api/v1/info", "").ok()?).ok()
}

/// Delete every message (no IDs given means all of them).
pub fn clear(mail: &Mail) -> io::Result<()> {
    api(inbox_port(mail), "DELETE", "/api/v1/messages", "{}").map(|_| ())
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
         Apps on this Mac can send mail to 127.0.0.1:{port}. It lands in this inbox and never reaches a \
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
    fn reads_only_successful_api_replies() {
        assert_eq!(reply_body("HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\n\r\nok").unwrap(), "ok");
        assert!(reply_body("HTTP/1.0 403 Forbidden\r\n\r\nInvalid Host").unwrap_err().to_string().contains("403"));
        assert!(reply_body("HTTP/1.0 200 OK\r\n").is_err(), "no blank line: incomplete");
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
