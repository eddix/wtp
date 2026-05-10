# wtp Workspace Restructuring — Design Document

> Generated from source analysis. Every type, function, and path reference has been verified against the current codebase.

---

## Table of Contents

- [Phase 1: wtp-core Extraction](#phase-1-wtp-core-extraction)
  - [1.1 Root Cargo.toml (workspace definition)](#11-root-cargotoml-workspace-definition)
  - [1.2 wtp-core/Cargo.toml](#12-wtp-corecargotoml)
  - [1.3 wtp-cli/Cargo.toml](#13-wtp-clicargotoml)
  - [1.4 File Moves](#14-file-moves)
  - [1.5 crate::core:: → crate:: replacements in core files](#15-cratecore--crate-replacements-in-core-files)
  - [1.6 crate::core:: → wtp_core:: replacements in CLI files](#16-cratecore--wtp_core-replacements-in-cli-files)
  - [1.7 main.rs mod core removal](#17-mainrs-mod-core-removal)
  - [1.8 GitStatus formatting — extension trait migration](#18-gitstatus-formatting--extension-trait-migration)
  - [1.9 scan_git_repos migration to wtp-core](#19-scan_git_repos-migration-to-wtp-core)
  - [1.10 wtp-core/src/lib.rs](#110-wtp-coresrclibrs)
  - [1.11 Integration tests update](#111-integration-tests-update)
  - [1.12 Verification checklist](#112-verification-checklist)
- [Phase 2–5: wtp-gui](#phase-25-wtp-gui)
  - [2.1 wtp-gui/Cargo.toml](#21-wtp-guicargotoml)
  - [2.2 Source files overview](#22-source-files-overview)
  - [2.3 main.rs](#23-mainrs)
  - [2.4 state.rs — AppState](#24-staters--appstate)
  - [2.5 tray.rs — System tray](#25-trayrs--system-tray)
  - [2.6 app.rs — MainWindow](#26-apprs--mainwindow)
  - [2.7 Views](#27-views)
  - [2.8 Components](#28-components)

---

## Phase 1: wtp-core Extraction

### 1.1 Root Cargo.toml (workspace definition)

Replace the entire root `Cargo.toml` with:

```toml
[workspace]
members = ["wtp-core", "wtp-cli", "wtp-derive"]
resolver = "2"
```

> `wtp-gui` will be added to `members` in Phase 2.

### 1.2 wtp-core/Cargo.toml

```toml
[package]
name = "wtp-core"
version = "0.1.0"
edition = "2024"
authors = ["eddix <eli.tech.arm@gmail.com>"]
description = "Core library for wtp — WorkTree for Polyrepo"
license = "MIT"
repository = "https://github.com/eddix/wtp"
rust-version = "1.90"

[lib]
name = "wtp_core"
path = "src/lib.rs"

[dependencies]
# Serialization
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# Error handling
thiserror = "2.0"

# Async runtime
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "process", "io-util"] }

# Utilities
dirs = "5.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.11", features = ["v4", "serde"] }
shellexpand = "3.1"
walkdir = "2.5"
indexmap = { version = "2.6", features = ["serde"] }

[dev-dependencies]
tempfile = "3.13"
```

**Key decisions**:
- **No** `colored`, `anstyle`, `anstream` — presentation is CLI/GUI responsibility
- **No** `clap`, `wtp-derive` — CLI-only
- **No** `ratatui`, `crossterm`, `skim` — TUI/fuzzy-only
- **No** `anyhow` — core uses `thiserror`-based `WtpError`; CLI layers `anyhow` on top
- **Includes** `walkdir` — needed by `scan_git_repos` (moved from CLI to core, see §1.9)
- **Includes** `tokio` — needed by `WorkspaceManager::create_workspace` (async hook execution)

### 1.3 wtp-cli/Cargo.toml

```toml
[package]
name = "wtp"
version = "0.1.0"
edition = "2024"
authors = ["eddix <eli.tech.arm@gmail.com>"]
description = "WorkTree for Polyrepo - Manage multiple git worktrees across repositories"
license = "MIT"
repository = "https://github.com/eddix/wtp"
keywords = ["git", "worktree", "cli", "polyrepo", "workspace"]
categories = ["command-line-utilities", "development-tools"]
rust-version = "1.90"

[[bin]]
name = "wtp"
path = "src/main.rs"

[dependencies]
wtp-core = { path = "../wtp-core" }

# CLI framework
clap = { version = "4.5", features = ["derive", "env", "cargo"] }
wtp-derive = { path = "../wtp-derive" }

# Error handling
anyhow = "1.0"

# Async runtime
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "process", "io-util"] }

# TUI framework
ratatui = "0.30"
crossterm = "0.29"
skim = { version = "3.6", default-features = false, optional = true }

# Terminal output
colored = "2.1"
anstyle = "1.0"
anstream = "0.6"

# Utilities (used by CLI only: shellexpand for import path expansion)
shellexpand = "3.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
tempfile = "3.13"

[features]
default = ["fuzzy"]
fuzzy = ["skim"]
```

**Key decisions**:
- `name = "wtp"` — preserves the binary name for `cargo install`
- `shellexpand` duplicated — CLI uses it in `import.rs` for `--repo` path expansion; core also uses it in `config.rs`. Both need it.

### 1.4 File Moves

| Source | Destination | Notes |
|--------|-------------|-------|
| `src/core/mod.rs` | `wtp-core/src/lib.rs` | Rewritten (see §1.10) |
| `src/core/config.rs` | `wtp-core/src/config.rs` | Path fixes (see §1.5) |
| `src/core/error.rs` | `wtp-core/src/error.rs` | No changes needed |
| `src/core/fence.rs` | `wtp-core/src/fence.rs` | Path fixes (see §1.5) |
| `src/core/git.rs` | `wtp-core/src/git.rs` | Remove `colored`; extract format methods (see §1.8) |
| `src/core/workspace.rs` | `wtp-core/src/workspace.rs` | Path fixes (see §1.5) |
| `src/core/worktree.rs` | `wtp-core/src/worktree.rs` | Path fixes (see §1.5) |
| `src/main.rs` | `wtp-cli/src/main.rs` | Remove `mod core;` (see §1.7) |
| `src/cli/` (entire dir) | `wtp-cli/src/cli/` | Path fixes (see §1.6) |
| `tests/integration_test.rs` | `wtp-cli/tests/integration_test.rs` | No code changes (see §1.11) |

### 1.5 crate::core:: → crate:: replacements in core files

These are all replacements needed inside `wtp-core/src/` after the files are moved. Every `crate::core::X` becomes `crate::X` because the core module is now the crate root.

| File | Line | Old | New |
|------|------|-----|-----|
| `config.rs` | 8 | `use crate::core::error::{Result, WtpError};` | `use crate::error::{Result, WtpError};` |
| `fence.rs` | 6 | `use crate::core::error::{Result, WtpError};` | `use crate::error::{Result, WtpError};` |
| `fence.rs` | 43 | `config: &crate::core::GlobalConfig` | `config: &crate::GlobalConfig` |
| `fence.rs` | 207 | `config: &crate::core::GlobalConfig` | `config: &crate::GlobalConfig` |
| `git.rs` | 6 | `use crate::core::error::{Result, WtpError};` | `use crate::error::{Result, WtpError};` |
| `workspace.rs` | 3 | `use crate::core::config::{GlobalConfig, LoadedConfig, WTP_DIR};` | `use crate::config::{GlobalConfig, LoadedConfig, WTP_DIR};` |
| `workspace.rs` | 4 | `use crate::core::error::{Result, WtpError};` | `use crate::error::{Result, WtpError};` |
| `workspace.rs` | 5 | `use crate::core::fence::Fence;` | `use crate::fence::Fence;` |
| `workspace.rs` | 150 | `crate::core::worktree::WorktreeToml::new()` | `crate::worktree::WorktreeToml::new()` |
| `workspace.rs` | 163 | `crate::core::fence::ensure_fence(&self.loaded_config.config)` | `crate::fence::ensure_fence(&self.loaded_config.config)` |
| `workspace.rs` | 181 | `&HashMap<String, crate::core::config::HostConfig>` | `&HashMap<String, crate::config::HostConfig>` |
| `worktree.rs` | 141 | `crate::core::Result<Self>` | `crate::Result<Self>` |
| `worktree.rs` | 151 | `crate::core::Result<()>` | `crate::Result<()>` |
| `worktree.rs` | 153 | `crate::core::fence::global_fence()` | `crate::fence::global_fence()` |
| `worktree.rs` | 160 | `fence: &crate::core::fence::Fence) -> crate::core::Result<()>` | `fence: &crate::fence::Fence) -> crate::Result<()>` |
| `worktree.rs` | 213 | `crate::core::Result<Self>` | `crate::Result<Self>` |
| `worktree.rs` | 222 | `crate::core::Result<()>` | `crate::Result<()>` |
| `worktree.rs` | 223 | `crate::core::fence::global_fence()` | `crate::fence::global_fence()` |
| `worktree.rs` | 244 | `crate::core::Result<()>` | `crate::Result<()>` |
| `worktree.rs` | 251 | `crate::core::Result<bool>` | `crate::Result<bool>` |
| `worktree.rs` | 253 | `crate::core::error::WtpError::config` | `crate::error::WtpError::config` |

**Total: 21 replacements across 5 files.**

Also remove from `git.rs` line 7: `use colored::Colorize;` (see §1.8).

### 1.6 crate::core:: → wtp_core:: replacements in CLI files

These are all replacements needed inside `wtp-cli/src/` after the files are moved. Every `crate::core::X` becomes `wtp_core::X`.

| File | Line | Old | New |
|------|------|-----|-----|
| `cli/mod.rs` | 361 | `crate::core::LoadedConfig::load()?` | `wtp_core::LoadedConfig::load()?` |
| `cli/mod.rs` | 369 | `crate::core::fence::init_global_fence(...)` | `wtp_core::fence::init_global_fence(...)` |
| `cli/mod.rs` | 373 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 374 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 375 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 376 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 377 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 378 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 379 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 381 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 384 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/mod.rs` | 385 | `crate::core::WorkspaceManager::new(loaded_config)` | `wtp_core::WorkspaceManager::new(loaded_config)` |
| `cli/cd.rs` | 10 | `use crate::core::WorkspaceManager;` | `use wtp_core::WorkspaceManager;` |
| `cli/config.rs` | 3 | `use crate::core::WorkspaceManager;` | `use wtp_core::WorkspaceManager;` |
| `cli/create.rs` | 6 | `use crate::core::WorkspaceManager;` | `use wtp_core::WorkspaceManager;` |
| `cli/eject.rs` | 10 | `use crate::core::{GitClient, WorktreeManager, WorkspaceManager};` | `use wtp_core::{GitClient, WorktreeManager, WorkspaceManager};` |
| `cli/eject.rs` | 116 | `worktrees: &[crate::core::WorktreeEntry]` | `worktrees: &[wtp_core::WorktreeEntry]` |
| `cli/fuzzy.rs` | 6 | `use crate::core::WorkspaceManager;` | `use wtp_core::WorkspaceManager;` |
| `cli/host.rs` | 9 | `use crate::core::WorkspaceManager;` | `use wtp_core::WorkspaceManager;` |
| `cli/host.rs` | 95 | `crate::core::config::HostConfig { root: path_buf }` | `wtp_core::config::HostConfig { root: path_buf }` |
| `cli/import.rs` | 11–12 | `use crate::core::{ fence::Fence, GitClient, RepoRef, WorktreeEntry, WorktreeManager, WorkspaceManager, };` | `use wtp_core::{ fence::Fence, GitClient, RepoRef, WorktreeEntry, WorktreeManager, WorkspaceManager, };` |
| `cli/ls.rs` | 6 | `use crate::core::{GitClient, WorkspaceManager, WorktreeManager};` | `use wtp_core::{GitClient, WorkspaceManager, WorktreeManager};` |
| `cli/remove.rs` | 9 | `use crate::core::{GitClient, WorkspaceManager, WorktreeManager};` | `use wtp_core::{GitClient, WorkspaceManager, WorktreeManager};` |
| `cli/status.rs` | 8 | `use crate::core::{GitClient, WorktreeManager, WorkspaceManager};` | `use wtp_core::{GitClient, WorktreeManager, WorkspaceManager};` |
| `cli/status.rs` | 59 | `worktrees: &[crate::core::WorktreeEntry]` | `worktrees: &[wtp_core::WorktreeEntry]` |
| `cli/status.rs` | 106 | `worktrees: &[crate::core::WorktreeEntry]` | `worktrees: &[wtp_core::WorktreeEntry]` |
| `cli/switch.rs` | 10–11 | `use crate::core::{ fence::Fence, GitClient, RepoRef, WorktreeEntry, WorktreeManager, WorkspaceManager, };` | `use wtp_core::{ fence::Fence, GitClient, RepoRef, WorktreeEntry, WorktreeManager, WorkspaceManager, };` |

**Total: 27 replacements across 13 files.**

### 1.7 main.rs mod core removal

In `wtp-cli/src/main.rs` (originally `src/main.rs`):

**Remove** line 23:
```rust
mod core;
```

**Keep** line 22:
```rust
mod cli;
```

No other changes to `main.rs` — it does not reference `crate::core` directly. The `anstream` and `anstyle` imports on lines 19–20 stay as-is.

### 1.8 GitStatus formatting — extension trait migration

**Problem**: `GitStatus` (in `wtp-core/src/git.rs`) has three methods that use the `colored` crate:
- `format_compact()` (lines 451–485)
- `format_detail_status()` (lines 488–504)
- `format_detail_remote()` (lines 507–521)

`colored` is a terminal presentation concern and must not be in `wtp-core`.

**Solution**: Move these three methods to an extension trait in `wtp-cli`.

#### Step 1: Remove from `wtp-core/src/git.rs`

Delete the entire `impl GitStatus` block (lines 449–521) and the `use colored::Colorize;` import (line 7).

#### Step 2: Create `wtp-cli/src/cli/git_status_fmt.rs`

```rust
//! Terminal formatting extension for GitStatus
//!
//! Provides colored terminal output for git status information.
//! This is CLI-specific; GUI will use its own rendering.

use colored::Colorize;
use wtp_core::git::GitStatus;

/// Extension trait for CLI-specific formatting of GitStatus
pub trait GitStatusFormat {
    /// Format status as a compact colored string (for default `wtp status` / `wtp ls`)
    fn format_compact(&self) -> String;

    /// Format detailed status info (for `wtp status --long`)
    fn format_detail_status(&self) -> String;

    /// Format remote tracking info (for `wtp status --long`)
    fn format_detail_remote(&self) -> String;
}

impl GitStatusFormat for GitStatus {
    fn format_compact(&self) -> String {
        if !self.dirty && self.ahead == 0 && self.behind == 0 {
            return format!("{}", "\u{2713} clean".green());
        }

        let mut parts: Vec<String> = Vec::new();

        if self.dirty {
            let mut detail = Vec::new();
            if self.staged > 0 {
                detail.push(format!("{} staged", self.staged));
            }
            if self.unstaged > 0 {
                detail.push(format!("{} unstaged", self.unstaged));
            }
            if self.untracked > 0 {
                detail.push(format!("{} untracked", self.untracked));
            }
            let status_str = format!("* {}", detail.join(", "));
            parts.push(format!("{}", status_str.yellow()));
        }

        if self.ahead > 0 || self.behind > 0 {
            let mut remote_parts = Vec::new();
            if self.ahead > 0 {
                remote_parts.push(format!("{}", format!("+{}", self.ahead).green()));
            }
            if self.behind > 0 {
                remote_parts.push(format!("{}", format!("-{}", self.behind).red()));
            }
            parts.push(format!("({})", remote_parts.join(" ")));
        }

        parts.join("  ")
    }

    fn format_detail_status(&self) -> String {
        if !self.dirty {
            return format!("{}", "\u{2713} clean".green());
        }

        let mut detail = Vec::new();
        if self.staged > 0 {
            detail.push(format!("{} staged", self.staged));
        }
        if self.unstaged > 0 {
            detail.push(format!("{} unstaged", self.unstaged));
        }
        if self.untracked > 0 {
            detail.push(format!("{} untracked", self.untracked));
        }
        format!("{}", detail.join(", ").yellow())
    }

    fn format_detail_remote(&self) -> String {
        if self.ahead == 0 && self.behind == 0 {
            return format!("{}", "up to date".green());
        }

        let mut parts = Vec::new();
        if self.ahead > 0 {
            parts.push(format!("{}", format!("+{} ahead", self.ahead).green()));
        }
        if self.behind > 0 {
            parts.push(format!("{}", format!("-{} behind", self.behind).red()));
        }
        parts.join(", ")
    }
}
```

#### Step 3: Register the module and add use statements

In `wtp-cli/src/cli/mod.rs`, add:
```rust
pub mod git_status_fmt;
```

In every CLI file that calls `.format_compact()`, `.format_detail_status()`, or `.format_detail_remote()`, add:
```rust
use crate::cli::git_status_fmt::GitStatusFormat;
```

**Files that need the import**:

| File | Methods used | Lines using them |
|------|-------------|-----------------|
| `cli/status.rs` | `format_compact`, `format_detail_status`, `format_detail_remote` | 84, 186, 194 |
| `cli/ls.rs` | `format_compact` | 80 |
| `cli/remove.rs` | `format_detail_status` | 48, 83 |
| `cli/eject.rs` | `format_detail_status` | 81, 90 |

### 1.9 scan_git_repos migration to wtp-core

**Problem**: `scan_git_repos()` (in `src/cli/fuzzy.rs` lines 221–277) and its helper `is_bare_git_repo()` (lines 202–207) are pure business logic (directory scanning for git repos). GUI also needs this functionality.

**Solution**: Move both functions to `wtp-core/src/git.rs`.

#### Current signatures (from `src/cli/fuzzy.rs`):

```rust
/// Check if a directory looks like a bare git repository.
fn is_bare_git_repo(path: &Path) -> bool {
    !path.join(".git").exists()
        && path.join("HEAD").is_file()
        && path.join("objects").is_dir()
        && path.join("refs").is_dir()
}

/// Scan a directory for git repositories (normal and bare).
/// Returns paths relative to `root`.
pub fn scan_git_repos(root: &Path) -> Vec<String> {
    // uses walkdir::WalkDir
    // ...
}
```

#### Target location: `wtp-core/src/git.rs`

Add `use walkdir::WalkDir;` at the top (already in wtp-core's dependencies).

Move both functions as module-level public functions in `wtp-core/src/git.rs`:
- `pub fn is_bare_git_repo(path: &Path) -> bool`
- `pub fn scan_git_repos(root: &Path) -> Vec<String>`

#### Update CLI callers

In `wtp-cli/src/cli/fuzzy.rs`:
- **Remove** `is_bare_git_repo()` function (lines 202–207)
- **Remove** `scan_git_repos()` function (lines 221–277)
- **Replace** all calls to `scan_git_repos(...)` with `wtp_core::git::scan_git_repos(...)`

Callsites in `fuzzy.rs`:
- Line 293: `let repos = scan_git_repos(&host_root);` → `let repos = wtp_core::git::scan_git_repos(&host_root);`

In `wtp-cli/src/cli/import.rs`:
- Line (within `resolve_repo_interactively`): The function calls `fuzzy::resolve_repo_interactively` which internally calls `scan_git_repos`. No direct changes needed in `import.rs` — it goes through `fuzzy.rs`.

### 1.10 wtp-core/src/lib.rs

Replace the contents of `src/core/mod.rs` (which becomes `wtp-core/src/lib.rs`) with:

```rust
//! wtp-core: Core library for WorkTree for Polyrepo
//!
//! This crate contains all business logic independent of CLI/GUI interfaces.
//! It can be used programmatically or tested in isolation.

pub mod config;
pub mod error;
pub mod fence;
pub mod git;
pub mod workspace;
pub mod worktree;

pub use config::{GlobalConfig, LoadedConfig};
pub use error::Result;
pub use git::GitClient;
pub use workspace::WorkspaceManager;
pub use worktree::{RepoRef, WorktreeEntry, WorktreeManager};
```

This is functionally identical to the current `src/core/mod.rs` — no API change.

### 1.11 Integration tests update

`tests/integration_test.rs` → `wtp-cli/tests/integration_test.rs`

**No code changes needed.** The test file:
- Uses `env!("CARGO_BIN_EXE_wtp")` which resolves to the binary in the workspace target dir
- Does not import any `crate::core::` paths
- Only interacts with the `wtp` binary via `Command`

### 1.12 Verification checklist

After all changes:

1. `cargo build --workspace` — all three crates compile
2. `cargo test --workspace` — all existing tests pass
3. `cargo run -p wtp -- --help` — CLI help works
4. `cargo run -p wtp -- ls` — list workspaces
5. `cargo run -p wtp -- status` — show status (from a workspace dir)
6. `cargo install --path wtp-cli` — installs `wtp` binary

---

## Phase 2–5: wtp-gui

### 2.1 wtp-gui/Cargo.toml

```toml
[package]
name = "wtp-gui"
version = "0.1.0"
edition = "2024"
authors = ["eddix <eli.tech.arm@gmail.com>"]
description = "GUI application for wtp — WorkTree for Polyrepo"
license = "MIT"
repository = "https://github.com/eddix/wtp"
rust-version = "1.90"

[[bin]]
name = "wtp-gui"
path = "src/main.rs"

[dependencies]
wtp-core = { path = "../wtp-core" }

# GUI framework (pin to a specific Zed commit for API stability)
gpui = { git = "https://github.com/zed-industries/zed", package = "gpui", rev = "TBD_PIN_COMMIT_HASH" }

# System tray
tray-icon = "0.19"

# Async runtime
tokio = { version = "1.40", features = ["rt-multi-thread", "macros"] }

# Error handling
anyhow = "1.0"

# Utilities
smol = "2.0"  # for Timer in tray event polling
```

> **Action required**: Before implementation, determine the exact GPUI commit hash to pin. Run a test build against a recent Zed commit to verify API compatibility.

After creating `wtp-gui/`, update root `Cargo.toml`:
```toml
[workspace]
members = ["wtp-core", "wtp-cli", "wtp-gui", "wtp-derive"]
resolver = "2"
```

### 2.2 Source files overview

```
wtp-gui/
├── Cargo.toml
├── assets/
│   └── icon.png                  # 16x16 or 22x22 tray icon
└── src/
    ├── main.rs                   # Entry point: GPUI App + tray setup
    ├── app.rs                    # MainWindow view (top-level container)
    ├── tray.rs                   # System tray logic
    ├── state.rs                  # AppState (shared Entity)
    ├── views/
    │   ├── mod.rs
    │   ├── workspace_list.rs     # Workspace list panel
    │   ├── workspace_detail.rs   # Single workspace detail
    │   ├── create_workspace.rs   # Create workspace dialog
    │   ├── import_repo.rs        # Import repo (with search)
    │   └── config_panel.rs       # Configuration editor
    └── components/
        ├── mod.rs
        ├── search_input.rs       # Search/filter input
        └── status_badge.rs       # Git status badge
```

### 2.3 main.rs

```rust
//! wtp-gui entry point

mod app;
mod tray;
mod state;
mod views;
mod components;

use gpui::App;

fn main() {
    let app = App::new();
    app.run(|cx| {
        // Create shared application state
        let state = cx.new(|_| state::AppState::load());

        // Set up system tray
        tray::setup_tray(cx, state.clone());

        // Optionally open main window on first launch
        // app::open_main_window(cx, state.clone());
    });
}
```

**GPUI concepts used**:
- `App::new()` / `app.run()` — creates the GPUI application and enters the run loop
- `cx.new(|_| ...)` — creates an `Entity<AppState>` (GPUI's managed state container)
- `Entity<T>` — reference-counted handle to shared state, observable for changes

### 2.4 state.rs — AppState

```rust
//! Shared application state

use wtp_core::{LoadedConfig, WorkspaceManager};
use wtp_core::workspace::WorkspaceInfo;
use wtp_core::git::GitClient;

/// Global application state, wrapped in `Entity<AppState>` for GPUI reactivity
pub struct AppState {
    /// The loaded wtp configuration
    pub loaded_config: LoadedConfig,
    /// Cached workspace list
    pub workspaces: Vec<WorkspaceInfo>,
    /// Current navigation state
    pub current_view: ViewState,
    /// Loading indicator
    pub loading: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    WorkspaceList,
    WorkspaceDetail(String),  // workspace name
    CreateWorkspace,
    ImportRepo(String),       // workspace name
    Config,
}

impl AppState {
    /// Load initial state from wtp configuration
    pub fn load() -> Self {
        let (loaded_config, _warning) = LoadedConfig::load()
            .unwrap_or_else(|_| (LoadedConfig { config: Default::default(), source_path: None }, None));

        let manager = WorkspaceManager::new(loaded_config.clone());
        let workspaces = manager.list_workspaces();

        Self {
            loaded_config,
            workspaces,
            current_view: ViewState::WorkspaceList,
            loading: false,
        }
    }

    /// Refresh workspace list from disk
    pub fn refresh_workspaces(&mut self) {
        let manager = WorkspaceManager::new(self.loaded_config.clone());
        self.workspaces = manager.list_workspaces();
    }
}
```

**GPUI pattern**: `Entity<AppState>` is created once in `main.rs` and shared by all views. Any mutation via `entity.update(cx, |state, cx| { ... })` triggers observers to re-render.

### 2.5 tray.rs — System tray

```rust
//! System tray integration via tray-icon

use gpui::{App, Entity, Context, AsyncAppContext};
use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem}};
use crate::state::AppState;
use std::time::Duration;

/// Set up the system tray icon and menu
pub fn setup_tray(cx: &mut App, state: Entity<AppState>) {
    // Load icon from embedded bytes
    let icon = load_tray_icon();

    // Build initial menu
    let menu = build_tray_menu(cx, &state);

    // Create tray icon
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("wtp")
        .build()
        .expect("Failed to create tray icon");

    // Poll tray-icon events and dispatch to GPUI main thread
    cx.spawn(|mut cx: AsyncAppContext| async move {
        let receiver = MenuEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                cx.update(|cx| {
                    handle_tray_event(event, cx);
                }).ok();
            }
            smol::Timer::after(Duration::from_millis(100)).await;
        }
    }).detach();

    // Observe state changes to rebuild tray menu dynamically
    cx.observe(&state, |state, cx| {
        // Rebuild menu when workspaces change
        let menu = build_tray_menu(cx, &state);
        // Update tray menu (tray-icon API)
    }).detach();
}

fn build_tray_menu(cx: &App, state: &Entity<AppState>) -> Menu {
    let menu = Menu::new();
    // Read workspace list from state
    // Add workspace items
    // Add separator
    // Add "Open Dashboard...", "Create Workspace...", separator, "Quit"
    menu
}

fn handle_tray_event(event: MenuEvent, cx: &mut App) {
    // Match event.id against known menu item IDs
    // Dispatch to: open_main_window, navigate_to_create, quit
}

fn load_tray_icon() -> tray_icon::Icon {
    let icon_bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(icon_bytes).expect("Failed to load icon");
    let rgba = image.into_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
        .expect("Failed to create icon")
}
```

**Key integration point**: `tray-icon` uses `MenuEvent::receiver()` which returns a `std::sync::mpsc::Receiver`. We poll it in a `cx.spawn()` async task and bridge events to the GPUI main thread via `cx.update()`.

### 2.6 app.rs — MainWindow

```rust
//! Main application window

use gpui::*;
use crate::state::{AppState, ViewState};

/// Top-level window container with sidebar navigation
pub struct MainWindow {
    state: Entity<AppState>,
}

impl MainWindow {
    pub fn new(state: Entity<AppState>) -> Self {
        Self { state }
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .size_full()
            .child(self.render_sidebar(cx))
            .child(self.render_content(state, cx))
    }
}

impl MainWindow {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(200.0))
            .flex_col()
            .bg(rgb(0x1e1e2e))
            // Sidebar items: Spaces, Config
    }

    fn render_content(&self, state: &AppState, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .child(match &state.current_view {
                ViewState::WorkspaceList => {
                    // views::workspace_list::WorkspaceListView
                    div().child("Workspace List")
                }
                ViewState::WorkspaceDetail(name) => {
                    div().child(format!("Detail: {}", name))
                }
                ViewState::CreateWorkspace => {
                    div().child("Create Workspace")
                }
                ViewState::ImportRepo(ws) => {
                    div().child(format!("Import to: {}", ws))
                }
                ViewState::Config => {
                    div().child("Config")
                }
            })
    }
}

/// Open or focus the main window
pub fn open_main_window(cx: &mut App, state: Entity<AppState>) {
    let window_options = WindowOptions {
        // title, size, etc.
        ..Default::default()
    };
    cx.open_window(window_options, |cx| {
        cx.new(|_| MainWindow::new(state))
    }).unwrap();
}
```

**GPUI View pattern**:
- `struct MainWindow` holds `Entity<AppState>`
- `impl Render for MainWindow` defines the UI tree
- `self.state.read(cx)` gets immutable access to state
- `self.state.update(cx, |state, cx| { ... })` for mutations
- `div()`, `.flex()`, `.child()` etc. are GPUI element builders

### 2.7 Views

Each view follows the same pattern:

```rust
pub struct WorkspaceListView {
    state: Entity<AppState>,
}

impl Render for WorkspaceListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        // Render workspace list from state.workspaces
    }
}
```

**Key wtp-core API calls per view**:

| View | wtp-core APIs | Async? |
|------|--------------|--------|
| `WorkspaceListView` | `WorkspaceManager::list_workspaces()` | No |
| `WorkspaceDetailView` | `WorktreeManager::load(path)`, `GitClient::get_status(path)`, `GitClient::get_ahead_behind(path, base)`, `GitClient::get_head_commit(path)`, `GitClient::get_last_commit_subject(path)`, `GitClient::get_last_commit_relative_time(path)`, `GitClient::get_stash_count(path)` | Yes (spawn) |
| `CreateWorkspaceView` | `WorkspaceManager::create_workspace(name, run_hook)` | Yes (async) |
| `ImportRepoView` | `wtp_core::git::scan_git_repos(root)`, `GitClient::is_in_git_repo(path)`, `GitClient::get_repo_root(path)`, `GitClient::branch_exists(path, branch)`, `GitClient::create_worktree_with_branch(...)`, `GitClient::add_worktree_for_branch(...)`, `WorktreeManager::add_worktree(entry)` | Yes (spawn) |
| `ConfigPanelView` | `LoadedConfig::load()`, `LoadedConfig::save()`, `GlobalConfig` field access | No (except save) |

**Async pattern for all I/O operations**:

```rust
// Inside a view event handler:
let state = self.state.clone();
cx.spawn(|this, mut cx| async move {
    // Do I/O (git commands, file system) off main thread
    let result = some_async_operation().await;

    // Update state on main thread
    cx.update(|cx| {
        state.update(cx, |state, cx| {
            // Apply result to state
            state.refresh_workspaces();
            cx.notify(); // Trigger re-render
        });
    }).ok();
}).detach();
```

### 2.8 Components

#### `status_badge.rs`

Renders git status as colored badges instead of terminal escape codes:

```rust
pub struct StatusBadge {
    pub status: wtp_core::git::GitStatus,
}

impl Render for StatusBadge {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if !self.status.dirty && self.status.ahead == 0 && self.status.behind == 0 {
            return div()
                .child("\u{2713} clean")
                .text_color(rgb(0xa6e3a1)); // green
        }

        div().flex().gap(px(4.0))
            .when(self.status.staged > 0, |d| {
                d.child(
                    div().child(format!("{} staged", self.status.staged))
                         .text_color(rgb(0xf9e2af)) // yellow
                )
            })
            .when(self.status.unstaged > 0, |d| {
                d.child(
                    div().child(format!("{} unstaged", self.status.unstaged))
                         .text_color(rgb(0xf9e2af))
                )
            })
            .when(self.status.untracked > 0, |d| {
                d.child(
                    div().child(format!("{} untracked", self.status.untracked))
                         .text_color(rgb(0xf9e2af))
                )
            })
    }
}
```

This demonstrates **why** `colored` must not be in `wtp-core` — the GUI uses GPUI's element API for styling, not terminal escape codes.

#### `search_input.rs`

For the ImportRepo view, provides a text input with filtering:

```rust
pub struct SearchInput {
    query: String,
    on_change: Box<dyn Fn(&str)>,
}
```

> Implementation depends on GPUI's text input component maturity. May use `gpui-component` crate if available.

---

## ADR-001: Separate formatting from core data types

### Status
Accepted

### Context
`GitStatus` in `wtp-core` had terminal-colored `format_*` methods coupled to the `colored` crate. GUI needs different rendering.

### Decision
Extract format methods to an extension trait `GitStatusFormat` in `wtp-cli`. Core provides raw data only.

### Consequences
- **Easier**: GUI can implement its own rendering without pulling in terminal deps
- **Harder**: CLI files need an extra `use` import for the trait

---

## ADR-002: scan_git_repos lives in wtp-core

### Status
Accepted

### Context
`scan_git_repos()` scans directories for git repos — pure I/O logic with no presentation. Both CLI (fuzzy import) and GUI (import dialog) need it.

### Decision
Move `scan_git_repos()` and `is_bare_git_repo()` from `src/cli/fuzzy.rs` to `wtp-core/src/git.rs`.

### Consequences
- **Easier**: GUI gets repo scanning out of the box; no code duplication
- **Harder**: `walkdir` added to wtp-core's deps (small, no transitive bloat)

---

## ADR-003: tokio stays in wtp-core

### Status
Accepted

### Context
`WorkspaceManager::create_workspace()` is `async` because it runs hook scripts via `tokio::process::Command`. Removing tokio from core would require making the function synchronous (blocking the thread during hook execution) or splitting the hook execution out.

### Decision
Keep `tokio` in wtp-core. The async interface is cleaner and both CLI and GUI already use async runtimes.

### Consequences
- **Easier**: No API breakage; hook execution stays non-blocking
- **Harder**: wtp-core has a heavier dependency (tokio), but it's already needed by both consumers
