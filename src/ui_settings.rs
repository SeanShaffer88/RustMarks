//! Persisted appearance settings for the Tessel design system: theme mode,
//! accent preset and density. Stored as readable JSON in the data directory
//! (`settings.json` beside `rules.json`); no telemetry, no network.
use crate::parser::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Follow the operating system's light/dark setting.
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccentPreset {
    /// Tessel's brand crimson.
    #[default]
    Crimson,
    Blue,
    Teal,
    /// Neutral accent for color-critical work.
    Graphite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Compact for rows, trees and toolbars; Comfortable for navigation,
    /// forms and dialogs (the Tessel default).
    #[default]
    PerComponent,
    Compact,
    Comfortable,
    Touch,
}

fn default_snapshot_keep() -> u32 {
    100
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub theme: ThemeMode,
    pub accent: AccentPreset,
    pub density: Density,
    /// How many history snapshots to keep (0 = keep all). Older snapshots
    /// are pruned automatically after each new one is taken.
    #[serde(default = "default_snapshot_keep")]
    pub snapshot_keep_last: u32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::default(),
            accent: AccentPreset::default(),
            density: Density::default(),
            snapshot_keep_last: default_snapshot_keep(),
        }
    }
}

impl UiSettings {
    pub fn default_path() -> PathBuf {
        crate::paths::data_dir().join("settings.json")
    }

    /// Load settings from the default location; any problem (missing file,
    /// invalid JSON) yields the defaults rather than an error.
    pub fn load() -> Self {
        Self::load_from(&Self::default_path())
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::default_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        crate::storage::atomic_write(path, text.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_system_crimson_per_component() {
        let settings = UiSettings::default();
        assert_eq!(settings.theme, ThemeMode::System);
        assert_eq!(settings.accent, AccentPreset::Crimson);
        assert_eq!(settings.density, Density::PerComponent);
    }

    #[test]
    fn settings_roundtrip_through_a_file() {
        let dir = std::env::temp_dir().join(format!("rustmarks-ui-{}", uuid::Uuid::new_v4()));
        let path = dir.join("settings.json");
        let settings = UiSettings {
            theme: ThemeMode::Light,
            accent: AccentPreset::Teal,
            density: Density::Touch,
            snapshot_keep_last: 25,
        };
        settings.save_to(&path).unwrap();
        assert_eq!(UiSettings::load_from(&path), settings);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn older_files_without_the_snapshot_limit_default_to_100() {
        let dir = std::env::temp_dir().join(format!("rustmarks-ui-{}", uuid::Uuid::new_v4()));
        let path = dir.join("settings.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            r#"{"theme":"Dark","accent":"Crimson","density":"Compact"}"#,
        )
        .unwrap();
        let loaded = UiSettings::load_from(&path);
        assert_eq!(loaded.snapshot_keep_last, 100);
        assert_eq!(UiSettings::default().snapshot_keep_last, 100);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_invalid_files_fall_back_to_defaults() {
        let missing = std::env::temp_dir()
            .join(format!("rustmarks-ui-{}", uuid::Uuid::new_v4()))
            .join("nope.json");
        assert_eq!(UiSettings::load_from(&missing), UiSettings::default());
        let dir = std::env::temp_dir().join(format!("rustmarks-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(UiSettings::load_from(&path), UiSettings::default());
        // Partial JSON keeps defaults for the missing fields.
        std::fs::write(&path, "{\"theme\":\"dark\"}").unwrap();
        let loaded = UiSettings::load_from(&path);
        assert_eq!(loaded.theme, ThemeMode::Dark);
        assert_eq!(loaded.accent, AccentPreset::Crimson);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
