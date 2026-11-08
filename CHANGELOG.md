# Changelog

## Unreleased

### Fixed
- The Library grid and the bookmark counts (explorer and status bar) kept showing the previous document right after opening a file, and after undo/redo: opening and history navigation replace the document and sync the checkpoint themselves, so the revision counter that keys the grid caches never moved. The revision now advances on open, undo, redo and history navigation, with regression tests. Found in the v1.1.0 field pass.

### Changed (screenshots)
- README screenshots refreshed with real v1.1.0 captures from the field pass (all workspaces, portable mode, real Windows desktop), including the first Health view capture; Merge, Organize, History and Health were exercised against two example bookmark files.

## 1.1.0 - 2026-11-07

### Added (watch folder + saved searches)
- Watch folder (Settings): watches a chosen folder for new or changed bookmark export files (polled every ~2.5s; the first scan is a silent baseline) and shows a banner offering to stage the file as a merge — never an automatic import, and the currently open file is never flagged.
- Saved searches: save the Library search under a name from the explorer, reapply in one click, delete with ×; stored in `settings.json`. The status bar and explorer bookmark counts are now cached like the grid rows.

### Added (snapshot retention + large-library scale)
- Snapshot retention: History prunes automatically to the newest 100 snapshots by default; Settings → Snapshots offers 25/50/100/250/All, the History view shows snapshot count and disk use with a Prune now button, and older `settings.json` files pick up the default.
- Scale: the Library grid caches its filtered rows and folder list (recomputed only when the document, folder or search changes), the row filter moved into the library as a pure function, and an automated test filters a 10,000-bookmark library.

### Added (command line + drag-and-drop)
- Headless CLI: running `rustmarks` with any argument skips the window and runs `check`, `health` (with `--fix` and `--json`), `merge` (`--prefer existing|incoming|combine`), `clean-urls`, `export-chrome` or `snapshot` — the same engines as the GUI, with script-friendly exit codes. `rustmarks help` lists them all.
- Drag-and-drop: dropping a bookmark file (`.html`, `.htm`, `.json`) onto the window opens it with a hover hint; unsaved changes get the same discard confirmation (with safety snapshot) as Ctrl+O.

### Added (library health)
- New **Health** view (Ctrl+5, also in the command palette): a fully local scan reporting duplicate URLs (keyed exactly like the merge engine), empty folders, missing titles, tracking parameters and malformed URLs, with per-issue and per-group automatic fixes — every fix takes a snapshot first and is undoable. Missing titles are derived from the URL; malformed URLs are flagged for manual repair in the grid, never auto-fixed. Nothing is checked over the network. View shortcuts renumbered: Health Ctrl+5, History Ctrl+6, Settings Ctrl+7.

### Changed (screenshots)
- README screenshots refreshed with real v1.0.2 captures from the Tessel field pass (all six workspaces, portable mode, real Windows desktop). The v1.0.1 loaded-library capture was retired; the Library shot now shows the Tessel empty state with its grid motif.

## 1.0.2 - 2026-11-07

### Changed (Tessel design system)
- The whole interface now conforms to the Tessel design system (SeanShaffer88/TesselGUI): token colors replace the blue-accented charcoal theme, with crimson as the single accent — one primary action per region, a crimson selection marker and active-region rule, and the 24px grid motif in empty states.
- Dark and Light themes follow the operating system, or can be fixed in Settings & help, which also gains accent presets (Crimson, Blue, Teal, Graphite) and density choices (Per component, Compact, Comfortable, Touch). Appearance persists locally in `settings.json`. One dependency added for OS theme detection: `dark-light`.
- Segoe UI and Cascadia Mono / Consolas load from the Windows Fonts folder at runtime; nothing is bundled.

### Added (command palette)
- Ctrl+K opens a command palette over every command (fuzzy search, shortcut hints, disabled commands explain why). Ctrl+, opens Settings, F1 opens help, and Ctrl+1…6 jump to the six views. A shortcut hint line now sits in the status bar.
- Settings & help subtitle now names the design system ("Tessel design") alongside the version.

### Added (screenshots)
- README now includes real screenshots (`docs/screenshots/`): Library (loaded and first-launch), Merge, Compare, Organize, History and Settings, captured on a real Windows machine from the v1.0.1 portable build during its field pass.

### Fixed (settings version)
- Settings & help showed a hardcoded "RustMarks 0.1.0"; it now reports the actual build version.

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
