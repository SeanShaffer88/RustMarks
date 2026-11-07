# Audit

Date: 2026-11-07. Scope: build and test verification of `main` at 5d6cad6 plus fixes in this change.

| Check | Platform | Result |
| --- | --- | --- |
| `cargo fmt --check` | Linux, rustc/cargo 1.97 | Pass |
| `cargo test --all-targets` | Linux | Pass (22 + 11 + 29 + 7) |
| `cargo clippy --all-targets` | Linux | Pass, 0 warnings (4 fixed) |
| `cargo build --release` | Linux | Pass |
| Windows release build | Windows | Pending: verified by CI on the PR |
| Native visual review | Windows | Unverified |

Findings
- `main` did not compile (E0061 in `ui_components.rs`); fixed.
- Style-only clippy findings; fixed.
- Known limitations are listed in README "Remaining verification constraints".
