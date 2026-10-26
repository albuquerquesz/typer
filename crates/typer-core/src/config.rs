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
}

impl Default for Config {
    fn default() -> Self {
        Self { enabled: true, volume: 60, pack: DEFAULT_PACK.to_owned() }
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
