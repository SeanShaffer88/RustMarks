# Development and release verification gate

Keep follow-up changes uncommitted until the checks below are completed and reviewed. Do not open a PR while a build is known to be failing or verification blockers remain unexplained.

## Automated checks

- [x] Rust syntax and formatting via rustfmt 1.88.
- [x] Whitespace checks (`git diff --check`).
- [x] Four real, dependency-free grid-navigation unit tests via `rustc --test src/grid.rs`.
- [x] Seven standalone source-revision tests via `rustc --test src/revision.rs` (Linux; six apply on Windows).
- [x] Six CI-diagnostic-runner regression tests via `python -m unittest discover -s scripts -p "test_*.py"`.
- [x] Manifest metadata validation (`cargo metadata --no-deps --format-version 1`).
- [ ] Generate and retain `Cargo.lock` using stable Rust with registry access.
- [ ] `cargo test --all-targets` (integration tests, grid tests and headless egui panel smoke test).
- [ ] `cargo clippy --all-targets`; address meaningful warnings.
- [ ] `cargo build --release` on Windows.
- [ ] Diagnose the **failed earlier revision's CI run**, not just formatting success.

### Current environment blockers

The sandbox has no preinstalled Rust toolchain. Rustc/rustfmt/cargo 1.88 were provisioned outside the repository from npm's `@rustbin` packages. A full test attempt failed while downloading `https://index.crates.io/config.json` with a TLS connection error; outbound access does not include the crates registry/download hosts. The earlier merged revision's GitHub Windows job failed in `cargo test --all-targets`. GitHub job status/annotations are readable, but the detailed log download redirects to `results-receiver.actions.githubusercontent.com`, outside the sandbox's allowed hosts. Do not interpret these constraints as proof of compilation success.

The current pass retried `cargo test --all-targets`: it failed before compilation fetching the registry configuration for the `base64` dependency with the same TLS error. No integration test result is inferred from standalone tests. The new CI diagnostic runner is designed to publish bounded failure output as check annotations (retrievable through the GitHub API), preserve the gate's actual exit code, and retain full logs / a generated lockfile as artifacts. Its six local regression tests pass; its workflow integration still requires a real CI run after an authorized commit.

No toolchain binaries, build artifacts, temporary test harnesses, or credentials belong in the repository.

## Native Windows smoke tests

- [ ] Launch release executable; resize to the minimum window and test all panels / scroll areas.
- [ ] Open `examples/bookmarks.html`, native JSON and an actual Chrome/Edge export.
- [ ] Confirm that saving a Chrome import asks for a separate working file and cannot overwrite the source.
- [ ] Edit all grid columns, filter with `tag:dev -broken`, navigate Up/Down/Enter/Tab/Shift+Tab across virtualized rows.
- [ ] Ctrl/Shift row selection; bulk tag, URL cleanup, delete, and move to a nested folder.
- [ ] Rename/delete folders; verify undo/redo, saved-file dirty state and snapshot safety.
- [ ] Import and preview PNG/JPEG/ICO/GIF favicons; reject invalid/oversized images without losing prior state.
- [ ] Validate/save custom rules, restart, rescan suggestions; ensure invalid JSON preserves active rules.
- [ ] Accept individual/all categorization suggestions; review stable-identity moves in both diff modes.
- [ ] Merge multiple HTML/JSON files; test all conflict resolutions and duplicates within one input.
- [ ] Modify the working copy after merge preview and confirm that stale apply is refused.
- [ ] Compare side-by-side trees; synchronize folder collapse state; inspect note/tag/favicon-only changes.
- [ ] Save JSON and HTML twice; reopen and compare metadata / folder hierarchy.
- [ ] Modify an opened file externally (including a same-length change); saving must refuse without modifying the disk or clearing dirty state. Save to a distinct path and verify both files.
- [ ] Delete or retarget the source after opening; saving must not silently recreate or overwrite it. Confirm browser-source alias protection and symlink-target saves.
- [ ] Cleanup URLs with fragment routes, duplicate query keys, `%20` / `+` encoding, signed query parameters and non-web schemes; meaningful bytes must remain intact.
- [ ] Re-merge the same metadata several times; notes/tags must not duplicate, and intentionally empty import folders must survive.
- [ ] Inspect snapshots without changing working state, restore one, then restore a newer state.
- [ ] Cancel native dialogs, revert prompt and unsaved-close prompt; verify no unintended changes.
- [ ] Simulate write/snapshot failure and verify originals survive with a clear error message.
- [ ] Test a realistically large bookmark collection for UI latency and memory use.

## Before handing off for a commit or PR

Document actual command outputs and Windows QA results. Correct the implementation if any test fails. Update this checklist and README verification status only with evidence. User approval to commit or open a PR is still required; work is automatically saved on the active Arena branch.
