# Code Review Report

## Summary

The wtp project was restructured from a single crate into a Cargo workspace with 4 members: `wtp-core`, `wtp-cli`, `wtp-gui`, `wtp-derive`. The core separation is well-executed — dependencies are correctly split, path references are properly updated, and the extension trait pattern for CLI-specific formatting is clean.

**Overall assessment: REQUEST_CHANGES** — 2 blockers, 5 warnings found.

**Compilation:** `cargo check --workspace` passes (with expected dead_code warnings in wtp-gui scaffold).
**Tests:** `cargo test --workspace` passes (all 16 tests pass, 0 failures).

---

## Blockers (must fix before merge)

- [ ] **P1: `cargo install --path .` is broken** — The root `Cargo.toml` is now a virtual workspace manifest. `README.md:41`, `README_CN.md:41`, `INSTALL.md:84` all instruct `cargo install --path .` which now fails with `found a virtual manifest instead of a package manifest`. Must change to `cargo install --path wtp-cli` (or add `default-members` to workspace).

- [ ] **P1: New workspace crates are untracked** — `wtp-core/`, `wtp-cli/`, `wtp-gui/` are all untracked (`??` in git status). The committed diff only shows deletion of old files + workspace manifest change. A clean checkout from this branch will fail to build. These directories must be `git add`-ed and committed.

## Warnings (should fix)

- [ ] **P2: Old `src/` directory not fully removed** — `src/` still exists as an empty directory. While harmless, it should be removed to avoid confusion.

- [ ] **P2: Documentation references stale paths** — `README.md` (line ~555), `README_CN.md`, and `AGENTS.md` (line ~45, ~186) still reference the old `src/cli/`, `src/core/`, and `crate::core::` paths. These should be updated to reflect the new `wtp-core/src/`, `wtp-cli/src/cli/` structure.

- [ ] **P2: CI/install scripts need workspace-aware updates** — `install.sh:8` uses `cargo build --release` (builds everything including wtp-gui), `.github/workflows/release.yml:50,57` uses `cargo build --release --target ...` without `-p wtp`. These should target `-p wtp` explicitly, or the workspace should declare `default-members = ["wtp-cli"]` to avoid building the unfinished GUI crate in releases.

- [ ] **P2: wtp-gui silently swallows config errors** — `wtp-gui/src/state.rs:31-37` catches all `LoadedConfig::load()` errors and falls back to defaults. Parse/permission errors become invisible, potentially pointing the GUI at the wrong workspace root.

- [ ] **P2: `WorkspaceInfo` not re-exported from lib.rs** — `wtp-core/src/lib.rs` exports `WorkspaceManager` but not `WorkspaceInfo`. The GUI's `state.rs` accesses it via `wtp_core::workspace::WorkspaceInfo` (which works since the module is `pub`), but for API consistency it should be re-exported alongside `WorkspaceManager`.

## Low Priority (nice to have)

- [ ] **P3: Production `unwrap()` calls** — A few `unwrap()` calls exist in non-test code:
  - `wtp-core/src/config.rs:67` — `loaded_path.as_ref().unwrap()` (guarded by `found_paths.len() > 1` but still a panic point)
  - `wtp-cli/src/cli/fuzzy.rs:47,152` — `unwrap()` on skim results
  - `wtp-cli/src/cli/eject.rs:169` — `unwrap()` on path operation

- [ ] **P3: wtp-gui dead code warnings** — `ViewState`, `AppState`, `load()`, `refresh_workspaces()` are flagged as unused. Expected for scaffold, but consider `#[allow(dead_code)]` or feature-gating the state module until GPUI is wired up.

---

## Verified Items (all clean)

- [x] **Dependency correctness**: `wtp-core/Cargo.toml` contains NO CLI-specific deps (colored, clap, ratatui, crossterm, skim, anstyle, anstream)
- [x] **wtp-cli depends on wtp-core**: `wtp-cli/Cargo.toml:18` — `wtp-core = { path = "../wtp-core" }`
- [x] **No stale `crate::core::` paths** in `wtp-core/src/` or `wtp-cli/src/`
- [x] **GitStatusFormat extension trait** exists at `wtp-cli/src/cli/git_status_fmt.rs`, uses `wtp_core::git::GitStatus` correctly
- [x] **scan_git_repos** is in `wtp-core/src/git.rs:471` and referenced correctly from `wtp-cli/src/cli/fuzzy.rs:211`
- [x] **lib.rs exports** — `wtp-core/src/lib.rs` correctly exports `GlobalConfig`, `LoadedConfig`, `Result`, `GitClient`, `WorkspaceManager`, `WorktreeManager`, `RepoRef`, `WorktreeEntry`
- [x] **Cargo workspace** — Root `Cargo.toml` is a proper workspace definition with `members = ["wtp-core", "wtp-cli", "wtp-gui", "wtp-derive"]` and `resolver = "2"`
- [x] **wtp-gui structure** — All planned scaffold files created: `app.rs`, `state.rs`, `tray.rs`, `views/` (5 files), `components/` (2 files)
- [x] **Serialization** — `serde` with `Serialize`/`Deserialize` derives used properly in config types; `toml::to_string_pretty` for output
- [x] **Error handling** — `thiserror` for wtp-core errors, `anyhow` for CLI error propagation, proper `Result` chaining throughout
- [x] **Fence security module** — Correctly uses `crate::GlobalConfig` (not old `crate::core::config::GlobalConfig`)
- [x] **cargo check --workspace** — Passes
- [x] **cargo test --workspace** — All 16 tests pass

---

*Review generated by Codex (gpt-5.4) + manual verification on 2026-03-28*
