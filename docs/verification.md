# Development and release verification gate

Keep follow-up changes uncommitted until the checks below are completed and reviewed. Do not open a PR while a build is known to be failing or verification blockers remain unexplained.

## Automated checks

- [x] Rust syntax and formatting via rustfmt 1.88.
- [x] Whitespace checks (`git diff --check`).
- [x] Seven standalone explicit-review-state tests via `rustc --test src/review_state.rs`.
- [x] Six real, dependency-free grid-navigation unit tests via `rustc --test src/grid.rs`.
- [x] Seven standalone source-revision tests via `rustc --test src/revision.rs` (Linux; six apply on Windows).
- [x] Seven CI-diagnostic-runner regression tests via `python -m unittest discover -s scripts -p "test_*.py"`.
- [x] Manifest metadata validation (`cargo metadata --no-deps --format-version 1`).
- [x] Generate and retain `Cargo.lock` using stable Rust with registry access.
- [x] `cargo test --all-targets` (69 tests pass on Linux, 2026-11-07; integration tests, grid tests and headless egui panel smoke test).
- [x] `cargo clippy --all-targets`; 0 warnings on Linux, 2026-11-07.
- [ ] `cargo build --release` on Windows.
- [x] Diagnose the failed Windows CI compilation through its GitHub check annotation (E0502 in the tab bar).

### 2026-11-07 update

Registry access worked; the full Linux gate now passes (fmt, 69 tests, clippy, release build) after fixing an egui 0.28 `WidgetInfo::labeled` signature error. Windows CI and visual review remain pending. The "Current environment blockers" below are historical.

### Current environment blockers

The sandbox has no preinstalled Rust toolchain. Rustc/rustfmt/cargo 1.88 were provisioned outside the repository from npm's `@rustbin` packages. A full test attempt failed while downloading `https://index.crates.io/config.json` with a TLS connection error; outbound access does not include the crates registry/download hosts. The earlier merged revision's GitHub Windows job failed in `cargo test --all-targets`. GitHub job status/annotations are readable, but the detailed log download redirects to `results-receiver.actions.githubusercontent.com`, outside the sandbox's allowed hosts. Do not interpret these constraints as proof of compilation success.

The current pass retried `cargo test --all-targets`: it failed before compilation fetching the registry configuration for the `base64` dependency with the same TLS error. No integration test result is inferred from standalone tests.

### Diagnosed Windows build failure and corrections

The annotation for [Windows run 42391518278](https://github.com/SeanShaffer88/RustMarks/actions/runs/42391518278), check run `128459917946`, contains the actual compiler diagnostic:

```text
error[E0502]: cannot borrow `*self` as immutable because it is also borrowed as mutable
  src/editor.rs:1263:56 (merged revision)
  ui.selectable_value(&mut self.tab, Tab::Diff,
      format!("⇄  Changes{}", if self.dirty() { " ●" } else { "" }))
```

The mutable tab borrow and immutable whole-document borrow overlapped in one call. The tab bar now computes its owned/static label before borrowing the active tab, through a shared `editor_tabs` renderer. The same annotation reported a `float_literal_f32_fallback` warning in theme stroke construction; its width now uses an explicit `1.0_f32`.

Two new headless GUI regressions cover dirty-marker transitions through edit/undo/redo/save/revert and real pointer selection of all tabs in clean and modified documents. The existing seven-panel smoke test now renders the real tab bar too, closing the previous test-coverage omission. **These GUI tests require Cargo dependencies and have not run locally.** The reported source error is corrected, but a fresh Windows build is still required before marking the build gate passed.

### Locally reproduced regressions (red → green)

- `navigation_does_not_overflow_when_table_dimensions_are_large`: failed on the previous implementation with `attempt to multiply with overflow`; passes after replacing flattened index multiplication with coordinate-based navigation. An exhaustive small-table reference test also passes.
- `test_compiler_error_survives_noisy_output_after_the_diagnostic`: failed because a subsequent 250-line build log displaced `error[E0502]` and its source location from the annotation; passes after retaining bounded diagnostic-context blocks separately from the final output tail. Exit codes, escaping, full logs and annotation-injection protections remain covered.

Current standalone results: **6 grid tests + 7 source-revision tests + 7 Python CI-runner tests pass**. Formatting, manifest metadata and whitespace validation pass. The CI diagnostic workflow has now successfully exposed the real compiler error through the GitHub API; this pass's refined diagnostic logic still needs a fresh workflow run. Full integration tests, Clippy and a Windows release build remain unchecked.

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

## Current charcoal workspace and explicit merge-review development

The active branch is `arena/2933474c-rustmarks`. Its checkout contained the merged build fixes but not the earlier uncommitted visual draft. The reference-inspired charcoal workspace has been implemented here, with presentation separated into `ui_components.rs` and `merge_workspace.rs` and a testable, immutable-base merge session in the core library.

### Verified baseline versus current code

The earlier fixes passed Windows CI at `8598250140dd27552c812701db3b5b8706305dce`, [run 42393028363](https://github.com/SeanShaffer88/RustMarks/actions/runs/42393028363), including full tests, Clippy and a release build. This supersedes historical statements above about the old E0502 fix awaiting execution. It does **not** cover the current uncommitted UI/session changes.

- [x] New dependency-free decision-state tests: explicit decisions, clearing a decision, bulk choice, stale protection, invalid indices, no-conflict readiness and hidden-conflict gating.
- [x] Formatting, manifest metadata, whitespace checks, 20 standalone Rust tests and seven Python CI-runner tests.
- [ ] Execute seven new `tests/merge_review.rs` integration tests (including real example-file metrics).
- [ ] Execute new headless tests for the real shell at 1000×650 / 1440×900, filters, empty merge state and theme application.
- [ ] Execute application regressions for failed refresh retaining choices, unresolved/stale apply not snapshotting, failed snapshot retaining the ready review, and successful staging snapshotting the original library.
- [ ] Full Cargo tests, Clippy and Windows release build for this revision.
- [ ] Native visual review: charcoal surfaces, source cards, file-name truncation, menu/sidebar keyboard focus, dropdowns, filter/bulk controls, scrolling, Windows DPI scaling and both supported sizes.
- [ ] End-to-end Source A open / Source B selection / Refresh / Clear / Review Changes with the two example HTML files. Expected metrics: 5 existing, 4 incoming, 3 duplicates, 2 conflicts, 1 new, 6 candidate bookmarks.

Full Cargo testing was attempted again but failed fetching the registry configuration for `base64` from `index.crates.io` before compilation. Do not equate standalone review-state tests with a full merge-engine or GUI test pass. All new tests requiring Cargo dependencies remain pending. No commit, push or PR was made during this pass.
