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
