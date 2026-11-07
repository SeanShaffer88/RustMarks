# RustMarks

An offline-first, native **Rust / egui desktop bookmark studio**, designed for Windows with a VS Code Dark Modern-inspired workspace. No browser runtime, account, telemetry, or backend.

## Run on Windows

Install the stable [Rust toolchain](https://rustup.rs/) and the **Desktop development with C++** workload from Visual Studio Build Tools (including the Windows SDK), then:

```powershell
cargo run --release
```

The standalone executable is built at `target/release/rustmarks.exe`:

```powershell
cargo build --release
```

The GitHub Actions workflow builds and tests on Windows and uploads the executable as `RustMarks-Windows`. Builds require internet access to download Rust crates; the finished application operates offline.

Linux is also supported by eframe/rfd, but requires platform development libraries (on Debian/Ubuntu: `build-essential pkg-config libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev libasound2-dev libudev-dev`) and a graphical session. Linux file dialogs use XDG portals.

## First steps

1. **Open** an exported Netscape bookmark HTML file, Chrome/Edge bookmark JSON, or a RustMarks JSON document. Try `examples/bookmarks.html`.
2. Expand the **Explorer** and select a folder, or use **All bookmarks**. Search titles, URLs, tags and notes using expressions such as `tag:dev -broken`.
3. Edit cells directly. Click row numbers to select, Ctrl-click to toggle rows, or Shift-click to select a range. Apply tags, delete selections, or remove tracking parameters in bulk.
4. Use **Details** for longer notes, URL validation, and local favicon import/preview (PNG, JPEG, ICO or GIF, normalized to a 32px PNG). Folder names can be edited in the sidebar.
5. Open **Smart Organizer**, scan local suggestions, and accept individually or move all suggestions into category folders.
6. In **Merge Studio**, choose one or more files. Inspect duplicates and choose existing, incoming, or combined tags/notes per conflict. Apply to the working copy, not directly to disk.
7. Inspect **Changes** in side-by-side or unified mode, then **Save** to commit. Revert restores the last committed document after taking a safety snapshot.
8. Use **Version History** to inspect and restore older snapshots into the working copy; review and save to commit the restored state.

**Shortcuts:** Ctrl+O open · Ctrl+S save · Ctrl+Shift+S save as · Ctrl+Z / Ctrl+Y session undo/redo when not editing text. In a grid cell, Up/Down or Enter moves between rows and Tab/Shift+Tab moves between cells. Left/Right retains normal text cursor navigation.

The grid virtualizes visible rows and keeps stable cell IDs. The bulk **Move selected** action preserves bookmark identity and accepts any folder in the tree. Document undo is bounded to 50 checkpoints; rapid typing is grouped, and text fields retain their own undo while focused.

## Formats & data safety

- **RustMarks JSON** is the lossless working format: stable IDs, tree structure, notes, tags, timestamps, suggestions and favicon data are preserved.
- **Netscape HTML** imports nested folders and bookmark attributes. Export escapes text and preserves dates, tags, icons and notes (`DATA-NOTES`, a RustMarks extension, and legacy `<DD>` descriptions). Other browsers may ignore custom note/tag attributes. IDs are regenerated on HTML import.
- **Chrome/Edge JSON** imports browser `roots` trees and converts the Windows-epoch microsecond timestamps. Settings includes a separate browser-compatible JSON export; that format omits RustMarks-only metadata. Saving an imported Chrome JSON file prompts for a new working file and refuses the original source path, preserving the browser file. Working files are native JSON or Netscape HTML. Never edit a live browser profile file.
- Saves use a same-directory temporary file, flush it, then rename it over the target. The previous committed state and the working copy are snapshotted before save. The containing directory must be writable.
- Major operations take compressed JSON snapshots first and cancel if a snapshot fails. These are stored in `%APPDATA%\RustMarks\history\` (or `$XDG_DATA_HOME/RustMarks/history`, defaulting to `~/.local/share/RustMarks/history` on Linux).
- Imports are limited to 64 MiB, folder depth to 128 levels, and restored snapshots to 64 MiB decompressed. Native documents require a folder root, a supported version, and unique node IDs. Local image imports have compressed-size, dimension and allocation limits.
- Closing with unsaved changes prompts to save, discard with a safety snapshot, or cancel.
- History is local, not encrypted, and has no automatic retention limit. Snapshots include bookmark URLs and notes; manage disk space and permissions accordingly.
- Merge URL comparison strips fragments and common tracking keys (`utm_*`, `fbclid`, `gclid`, `msclkid`), sorts query parameters, and otherwise preserves meaningful URL distinctions. Original URL text is retained unless you explicitly run URL cleanup. Import folder structure is retained under its source root.

## Architecture

| Module | Responsibility |
| --- | --- |
| `models.rs` | Unified document, folders, bookmarks and tree operations |
| `parser.rs` | Netscape HTML, Chrome JSON, native JSON and exporters |
| `heuristics.rs`, `rules.rs` | Weighted, user-editable local domain/token rules and expression filtering |
| `merge.rs` | URL normalization, staged merge and metadata resolutions |
| `diff_view.rs` | ID-based additions, removals, metadata edits and folder moves |
| `history.rs` | Compressed, atomically published JSON snapshots |
| `editor.rs` | Desktop workspace, inline grid, selection and file workflows |
| `vscode_theme.rs` | Dark Modern palette and egui styling |
| `grid.rs`, `undo.rs` | Testable keyboard cell navigation and bounded session undo/redo |
| `storage.rs`, `favicon.rs` | Atomic file publication and bounded local image processing |

The core library is separate from the native shell for integration testing. The original project specification is in [`docs/blueprint.md`](docs/blueprint.md).

## Development & verification

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets
cargo build --release
```

Integration tests cover serialization, HTML escaping/BOM/legacy descriptions, Chrome timestamps including extremes, filtering, domain boundaries, user dictionaries, URL normalization, merge resolutions, organizer identity, same-name-folder moves and sibling reorder diffs, favicon conversion, atomic-write failure cleanup, snapshot restore, and undo/redo. A headless egui smoke test renders all seven panels against a 1,000-bookmark document. Four dependency-free unit tests cover grid navigation.

### Local categorization dictionaries

Settings provides a JSON dictionary editor with validation and an explicit save action. Rules persist beside the history directory in `RustMarks/rules.json` and load on startup. Each rule has a `category`, `domains`, `keywords`, `domain_weight` (default 10), and `keyword_weight` (default 1). Hosts match exact domains and subdomains; keywords match whole title/path/tag/note tokens. Earlier rules win equal scores. Invalid edits do not replace the active dictionary. Saving a dictionary does not move bookmarks: rescan and review suggestions in Smart Organizer.

### Verification status / release gate

No new commit or pull request has been made for the follow-up development. `rustfmt --check`, manifest metadata validation, and Git whitespace checks run locally. All four standalone grid-navigation tests pass. Full `cargo test --all-targets` remains blocked by the sandbox's inability to download dependencies from `index.crates.io`; no full-build or complete-test pass is claimed. The **previous merged revision's Windows CI failed** during its test/build step; detailed logs are hosted on a domain unavailable to this sandbox. This is an unresolved release blocker, not a passing baseline.

Before the next commit/PR, complete the build/test and native Windows QA gates in [`docs/verification.md`](docs/verification.md). The headless UI smoke test and expanded integration suite are implemented but have not yet run. `Cargo.lock` must be generated and reviewed on a machine with registry access for reproducible application builds.

### Remaining verification constraints

There is no automatic network favicon fetching: this is intentionally an offline-first utility. JSON is the document/snapshot persistence layer rather than SQLite. Snapshots are compressed with gzip and have no automatic retention limit. The grid virtualizes drawing but filtering and document comparisons still traverse the tree; very large collections need performance testing. Disk snapshots cover major operations; session undo/redo is memory-only, not crash recovery for every keystroke. There is no synchronized horizontal tree scrolling or arbitrary folder drag-and-drop (bulk bookmark moves are supported).

## License

MIT — see [LICENSE](LICENSE).
