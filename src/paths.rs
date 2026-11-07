//! Resolves the application data directory.
//!
//! Portable mode: if a file named `portable.txt` sits next to the executable,
//! all data (history, rules, window state) lives in `data/` beside it and
//! nothing is written under %APPDATA%. Otherwise the per-user location is used.
use std::path::{Path, PathBuf};

pub const PORTABLE_MARKER: &str = "portable.txt";

/// Pure resolver, testable without touching the real environment.
pub fn resolve(exe_dir: Option<&Path>, user_base: PathBuf) -> PathBuf {
    match exe_dir {
        Some(dir) if dir.join(PORTABLE_MARKER).is_file() => dir.join("data"),
        _ => user_base.join("RustMarks"),
    }
}

fn user_base() -> PathBuf {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".local/share")
        })
}

/// The directory that holds `history/`, `rules.json` and `window.json`.
pub fn data_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    resolve(exe_dir.as_deref(), user_base())
}
