//! Persisted main-window geometry: size, position and maximized state.
use crate::{parser::Result, storage};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_SIZE: [f32; 2] = [1440., 900.];
pub const MIN_SIZE: [f32; 2] = [1000., 650.];
const MAX_DIM: f32 = 16384.;
const MAX_COORD: f32 = 32000.;
const FILE_LIMIT: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub size: [f32; 2],
    pub position: Option<[f32; 2]>,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            position: None,
            maximized: false,
        }
    }
}

impl WindowState {
    /// Clamp to sane values: non-finite or absurd data falls back to defaults,
    /// small sizes are raised to the minimum, implausible positions are dropped.
    pub fn sanitized(self) -> Self {
        let ok = |v: f32| v.is_finite();
        let size = if self.size.iter().copied().all(ok) {
            [
                self.size[0].clamp(MIN_SIZE[0], MAX_DIM),
                self.size[1].clamp(MIN_SIZE[1], MAX_DIM),
            ]
        } else {
            DEFAULT_SIZE
        };
        let position = self
            .position
            .filter(|p| p.iter().copied().all(ok) && p.iter().all(|c| c.abs() <= MAX_COORD));
        Self {
            size,
            position,
            maximized: self.maximized,
        }
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .map(Self::sanitized)
    }

    /// Missing, oversized or corrupt files yield the default state.
    pub fn load(path: &Path) -> Self {
        let small = std::fs::metadata(path)
            .map(|m| m.len() <= FILE_LIMIT)
            .unwrap_or(false);
        if !small {
            return Self::default();
        }
        std::fs::read(path)
            .ok()
            .and_then(|b| Self::parse(&b))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        storage::atomic_write(path, &serde_json::to_vec_pretty(&self.sanitized())?)
    }

    /// Same data directory as the history and dictionaries.
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("XDG_DATA_HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".local/share")
            });
        base.join("RustMarks/window.json")
    }
}
