//! Shared types for typer: config, Mechvibes pack manifests, IPC protocol and paths.

pub mod client;
pub mod config;
pub mod keycodes;
pub mod pack;
pub mod paths;
pub mod popup;
pub mod protocol;

pub use config::Config;
pub use protocol::{PackInfo, Request, Response, State};
