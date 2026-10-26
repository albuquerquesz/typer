use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths;

pub const DEFAULT_PACK: &str = "builtin:click";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// 0..=100
    pub volume: u8,
    pub pack: String,
    pub overlay: OverlayConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 60,
            pack: DEFAULT_PACK.to_owned(),
            overlay: OverlayConfig::default(),
        }
    }
}

/// Key overlay: whether it runs and where it sits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OverlayConfig {
    pub enabled: bool,
    /// Monitor (connector) it was placed on; `None` lets the compositor pick.
    pub monitor: Option<String>,
    /// Offset from the monitor's top-left corner; negative means "not placed
    /// yet" and the overlay picks bottom-center.
    pub x: i32,
    pub y: i32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            monitor: None,
            x: -1,
            y: -1,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::load_from(&paths::config_file())
    }

    pub fn load_from(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str::<Config>(&s).ok())
            .map(|mut c| {
                c.volume = c.volume.min(100);
                c
            })
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = paths::config_file();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, toml::to_string_pretty(self)?)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}
