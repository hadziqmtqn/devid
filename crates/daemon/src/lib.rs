//! Devid daemon: owns all state and behaviour; clients talk to it over a unix socket (D-012).

pub mod config;
pub mod nginx;
pub mod paths;
pub mod php;
pub mod protocol;
pub mod server;
pub mod sites;
pub mod supervisor;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
