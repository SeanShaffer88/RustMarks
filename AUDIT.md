# Audit

Date: 2026-11-07. Revision: `main` after PR #11 plus the final-polish change.

| Check | Platform | Result |
| --- | --- | --- |
| `cargo fmt --check` | Linux + Windows CI | Pass |
| `cargo test --all-targets` | Linux + Windows CI | Pass (73 tests) |
| `cargo clippy --all-targets` | Linux + Windows CI | Pass, 0 warnings |
| `cargo build --release` | Linux + Windows CI | Pass |
| MSI build + silent install/uninstall | Windows CI | Pass |
| Burn Setup.exe build | Windows CI | Pass (built; interactive run not exercised) |
| Portable mode (`portable.txt`) | Unit-tested resolver only | Partially verified |
| Native visual review, DPI/theme matrix | Windows desktop | Unverified |
| Release workflow end-to-end (tag) | GitHub | Unverified: tag push is blocked in the authoring sandbox |

Findings fixed: `main` did not compile (egui 0.28 API); clippy style findings; lockfile missing; no window-state persistence; missing tooltips; no portable mode or installers.

Known limitations
- Unsigned binaries (SmartScreen warning); x64 only.
- Window position is only sanity-checked, not validated against monitors.
- Uninstall keeps user data under `%APPDATA%\RustMarks`.
- No network favicon fetching (by design); JSON snapshots with no retention limit; no arbitrary folder drag-and-drop; very large libraries untested for performance.
- Charter items not met: UI is egui, not pure Win32 (accepted trade-off for this project); idle CPU/memory budgets not measured; no `--headless/--json` CLI.
