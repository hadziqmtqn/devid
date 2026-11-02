//! Wire format: one JSON object per line in each direction.
//! Requests are typed; replies are JSON objects with `ok: true` or `ok: false, error`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Stop,
    PhpList,
    PhpInstall { version: String, from: String },
    PhpUse { version: String },
}
