//! Devid daemon: owns all state and behaviour; clients talk to it over a unix socket (D-012).

pub mod config;
pub mod paths;
pub mod protocol;
pub mod server;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
