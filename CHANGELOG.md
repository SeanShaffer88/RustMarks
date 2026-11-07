# Changelog

## Unreleased

### Added (window state)
- Main window size, position and maximized state persist to `RustMarks/window.json` (debounced, atomic) and restore on launch. Corrupt/missing files fall back to 1440x900; sizes below the 1000x650 minimum are raised; implausible positions are dropped.

### Fixed
- Compile error in `ui_components::navigation`: `WidgetInfo::labeled` takes an `enabled` argument in egui 0.28.
- Clippy: collapsed nested `if`/`match` in `editor.rs` and `parser.rs`; `push('\n')` in `merge.rs`.

### Added
- `Cargo.lock` committed for reproducible application builds.
- `CHANGELOG.md`, `CREDITS.md`, `AUDIT.md`.

### Verification
- Linux: `cargo fmt --check`, `cargo test --all-targets` (69 tests), `cargo clippy --all-targets` (0 warnings), `cargo build --release` all pass.
- Windows CI result for this revision: see the pull request checks.

## 0.1.0
- Initial offline-first egui bookmark studio: library grid, merge review, compare, organizer, history, settings.
