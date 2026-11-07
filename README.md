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
4. Use **Details** for longer notes, URL validation and local favicon data URIs. Folder names can be edited in the sidebar.
5. Open **Smart Organizer**, scan local suggestions, and accept individually or move all suggestions into category folders.
6. In **Merge Studio**, choose one or more files. Inspect duplicates and choose existing, incoming, or combined tags/notes per conflict. Apply to the working copy, not directly to disk.
7. Inspect **Changes** in side-by-side or unified mode, then **Save** to commit. Revert restores the last committed document after taking a safety snapshot.
8. Use **Version History** to restore older snapshots into the working copy; review and save to commit the restored state.

**Shortcuts:** Ctrl+O open · Ctrl+S save · Ctrl+Shift+S save as.

## Formats & data safety

- **RustMarks JSON** is the lossless working format: stable IDs, tree structure, notes, tags, timestamps, suggestions and favicon data are preserved.
- **Netscape HTML** imports nested folders and bookmark attributes. Export escapes text and preserves dates, tags, icons and notes (`DATA-NOTES`, a RustMarks extension). Other browsers may ignore custom note/tag attributes. IDs are regenerated on HTML import.
- **Chrome/Edge JSON** imports browser `roots` trees and converts the Windows-epoch microsecond timestamps. Settings includes a separate browser-compatible JSON export; that format omits RustMarks-only metadata. Saving a loaded Chrome JSON file writes RustMarks JSON, not the browser's internal schema. Use **Save as** to keep the original browser file intact. Never edit a live browser profile file.
- Saves use a same-directory temporary file, flush it, then rename it over the target. The previous committed state and the working copy are snapshotted before save. The containing directory must be writable.
- Major operations take compressed JSON snapshots first and cancel if a snapshot fails. These are stored in `%APPDATA%\RustMarks\history\` (or `$XDG_DATA_HOME/RustMarks/history`, defaulting to `~/.local/share/RustMarks/history` on Linux).
- Closing with unsaved changes prompts to save, discard with a safety snapshot, or cancel.
- History is local, not encrypted, and has no automatic retention limit. Snapshots include bookmark URLs and notes; manage disk space and permissions accordingly.
- Merge URL comparison strips fragments and common tracking keys (`utm_*`, `fbclid`, `gclid`, `msclkid`), sorts query parameters, and otherwise preserves meaningful URL distinctions. Original URL text is retained unless you explicitly run URL cleanup. Import folder structure is retained under its source root.

## Architecture

| Module | Responsibility |
| --- | --- |
| `models.rs` | Unified document, folders, bookmarks and tree operations |
| `parser.rs` | Netscape HTML, Chrome JSON, native JSON and exporters |
| `heuristics.rs` | Weighted local domain/token rules and expression filtering |
| `merge.rs` | URL normalization, staged merge and metadata resolutions |
| `diff_view.rs` | ID-based additions, removals, metadata edits and folder moves |
| `history.rs` | Compressed, atomically published JSON snapshots |
| `editor.rs` | Desktop workspace, inline grid, selection and file workflows |
| `vscode_theme.rs` | Dark Modern palette and egui styling |

The core library is separate from the native shell for integration testing. The original project specification is in [`docs/blueprint.md`](docs/blueprint.md).

## Development & verification

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets
cargo build --release
```

Integration tests cover native serialization, nested HTML and escaping, Chrome dates, filtering, domain boundaries, URL normalization, merge resolutions, organizer identity/idempotency, structural diffs, snapshot restore and repeated disk saves.

**Implementation-environment verification:** Rust syntax/formatting was checked with rustfmt 1.88. `cargo test --all-targets` was attempted but dependency download was blocked by the sandbox's network restrictions (`index.crates.io`). Compilation, test execution and native UI smoke testing still need to run in CI or on a development machine; no passing build is claimed.

## Current scope

This is an initial implementation of all six roadmap areas, not a fully polished production release. JSON files are used for document persistence; SQLite is not required. Snapshots use gzip. The diff UI compares flattened tree paths/metadata side by side rather than synchronizing two expandable trees. The grid uses egui text-field keyboard navigation, not full Excel arrow-key cell navigation or virtualized rows. Rules are currently extended in `heuristics.rs`, not through a user dictionary editor. Favicons can be preserved and edited as data URIs; there is no network fetching or image cache. Imported legacy `<DD>` descriptions are not currently read (RustMarks `DATA-NOTES` attributes are). There is no undo stack between snapshots, and very large bookmark trees may need grid virtualization.

## License

MIT — see [LICENSE](LICENSE).
