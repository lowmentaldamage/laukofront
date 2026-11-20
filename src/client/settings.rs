//! Settings remembered between runs, stored as TOML in the user's app-data folder
//! (`%APPDATA%\laukofront\settings.toml` on Windows, `$HOME/.config/laukofront/` elsewhere).
//! A missing or broken file simply gives the defaults.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedSettings {
    /// The on-map tutorial was finished or dismissed.
    pub tutorial_done: bool,
    /// Attack size the game starts with (0.1..=0.9).
    pub default_attack_ratio: f64,
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            tutorial_done: false,
            default_attack_ratio: 0.2,
        }
    }
}

impl SavedSettings {
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("laukofront").join("settings.toml"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| Self::from_toml(&s))
            .unwrap_or_default()
    }

    pub fn from_toml(s: &str) -> Option<Self> {
        let mut v: Self = toml::from_str(s).ok()?;
        v.default_attack_ratio = v.default_attack_ratio.clamp(0.1, 0.9);
        Some(v)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// Writes the file; failures are ignored (settings are a convenience).
    pub fn save(&self) {
        if let Some(p) = Self::path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.to_toml());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_clamping() {
        let s = SavedSettings {
            tutorial_done: true,
            default_attack_ratio: 0.5,
        };
        assert_eq!(SavedSettings::from_toml(&s.to_toml()), Some(s));
        let wild = SavedSettings::from_toml("default_attack_ratio = 7.0").unwrap();
        assert_eq!(wild.default_attack_ratio, 0.9);
        assert!(!wild.tutorial_done, "missing keys take defaults");
        assert!(SavedSettings::from_toml("not toml [").is_none());
    }
}
