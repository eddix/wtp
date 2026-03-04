//! Error types for wtp

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, WtpError>;

#[derive(Error, Debug)]
pub enum WtpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Workspace already exists: {name} at {path}")]
    WorkspaceAlreadyExists { name: String, path: PathBuf },

    #[error("Not in a git repository")]
    NotInGitRepo,

    #[error("Branch '{branch}' is already checked out in another worktree: {worktree_path}")]
    BranchAlreadyCheckedOut { branch: String, worktree_path: PathBuf },

    #[error("Worktree already exists: {path}")]
    WorktreeAlreadyExists { path: PathBuf },

    #[error("Serialization error: {0}")]
    Serialization(#[from] toml::ser::Error),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] toml::de::Error),
}

impl WtpError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn git(msg: impl Into<String>) -> Self {
        Self::Git(msg.into())
    }
}
