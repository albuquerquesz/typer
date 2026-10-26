//! Line-delimited JSON protocol spoken over `$XDG_RUNTIME_DIR/typer.sock`.
//! Each request is one JSON object per line and gets exactly one response line,
//! except `subscribe`, which keeps the connection open and streams `state` lines.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Toggle,
    SetEnabled { enabled: bool },
    SetVolume { volume: u8 },
    AdjustVolume { delta: i16 },
    SetPack { id: String },
    ListPacks,
    /// Play a sample key from the active pack.
    Preview,
    /// Rescan pack directories and reload the active pack.
    Reload,
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    State(State),
    Packs { packs: Vec<PackInfo> },
    Error { message: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub enabled: bool,
    pub volume: u8,
    pub pack: String,
    pub pack_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub builtin: bool,
}
