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

    /// Find a worktree by repo and branch
    pub fn find_by_repo_and_branch(&self, repo: &RepoRef, branch: &str) -> Option<&WorktreeEntry> {
        self.worktrees
            .iter()
            .find(|w| w.repo == *repo && w.branch == branch)
    }

    /// Whether any worktree's repo matches `pattern` (case-insensitive substring).
    pub fn has_repo_matching(&self, pattern: &str) -> bool {
        // Lower the pattern once here instead of per-worktree inside `matches`.
        let pattern = pattern.to_lowercase();
        self.worktrees
            .iter()
            .any(|w| w.repo.display().to_lowercase().contains(&pattern))
    }

    /// Resolve a worktree by key: the worktree directory name (exact, always
    /// unique within a workspace), the repo slug, or the repo display name.
    ///
    /// Directory name takes precedence. A slug/display key that matches
    /// multiple worktrees (same repo checked out on several branches) is
    /// ambiguous and returns an error listing the candidate directory names.
    pub fn find_by_slug(&self, key: &str) -> crate::Result<Option<&WorktreeEntry>> {
        if let Some(w) = self
            .worktrees
            .iter()
            .find(|w| w.worktree_path == std::path::Path::new(key))
        {
            return Ok(Some(w));
        }
        let matches: Vec<_> = self
            .worktrees
            .iter()
            .filter(|w| w.repo.slug() == key || w.repo.display() == key)
            .collect();
        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches[0])),
            _ => Err(Self::ambiguous_key_error(key, &matches)),
        }
    }

    /// Remove a worktree entry by key (directory name, repo slug, or display
    /// name — see [`Self::find_by_slug`]). Returns true if an entry was removed.
    /// Errors if a slug/display key matches multiple worktrees.
    pub fn remove_by_slug(&mut self, key: &str) -> crate::Result<bool> {
        if let Some(pos) = self
            .worktrees
            .iter()
            .position(|w| w.worktree_path == std::path::Path::new(key))
        {
            self.worktrees.remove(pos);
            return Ok(true);
        }
        let matches: Vec<_> = self
            .worktrees
            .iter()
            .filter(|w| w.repo.slug() == key || w.repo.display() == key)
            .collect();
        if matches.len() > 1 {
            return Err(Self::ambiguous_key_error(key, &matches));
        }
        let before = self.worktrees.len();
        self.worktrees
            .retain(|w| w.repo.slug() != key && w.repo.display() != key);
        Ok(self.worktrees.len() < before)
    }

    fn ambiguous_key_error(key: &str, matches: &[&WorktreeEntry]) -> crate::error::WtpError {
        let names: Vec<_> = matches
            .iter()
            .map(|w| format!("{} (branch: {})", w.worktree_path.display(), w.branch))
            .collect();
        crate::error::WtpError::config(format!(
            "Multiple worktrees match '{}': {}. Use the directory name to be specific.",
            key,
            names.join(", ")
        ))
    }

    /// Remove all worktree entries.
    pub fn clear(&mut self) {
        self.worktrees.clear();
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

    /// Generate a worktree path that encodes the branch name.
    /// Format: <repo_slug>@<branch>/ where the branch component is sanitized
    /// like a workspace name (e.g. `feature/x` becomes `feature_x`).
    ///
    /// Falls back to the plain slug if the branch sanitizes to nothing.
    pub fn generate_worktree_path_with_branch(&self, repo_slug: &str, branch: &str) -> PathBuf {
        let branch_part = crate::config::sanitize_workspace_name(branch);
        if branch_part.is_empty() {
            return PathBuf::from(repo_slug);
        }
        PathBuf::from(format!("{}@{}", repo_slug, branch_part))
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

    /// Remove a worktree entry by key (directory name, slug, or display name)
    /// and save. Returns true if an entry was removed.
    pub fn remove_worktree(&mut self, key: &str) -> crate::Result<bool> {
        let removed = self.config.remove_by_slug(key)?;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Remove all worktree entries and save once.
    pub fn remove_all(&mut self) -> crate::Result<()> {
        if self.config.worktrees.is_empty() {
            return Ok(());
        }
        self.config.clear();
        self.save()
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

    fn entry(repo: RepoRef, branch: &str, dir: &str) -> WorktreeEntry {
        WorktreeEntry::new(repo, branch.to_string(), PathBuf::from(dir), None, None)
    }

    /// Two branches of the same repo, as created with --with-branch-name.
    fn multi_branch_toml() -> WorktreeToml {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(entry(
            hosted("gh", "owner/myrepo"),
            "release-area-a-dev",
            "myrepo",
        ));
        toml.add_worktree(entry(
            hosted("gh", "owner/myrepo"),
            "release-area-b-dev",
            "myrepo@release-area-b-dev",
        ));
        toml
    }

    #[test]
    fn find_by_repo_and_branch_distinguishes_branches() {
        let toml = multi_branch_toml();
        let repo = hosted("gh", "owner/myrepo");
        let a = toml.find_by_repo_and_branch(&repo, "release-area-a-dev");
        assert_eq!(a.unwrap().worktree_path, PathBuf::from("myrepo"));
        let b = toml.find_by_repo_and_branch(&repo, "release-area-b-dev");
        assert_eq!(
            b.unwrap().worktree_path,
            PathBuf::from("myrepo@release-area-b-dev")
        );
        assert!(toml.find_by_repo_and_branch(&repo, "main").is_none());
    }

    #[test]
    fn find_by_slug_prefers_exact_directory_name() {
        let toml = multi_branch_toml();
        let found = toml
            .find_by_slug("myrepo@release-area-b-dev")
            .unwrap()
            .unwrap();
        assert_eq!(found.branch, "release-area-b-dev");
    }

    #[test]
    fn find_by_slug_ambiguous_slug_errors_with_directory_names() {
        let mut toml = multi_branch_toml();
        // Rename the first entry's dir so the bare slug matches no directory
        // and falls through to ambiguous slug matching.
        toml.worktrees[0].worktree_path = PathBuf::from("myrepo@release-area-a-dev");
        let err = toml.find_by_slug("myrepo").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("myrepo@release-area-a-dev"), "{}", msg);
        assert!(msg.contains("myrepo@release-area-b-dev"), "{}", msg);
    }

    #[test]
    fn find_by_slug_directory_name_wins_over_ambiguous_slug() {
        // First entry's dir is the bare slug — exact dir match resolves the
        // ambiguity instead of erroring.
        let toml = multi_branch_toml();
        let found = toml.find_by_slug("myrepo").unwrap().unwrap();
        assert_eq!(found.branch, "release-area-a-dev");
    }

    #[test]
    fn find_by_slug_unique_slug_still_works() {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(entry(hosted("gh", "owner/single"), "main", "single"));
        let found = toml.find_by_slug("single").unwrap().unwrap();
        assert_eq!(found.branch, "main");
        assert!(toml.find_by_slug("nope").unwrap().is_none());
    }

    #[test]
    fn remove_by_slug_directory_name_removes_only_that_entry() {
        let mut toml = multi_branch_toml();
        assert!(toml.remove_by_slug("myrepo@release-area-b-dev").unwrap());
        assert_eq!(toml.worktrees.len(), 1);
        assert_eq!(toml.worktrees[0].branch, "release-area-a-dev");
    }

    #[test]
    fn remove_by_slug_ambiguous_errors() {
        let mut toml = multi_branch_toml();
        toml.worktrees[0].worktree_path = PathBuf::from("myrepo@release-area-a-dev");
        let err = toml.remove_by_slug("myrepo").unwrap_err();
        assert!(err.to_string().contains("directory name"));
        assert_eq!(toml.worktrees.len(), 2);
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut toml = multi_branch_toml();
        toml.clear();
        assert!(toml.worktrees.is_empty());
    }

    #[test]
    fn generate_worktree_path_with_branch_formats() {
        let manager = WorktreeManager {
            config: WorktreeToml::new(),
            config_path: PathBuf::from("/tmp/worktree.toml"),
        };
        assert_eq!(
            manager.generate_worktree_path_with_branch("myrepo", "release-a-dev"),
            PathBuf::from("myrepo@release-a-dev")
        );
        // Branch names with separators are sanitized.
        assert_eq!(
            manager.generate_worktree_path_with_branch("myrepo", "feature/x"),
            PathBuf::from("myrepo@feature_x")
        );
        // Pathological branch names fall back to the plain slug.
        assert_eq!(
            manager.generate_worktree_path_with_branch("myrepo", "///"),
            PathBuf::from("myrepo")
        );
    }
}
