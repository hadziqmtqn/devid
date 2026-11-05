//! Wire format: one JSON object per line in each direction.
//! Requests are typed; replies are JSON objects with `ok: true` or `ok: false, error`.
//! Paths are absolute; the CLI resolves them before sending.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Stop,
    PhpList,
    /// Install a PHP version from a build folder, or from the release manifest when `from` is None.
    PhpInstall {
        version: String,
        from: Option<String>,
    },
    PhpUse {
        version: String,
    },
    /// Turn xdebug or pcov on or off for a PHP version (restarts that version's php-fpm).
    PhpExt {
        version: String,
        ext: String,
        enabled: bool,
    },
    /// Catalog extensions (Phase 6) the release offers for a PHP version, marked installed or not.
    PhpExtCatalog {
        version: String,
    },
    /// Download, verify and turn on catalog extensions, one after another; php-fpm restarts once at
    /// the end. Replies with one result per name.
    PhpExtInstall {
        version: String,
        names: Vec<String>,
    },
    /// The running `PhpExtInstall`, if any: which extension, phase and percent.
    PhpExtProgress,
    PhpExtRemove {
        version: String,
        name: String,
    },
    /// Bundled extensions (`php -m`), xdebug/pcov on or off, and installed catalog extensions.
    PhpExtList {
        version: String,
    },
    /// Restart one service: "php-8.4", "nginx", "caddy" or "apache".
    Restart {
        service: String,
    },
    /// Stop a service and keep it stopped until `ServiceStart` (or a daemon restart).
    ServiceStop {
        service: String,
    },
    ServiceStart {
        service: String,
    },
    StopAll,
    StartAll,
    /// Log files in ~/.devid/log.
    Logs,
    /// A log's tail (offset None) or what is new since `offset`; replies with the new offset.
    LogRead {
        name: String,
        offset: Option<u64>,
    },
    LogClear {
        name: String,
    },
    /// Install a web server binary (`from` optional for apache: the system httpd).
    ServerInstall {
        server: String,
        from: Option<String>,
    },
    /// Make an installed web server the active one (D-005: one at a time).
    ServerUse {
        server: String,
    },
    ServerList,
    /// Compare installed PHP/nginx/Caddy with the release manifest; install newer ones unless `check`.
    Update {
        check: bool,
    },
    Sites,
    /// Serve `path` as `<name>.<tld>`; the name defaults to the folder name.
    Link {
        name: Option<String>,
        path: String,
    },
    Unlink {
        name: String,
    },
    /// Serve every subfolder of `path`.
    Park {
        path: String,
    },
    Unpark {
        path: String,
    },
    /// Stop serving a parked site; its folder is not touched (D-029).
    Ignore {
        site: String,
    },
    /// Serve an ignored parked site again.
    Unignore {
        site: String,
    },
    /// Pin a site to a PHP version; the site is `site` or the one containing `cwd`.
    Isolate {
        version: String,
        site: Option<String>,
        cwd: String,
    },
    Unisolate {
        site: Option<String>,
        cwd: String,
    },
}

/// Send one request to the daemon's socket and return its JSON reply.
/// Shared by every client: the CLI, the TUI and the desktop app.
pub fn request(paths: &crate::paths::Paths, req: &Request) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader, Write};
    let sock = paths.socket();
    let mut stream = std::os::unix::net::UnixStream::connect(&sock).map_err(|_| {
        format!("daemon is not running (no socket at {}); start it with `devid daemon run`", sock.display())
    })?;
    let mut line = serde_json::to_string(req).expect("request serializes");
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&reply).map_err(|e| format!("bad reply from daemon: {e}"))?;
    if value["ok"] == false {
        return Err(value["error"].as_str().unwrap_or("daemon returned an error").to_string());
    }
    Ok(value)
}
