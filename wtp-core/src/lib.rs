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

pub use config::{GlobalConfig, LoadedConfig, sanitize_workspace_name};
pub use error::Result;
pub use git::{FullGitStatus, GitClient, GitStatus};
pub use workspace::{CreateResult, WorkspaceInfo, WorkspaceManager};
pub use worktree::{RepoRef, WorktreeEntry, WorktreeManager, WorktreeToml};
