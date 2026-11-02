//! Wire format: one JSON object per line in each direction.
//! Requests are typed; replies are JSON objects with `ok: true` or `ok: false, error`.
//! Paths are absolute; the CLI resolves them before sending.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Stop,
    PhpList,
    PhpInstall {
        version: String,
        from: String,
    },
    PhpUse {
        version: String,
    },
    /// Install a web server binary; M3 supports nginx only.
    ServerInstall {
        server: String,
        from: String,
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
