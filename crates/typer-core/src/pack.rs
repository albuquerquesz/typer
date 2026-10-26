//! Mechvibes pack manifests (`config.json`). Audio decoding lives in `typerd`;
//! this module only parses manifests so every crate can list packs cheaply.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::keycodes::mechvibes_to_evdev;
use crate::paths;
use crate::protocol::PackInfo;

pub const BUILTIN_PREFIX: &str = "builtin:";

/// Built-in synthesized packs: `(id, name)`.
pub const BUILTINS: &[(&str, &str)] = &[
    ("builtin:click", "Click (built-in)"),
    ("builtin:thock", "Thock (built-in)"),
];

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: Option<String>,
    key_define_type: Option<String>,
    sound: Option<String>,
    #[serde(default)]
    defines: BTreeMap<String, serde_json::Value>,
}

/// Where the sound for one key lives inside a pack.
#[derive(Debug, Clone, PartialEq)]
pub enum KeySound {
    /// Slice of the pack's single sprite file.
    Slice { start_ms: f64, duration_ms: f64 },
    /// A file of its own, relative to the pack directory.
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    /// Sprite file for `single` packs.
    pub sprite: Option<PathBuf>,
    /// evdev key code → sound.
    pub keys: BTreeMap<u16, KeySound>,
}

impl Manifest {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let file = dir.join("config.json");
        let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        let raw: RawManifest =
            serde_json::from_str(&text).with_context(|| format!("parsing {}", file.display()))?;
        Self::from_raw(dir, raw)
    }

    fn from_raw(dir: &Path, raw: RawManifest) -> anyhow::Result<Self> {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .context("pack directory has no name")?
            .to_owned();
        let multi = raw.key_define_type.as_deref() == Some("multi");
        let sprite = if multi { None } else { raw.sound.map(|s| dir.join(s)) };
        if !multi && sprite.is_none() {
            anyhow::bail!("pack {id}: single pack without a `sound` file");
        }

        let mut keys = BTreeMap::new();
        for (code, value) in raw.defines {
            let Some(evdev) = code.parse().ok().and_then(mechvibes_to_evdev) else { continue };
            let sound = match (&value, multi) {
                (serde_json::Value::String(file), true) => KeySound::File(dir.join(file)),
                (serde_json::Value::Array(pair), false) if pair.len() == 2 => {
                    match (pair[0].as_f64(), pair[1].as_f64()) {
                        (Some(start_ms), Some(duration_ms)) if duration_ms > 0.0 => {
                            KeySound::Slice { start_ms, duration_ms }
                        }
                        _ => continue,
                    }
                }
                _ => continue, // null / malformed entries are common in the wild
            };
            keys.insert(evdev, sound);
        }
        if keys.is_empty() {
            anyhow::bail!("pack {id}: no usable key definitions");
        }

        Ok(Self { name: raw.name.unwrap_or_else(|| id.clone()), id, dir: dir.to_owned(), sprite, keys })
    }
}

/// All packs: built-ins first, then valid user packs sorted by name.
pub fn list() -> Vec<PackInfo> {
    let mut packs: Vec<PackInfo> = BUILTINS
        .iter()
        .map(|(id, name)| PackInfo { id: (*id).into(), name: (*name).into(), builtin: true })
        .collect();
    let mut user: Vec<PackInfo> = fs::read_dir(paths::packs_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Manifest::load(&entry.path()).ok())
        .map(|m| PackInfo { id: m.id, name: m.name, builtin: false })
        .collect();
    user.sort_by_key(|a| a.name.to_lowercase());
    packs.extend(user);
    packs
}

pub fn user_pack_dir(id: &str) -> PathBuf {
    paths::packs_dir().join(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> anyhow::Result<Manifest> {
        Manifest::from_raw(Path::new("/packs/test-pack"), serde_json::from_str(json).unwrap())
    }

    #[test]
    fn single_pack() {
        let m = parse(
            r#"{"name":"Cherry","key_define_type":"single","sound":"sound.ogg",
                "defines":{"1":[0,120],"28":[500,150],"57416":[900,100],"30":null,"9999":[1,1]}}"#,
        )
        .unwrap();
        assert_eq!(m.id, "test-pack");
        assert_eq!(m.name, "Cherry");
        assert_eq!(m.sprite.as_deref(), Some(Path::new("/packs/test-pack/sound.ogg")));
        assert_eq!(m.keys.len(), 3);
        assert_eq!(m.keys[&28], KeySound::Slice { start_ms: 500.0, duration_ms: 150.0 });
        assert!(m.keys.contains_key(&103)); // Up arrow
    }

    #[test]
    fn single_is_default_type() {
        let m = parse(r#"{"sound":"a.ogg","defines":{"57":[0,80]}}"#).unwrap();
        assert_eq!(m.name, "test-pack");
        assert_eq!(m.keys.len(), 1);
    }

    #[test]
    fn multi_pack() {
        let m = parse(
            r#"{"key_define_type":"multi","defines":{"14":"bs.wav","57":"space.ogg","2":null}}"#,
        )
        .unwrap();
        assert!(m.sprite.is_none());
        assert_eq!(m.keys[&14], KeySound::File("/packs/test-pack/bs.wav".into()));
        assert_eq!(m.keys.len(), 2);
    }

    #[test]
    fn rejects_empty_and_spriteless() {
        assert!(parse(r#"{"key_define_type":"multi","defines":{}}"#).is_err());
        assert!(parse(r#"{"defines":{"1":[0,10]}}"#).is_err());
    }
}
