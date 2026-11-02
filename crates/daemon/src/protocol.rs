//! Wire format: one JSON object per line in each direction.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    // Untagged: most specific variant first, or an error would parse as `Ok`.
    Status(Status),
    Error { ok: bool, error: String },
    Ok { ok: bool },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    pub ok: bool,
    pub version: String,
    pub pid: u32,
    pub uptime_secs: u64,
    pub home: String,
    pub server: String,
    pub default_php: String,
}

impl Response {
    pub fn error(msg: impl Into<String>) -> Self {
        Response::Error { ok: false, error: msg.into() }
    }
}
