//! Line-delimited JSON protocol spoken over `$XDG_RUNTIME_DIR/typer.sock`.
//! Each request is one JSON object per line and gets exactly one response line,
//! except `subscribe` and `subscribe_keys`, which keep the connection open and
//! stream `state` / `key` lines.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Toggle,
    SetEnabled {
        enabled: bool,
    },
    SetVolume {
        volume: u8,
    },
    AdjustVolume {
        delta: i16,
    },
    SetPack {
        id: String,
    },
    ListPacks,
    /// Play a sample key from the active pack.
    Preview,
    /// Rescan pack directories and reload the active pack.
    Reload,
    Subscribe,
    /// Turn the key overlay on or off.
    SetOverlay {
        enabled: bool,
    },
    /// Enter or leave the overlay's drag-to-place mode.
    MoveOverlay {
        moving: bool,
    },
    /// Save where the overlay was dropped; also leaves move mode.
    SetOverlayPosition {
        monitor: Option<String>,
        x: i32,
        y: i32,
    },
    /// Shut the daemon down; it replies with the last state before exiting.
    Quit,
    /// Stream pressed keys as labels, for the overlay. Nothing is stored.
    SubscribeKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    State(State),
    Packs { packs: Vec<PackInfo> },
    Key { label: String },
    Error { message: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub enabled: bool,
    pub volume: u8,
    pub pack: String,
    pub pack_name: String,
    #[serde(default)]
    pub overlay: bool,
    #[serde(default)]
    pub overlay_moving: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub builtin: bool,
}
