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
- Merge comparison removes common tracking keys (`utm_*`, `fbclid`, `gclid`, `msclkid`) from HTTP(S) URLs and canonicalizes their scheme/host through the URL parser. **Fragments, query order, duplicate keys and raw query encoding are retained** to avoid collapsing distinct routes or changing meaningful values. Cleanup leaves non-web/invalid URLs and recognized signed queries (`signature`, `sig`, `X-Amz-Signature`, `X-Goog-Signature`, `oauth_signature`) untouched. Arbitrary application-specific signatures cannot be identified automatically; inspect staged changes before saving. Stored URLs stay verbatim unless you explicitly run URL cleanup.
- Merge imports retain folder structure, including intentionally empty folders. Combining the same notes/tags repeatedly does not append duplicate complete note lines or tags.
- Open files are tracked by exact source bytes. Saving back to the opened path checks for external edits, deletion, or a retargeted source symlink and refuses to overwrite a changed source. Reopen or use a distinct **Save as** destination to resolve the conflict. This is an optimistic check immediately before publication, **not an OS-level lock** against a writer racing the final rename. Saving through an existing symlink updates its target without replacing the link. Imported browser JSON protection recognizes canonical path aliases, rather than only comparing path strings.

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
| `storage.rs`, `revision.rs`, `favicon.rs` | Atomic file publication, exact-byte source-change checks and bounded local image processing |
| `url_cleanup.rs` | Conservative tracking cleanup that preserves route/query semantics |

The core library is separate from the native shell for integration testing. The original project specification is in [`docs/blueprint.md`](docs/blueprint.md).

## Development & verification

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets
cargo build --release
```

Integration tests cover serialization, HTML escaping/BOM/legacy descriptions, Chrome timestamps including extremes, filtering, domain boundaries, user dictionaries, URL normalization, merge resolutions, organizer identity, same-name-folder moves and sibling reorder diffs, favicon conversion, atomic-write failure cleanup, snapshot restore, and undo/redo. A headless egui smoke test renders all seven panels against a 1,000-bookmark document. Six dependency-free unit tests cover grid navigation (including overflow and exhaustive small-table cases), and seven standalone source-revision tests cover external edits/deletion, aliases, invalid UTF-8 and size limits (the symlink-retargeting test runs on Unix). Additional integration regressions cover checked save / Save as, signed URL cleanup, distinct fragment routes, repeat merge resolutions and empty folders.

### Local categorization dictionaries

Settings provides a JSON dictionary editor with validation and an explicit save action. Rules persist beside the history directory in `RustMarks/rules.json` and load on startup. Each rule has a `category`, `domains`, `keywords`, `domain_weight` (default 10), and `keyword_weight` (default 1). Hosts match exact domains and subdomains; keywords match whole title/path/tag/note tokens. Earlier rules win equal scores. Invalid edits do not replace the active dictionary. Saving a dictionary does not move bookmarks: rescan and review suggestions in Smart Organizer.

### Verification status / release gate

Local verification for the current pass: `rustfmt --check`, manifest metadata validation and Git whitespace checks pass. All **13 standalone Rust tests** (six grid-navigation and seven source-revision tests) and **seven CI-runner Python tests** pass. New regressions first reproduced grid-index overflow and lost compiler diagnostics, then passed after the fixes. Full `cargo test --all-targets` remains blocked by access to `index.crates.io`; no complete build/test pass is claimed.

The latest failed Windows CI annotation identified **E0502 in the editor tab bar**: `self.tab` was mutably borrowed while `self.dirty()` borrowed the whole app. The label is now computed before borrowing the tab, and the reported theme float-type warning is fixed. New headless tests exercise tab selection and clean/modified marker transitions, and the existing panel smoke test now includes the actual tab bar. **The GUI tests and fixed Windows build still require execution with Cargo dependencies.** See [`docs/verification.md`](docs/verification.md) for the diagnostic, regression evidence and remaining gates.

The workflow runs Cargo gates through `scripts/ci.py`, preserving exit status and full logs. Failed gates publish bounded compiler/test error context plus the final output tail as a check annotation and step summary, so later background compilation does not hide the actual failure. Logs and any generated lockfile are retained as artifacts. The wrapper has seven dependency-free regression tests (`python -m unittest discover -s scripts -p "test_*.py"`).

Before the next commit/PR, complete the build/test and native Windows QA gates in [`docs/verification.md`](docs/verification.md). The headless UI smoke test and expanded integration suite are implemented but have not yet run. `Cargo.lock` must be generated and reviewed on a machine with registry access for reproducible application builds.

### Remaining verification constraints

There is no automatic network favicon fetching: this is intentionally an offline-first utility. JSON is the document/snapshot persistence layer rather than SQLite. Snapshots are compressed with gzip and have no automatic retention limit. The grid virtualizes drawing but filtering and document comparisons still traverse the tree; very large collections need performance testing. Disk snapshots cover major operations; session undo/redo is memory-only, not crash recovery for every keystroke. There is no synchronized horizontal tree scrolling or arbitrary folder drag-and-drop (bulk bookmark moves are supported).

## License

MIT — see [LICENSE](LICENSE).
