//! Worktree data models and management

use chrono::{DateTime, Local};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing;

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
    Hosted { host: String, path: String },
    /// Absolute path to the repository
    Absolute { path: PathBuf },
}

impl RepoRef {
    /// Convert to absolute path using host mappings
    pub fn to_absolute_path(&self, hosts: &IndexMap<String, crate::config::HostConfig>) -> PathBuf {
        match self {
            RepoRef::Hosted { host, path } => {
                if let Some(host_config) = hosts.get(host) {
                    host_config.root.join(path)
                } else {
                    tracing::warn!("Host '{}' not found, treating path as relative", host);
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

    /// Case-insensitive substring match against the repo reference.
    ///
    /// Matches against the full display form (`host:path` for hosted repos,
    /// the absolute path for absolute repos), which already contains the slug.
    /// This lets a pattern like `i18n` match `byted:abc/i18n_sdk` as well as
    /// a path-level namespace like `byted:i18n/web`.
    pub fn matches(&self, pattern: &str) -> bool {
        self.display()
            .to_lowercase()
            .contains(&pattern.to_lowercase())
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn load(path: &std::path::Path) -> crate::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save to a file path
    pub fn save(&self, path: &std::path::Path) -> crate::Result<()> {
        let content = toml::to_string_pretty(self)?;
        match crate::fence::global_fence() {
            Some(f) => f.write(path, &content)?,
            None => {
                return Err(crate::error::WtpError::config(
                    "Security fence not initialized. Cannot write without boundary protection.",
                ));
            }
        }
        Ok(())
    }

    /// Save to a file path with explicit fence check
    pub fn save_with_fence(
        &self,
        path: &std::path::Path,
        fence: &crate::fence::Fence,
    ) -> crate::Result<()> {
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

    /// Whether any worktree's repo matches `pattern` (case-insensitive substring).
    pub fn has_repo_matching(&self, pattern: &str) -> bool {
        // Lower the pattern once here instead of per-worktree inside `matches`.
        let pattern = pattern.to_lowercase();
        self.worktrees
            .iter()
            .any(|w| w.repo.display().to_lowercase().contains(&pattern))
    }

    /// Find a worktree by repo slug (last component of the path)
    pub fn find_by_slug(&self, slug: &str) -> Option<&WorktreeEntry> {
        self.worktrees
            .iter()
            .find(|w| w.repo.slug() == slug || w.repo.display() == slug)
    }

    /// Remove a worktree entry by repo slug. Returns true if an entry was removed.
    /// Errors if multiple worktrees match the slug — use the full display name instead.
    pub fn remove_by_slug(&mut self, slug: &str) -> crate::Result<bool> {
        let matches: Vec<_> = self
            .worktrees
            .iter()
            .filter(|w| w.repo.slug() == slug || w.repo.display() == slug)
            .collect();
        if matches.len() > 1 {
            let names: Vec<_> = matches.iter().map(|w| w.repo.display()).collect();
            return Err(crate::error::WtpError::config(format!(
                "Multiple worktrees match '{}': {}. Use the full name to be specific.",
                slug,
                names.join(", ")
            )));
        }
        let before = self.worktrees.len();
        self.worktrees
            .retain(|w| w.repo.slug() != slug && w.repo.display() != slug);
        Ok(self.worktrees.len() < before)
    }
}

impl Default for WorktreeToml {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for worktree operations
pub struct WorktreeManager {
    config: WorktreeToml,
    config_path: PathBuf,
}

impl WorktreeManager {
    pub fn load(workspace_root: &std::path::Path) -> crate::Result<Self> {
        let config_path = workspace_root.join(".wtp").join("worktree.toml");
        let config = WorktreeToml::load(&config_path)?;
        Ok(Self {
            config,
            config_path,
        })
    }

    pub fn save(&self) -> crate::Result<()> {
        match crate::fence::global_fence() {
            Some(f) => self.config.save_with_fence(&self.config_path, f),
            None => Err(crate::error::WtpError::config(
                "Security fence not initialized. Cannot write without boundary protection.",
            )),
        }
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
    pub fn add_worktree(&mut self, entry: WorktreeEntry) -> crate::Result<()> {
        self.config.add_worktree(entry);
        self.save()?;
        Ok(())
    }

    /// Remove a worktree entry by slug and save. Returns true if an entry was removed.
    pub fn remove_worktree(&mut self, slug: &str) -> crate::Result<bool> {
        let removed = self.config.remove_by_slug(slug)?;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Remove multiple worktree entries by slug and save once.
    /// Returns the number of entries actually removed.
    pub fn remove_many(&mut self, slugs: &[&str]) -> crate::Result<usize> {
        let mut removed = 0;
        for slug in slugs {
            if self.config.remove_by_slug(slug)? {
                removed += 1;
            }
        }
        if removed > 0 {
            self.save()?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosted(host: &str, path: &str) -> RepoRef {
        RepoRef::Hosted {
            host: host.to_string(),
            path: path.to_string(),
        }
    }

    #[test]
    fn matches_slug_substring() {
        let repo = hosted("byted", "abc/i18n_sdk");
        assert!(repo.matches("i18n"));
        assert!(repo.matches("sdk"));
        assert!(repo.matches("i18n_sdk"));
    }

    #[test]
    fn matches_is_case_insensitive() {
        let repo = hosted("byted", "abc/I18N_Web");
        assert!(repo.matches("i18n"));
        assert!(repo.matches("WEB"));
    }

    #[test]
    fn matches_path_level_namespace() {
        // Pattern hits a directory segment, not just the final slug.
        let repo = hosted("byted", "i18n/web");
        assert!(repo.matches("i18n"));
    }

    #[test]
    fn matches_host_prefix() {
        // display() includes the host alias, so it is matchable too.
        let repo = hosted("byted", "abc/web");
        assert!(repo.matches("byted"));
    }

    #[test]
    fn matches_absolute_path() {
        let repo = RepoRef::Absolute {
            path: PathBuf::from("/home/u/codes/i18n_sdk"),
        };
        assert!(repo.matches("i18n"));
        assert!(!repo.matches("nomatch"));
    }

    #[test]
    fn does_not_match_unrelated() {
        let repo = hosted("byted", "abc/payments");
        assert!(!repo.matches("i18n"));
    }

    #[test]
    fn has_repo_matching_scans_all_worktrees() {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(WorktreeEntry::new(
            hosted("byted", "abc/payments"),
            "main".to_string(),
            PathBuf::from("payments"),
            None,
            None,
        ));
        toml.add_worktree(WorktreeEntry::new(
            hosted("byted", "abc/i18n_sdk"),
            "main".to_string(),
            PathBuf::from("i18n_sdk"),
            None,
            None,
        ));
        assert!(toml.has_repo_matching("i18n"));
        assert!(toml.has_repo_matching("payments"));
        assert!(!toml.has_repo_matching("nope"));
    }

    #[test]
    fn has_repo_matching_empty_is_false() {
        let toml = WorktreeToml::new();
        assert!(!toml.has_repo_matching("anything"));
    }
}
