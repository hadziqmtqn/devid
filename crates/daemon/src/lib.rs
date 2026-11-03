//! Devid daemon: owns all state and behaviour; clients talk to it over a unix socket (D-012).

pub mod apache;
pub mod ca;
pub mod caddy;
pub mod config;
pub mod dns;
pub mod helper;
pub mod install;
pub mod logs;
pub mod nginx;
pub mod paths;
pub mod php;
pub mod protocol;
pub mod releases;
pub mod server;
pub mod sites;
pub mod supervisor;
pub mod webserver;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
