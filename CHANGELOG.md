# Changelog

## Unreleased

## 1.0.1 - 2026-11-07

### Added (app icon)
- RustMarks now has its own icon (fanned bookmark stack, red on charcoal): embedded in the executable, shown in the window/taskbar, on the Start-menu shortcut, in Add/Remove Programs, and on Setup.exe. Artwork in `assets/` (`icon.ico`, `icon.png`).

### Changed (release contents)
- Releases now publish only `RustMarks-v<ver>-x64-Portable.zip` and `RustMarks-v<ver>-x64-Setup.exe`, plus `SHA256SUMS.txt`. The bare exe and the standalone MSI are no longer release assets (the MSI remains embedded in Setup.exe). The v1.0.0 release was trimmed to match.

## 1.0.0 - 2026-11-07

First stable release. Everything in 0.1.0–0.3.0, plus the installer, portable mode and tooltip work below.

### Added (tooltips, completed)
- Tooltips for sidebar navigation, View menu entries and diff tree toggles; every interactive control now has one.

### Added (installers)
- Per-user MSI (`RustMarks-v<ver>-x64.msi`) and bundled `RustMarks-v<ver>-x64-Setup.exe` (WiX 5, payload embedded, offline). Installs to `%LOCALAPPDATA%\Programs\RustMarks` with a Start-menu shortcut; no admin rights, no runtime prerequisites. The `Installers` workflow builds both and runs a silent install/uninstall smoke test on Windows for every packaging change. Release workflow now publishes them.
- Not covered: x86 builds, code signing (SmartScreen will warn), uninstall does not remove user data.

### Added (portable mode)
- If `portable.txt` sits next to `rustmarks.exe`, history, rules and window state are stored in an adjacent `data/` folder and nothing is written under %APPDATA%. The release workflow now ships `RustMarks-<tag>-x64-Portable.zip` (with the marker) plus the bare `RustMarks-<tag>-x64.exe` (per-user data). Paths are resolved in one place (`paths.rs`).
- Limit: the OS file dialogs and a read-only location (e.g. Program Files) are the user's responsibility; failures to write `data/` surface as normal save errors.

### Added
- Descriptive tooltips (action + side effects) on buttons, tabs, text fields, combo boxes and the diff toggle. 

## 0.2.0 - 2026-11-07

### Added (release)
- `release.yml`: pushing a `v*` tag builds on Windows with `--locked`, tests, and publishes `RustMarks-<tag>-x64.zip`, `RustMarks-<tag>-x64.exe` and `SHA256SUMS.txt` to a GitHub release. No installer/MSI yet; the build still stores data under %APPDATA%, so it is not a data-portable build.

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
