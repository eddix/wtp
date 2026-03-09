//! Worktree data models and management

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a worktree entry
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorktreeId(String);

impl WorktreeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

}

impl Default for WorktreeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorktreeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Reference to a git repository - can be relative to a host or absolute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoRef {
    /// Repository referenced by host alias and relative path
    /// e.g., host="gh", path="abc/def" => $HOME/codes/github.com/abc/def
    Hosted {
        host: String,
        path: String,
    },
    /// Absolute path to the repository
    Absolute {
        path: PathBuf,
    },
}

impl RepoRef {
    /// Convert to absolute path using host mappings
    pub fn to_absolute_path(&self, hosts: &std::collections::HashMap<String, PathBuf>) -> PathBuf {
        match self {
            RepoRef::Hosted { host, path } => {
                if let Some(host_root) = hosts.get(host) {
                    host_root.join(path)
                } else {
                    // Fallback to treating as absolute if host not found
                    PathBuf::from(path)
                }
            }
            RepoRef::Absolute { path } => path.clone(),
        }
    }

    /// Get the display representation (for status output)
    pub fn display(&self) -> String {
        match self {
            RepoRef::Hosted { host, path } => format!("{}:{}", host, path),
            RepoRef::Absolute { path } => path.display().to_string(),
        }
    }

    /// Get just the slug name from the path (last component)
    pub fn slug(&self) -> String {
        let path = match self {
            RepoRef::Hosted { path, .. } => PathBuf::from(path),
            RepoRef::Absolute { path } => path.clone(),
        };
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }
}

/// Entry representing a single worktree in a workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeEntry {
    /// Unique identifier
    pub id: WorktreeId,
    /// Reference to the original repository
    pub repo: RepoRef,
    /// Branch name
    pub branch: String,
    /// Path to the worktree directory (relative to workspace root)
    pub worktree_path: PathBuf,
    /// Base reference used when creating this worktree (optional)
    pub base: Option<String>,
    /// HEAD commit at the time of creation
    pub head_commit: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Local>,
}

impl WorktreeEntry {
    pub fn new(
        repo: RepoRef,
        branch: String,
        worktree_path: PathBuf,
        base: Option<String>,
        head_commit: Option<String>,
    ) -> Self {
        Self {
            id: WorktreeId::new(),
            repo,
            branch,
            worktree_path,
            base,
            head_commit,
            created_at: Local::now(),
        }
    }

}

/// The worktree.toml file structure stored in .wtp/ directory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorktreeToml {
    /// Version of the file format
    pub version: String,
    /// List of worktrees in this workspace
    pub worktrees: Vec<WorktreeEntry>,
}

impl WorktreeToml {
    pub fn new() -> Self {
        Self {
            version: "1".to_string(),
            worktrees: Vec::new(),
        }
    }

    /// Load from a file path
    pub fn load(path: &std::path::Path) -> crate::core::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save to a file path
    pub fn save(&self, path: &std::path::Path) -> crate::core::Result<()> {
        let content = toml::to_string_pretty(self)?;
        crate::core::fence::global_fence()
            .map(|f| f.write(path, &content))
            .unwrap_or_else(|| std::fs::write(path, content).map_err(|e| e.into()))?;
        Ok(())
    }
    
    /// Save to a file path with explicit fence check
    pub fn save_with_fence(&self, path: &std::path::Path, fence: &crate::core::fence::Fence) -> crate::core::Result<()> {
        let content = toml::to_string_pretty(self)?;
        fence.write(path, content)?;
        Ok(())
    }

    /// Add a new worktree entry
    pub fn add_worktree(&mut self, entry: WorktreeEntry) {
        self.worktrees.push(entry);
    }

    /// Find a worktree by repo (any branch)
    pub fn find_by_repo(&self, repo: &RepoRef) -> Option<&WorktreeEntry> {
        self.worktrees.iter().find(|w| w.repo == *repo)
    }

    /// Find a worktree by repo slug (last component of the path)
    pub fn find_by_slug(&self, slug: &str) -> Option<&WorktreeEntry> {
        self.worktrees
            .iter()
            .find(|w| w.repo.slug() == slug || w.repo.display() == slug)
    }

    /// Remove a worktree entry by repo slug. Returns true if an entry was removed.
    /// Errors if multiple worktrees match the slug — use the full display name instead.
    pub fn remove_by_slug(&mut self, slug: &str) -> std::result::Result<bool, String> {
        let matches: Vec<_> = self
            .worktrees
            .iter()
            .filter(|w| w.repo.slug() == slug || w.repo.display() == slug)
            .collect();
        if matches.len() > 1 {
            let names: Vec<_> = matches.iter().map(|w| w.repo.display()).collect();
            return Err(format!(
                "Multiple worktrees match '{}': {}. Use the full name to be specific.",
                slug,
                names.join(", ")
            ));
        }
        let before = self.worktrees.len();
        self.worktrees
            .retain(|w| w.repo.slug() != slug && w.repo.display() != slug);
        Ok(self.worktrees.len() < before)
    }
}

/// Manager for worktree operations
pub struct WorktreeManager {
    config: WorktreeToml,
    config_path: PathBuf,
}

impl WorktreeManager {
    pub fn load(workspace_root: &std::path::Path) -> crate::core::Result<Self> {
        let config_path = workspace_root.join(".wtp").join("worktree.toml");
        let config = WorktreeToml::load(&config_path)?;
        Ok(Self {
            config,
            config_path,
        })
    }

    pub fn save(&self) -> crate::core::Result<()> {
        crate::core::fence::global_fence()
            .map(|f| self.config.save_with_fence(&self.config_path, f))
            .unwrap_or_else(|| self.config.save(&self.config_path))
    }
    
    pub fn config(&self) -> &WorktreeToml {
        &self.config
    }

    /// Generate a unique worktree path for a repo
    /// Format: <repo_slug>/
    pub fn generate_worktree_path(&self, repo_slug: &str) -> PathBuf {
        PathBuf::from(repo_slug)
    }

    /// Get all worktrees
    pub fn list_worktrees(&self) -> &[WorktreeEntry] {
        &self.config.worktrees
    }

    /// Add a worktree entry
    pub fn add_worktree(&mut self, entry: WorktreeEntry) -> crate::core::Result<()> {
        self.config.add_worktree(entry);
        self.save()?;
        Ok(())
    }

    /// Remove a worktree entry by slug and save. Returns true if an entry was removed.
    pub fn remove_worktree(&mut self, slug: &str) -> crate::core::Result<bool> {
        let removed = self.config.remove_by_slug(slug)
            .map_err(crate::core::error::WtpError::config)?;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }
}
