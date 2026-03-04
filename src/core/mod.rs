//! Core module containing all business logic
//!
//! This module is independent of the CLI/TUI interface and can be used
//! programmatically or tested in isolation.

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
