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
    /// Parent ref for stacked worktrees. Resolved preferentially as the
    /// branch of another worktree of the same repo in this workspace
    /// (forming a chain); otherwise treated as a plain git ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Fork point: commit of `parent` recorded at layer creation and
    /// updated after each successful restack. `wtp restack` rebases with
    /// `git rebase --onto <parent> <parent_head>` so only commits unique
    /// to this layer are replayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_head: Option<String>,
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
            parent: None,
            parent_head: None,
        }
    }

    /// Attach stacked-worktree parent information to this entry.
    pub fn with_parent(mut self, parent: String, parent_head: Option<String>) -> Self {
        self.parent = Some(parent);
        self.parent_head = parent_head;
        self
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

    /// Resolve `entry.parent` as another worktree of the same repository in
    /// this workspace (a stack layer). Returns `None` when the entry has no
    /// parent or the parent is a plain git ref rather than a layer.
    pub fn resolve_parent_layer(&self, entry: &WorktreeEntry) -> Option<&WorktreeEntry> {
        let parent = entry.parent.as_deref()?;
        self.worktrees
            .iter()
            .find(|w| w.repo == entry.repo && w.branch == parent)
    }

    /// Worktrees of the same repository whose parent is `entry`'s branch
    /// (its direct stack children).
    pub fn children_of(&self, entry: &WorktreeEntry) -> Vec<&WorktreeEntry> {
        self.worktrees
            .iter()
            .filter(|w| w.repo == entry.repo && w.parent.as_deref() == Some(&entry.branch))
            .collect()
    }

    /// Whether setting `entry`'s parent to `new_parent` would create a cycle
    /// among the stack layers of the same repository. Walks up the layer
    /// chain starting at `new_parent`; reaching `entry` again is a cycle.
    pub fn would_create_cycle(&self, entry: &WorktreeEntry, new_parent: &str) -> bool {
        let mut current = new_parent.to_string();
        // Bounded by the worktree count: walking longer than that means the
        // existing chain already contains a cycle — refuse to extend it.
        for _ in 0..=self.worktrees.len() {
            if current == entry.branch {
                return true;
            }
            let layer = self
                .worktrees
                .iter()
                .find(|w| w.repo == entry.repo && w.branch == current);
            match layer.and_then(|w| w.parent.clone()) {
                Some(next) => current = next,
                None => return false,
            }
        }
        true
    }

    /// All worktrees in the stack chain containing `entry`: the chain's root
    /// plus every descendant, in parents-first order. For an entry with no
    /// layer parent and no children this is just the entry itself.
    pub fn chain_of(&self, entry: &WorktreeEntry) -> Vec<&WorktreeEntry> {
        // Walk up to the chain root (bounded — parent cycles terminate).
        let mut root = match self.find_by_path(&entry.worktree_path) {
            Some(e) => e,
            None => return Vec::new(),
        };
        for _ in 0..=self.worktrees.len() {
            match self.resolve_parent_layer(root) {
                Some(p) => root = p,
                None => break,
            }
        }
        let root_path = root.worktree_path.clone();
        self.stacked_order()
            .into_iter()
            .filter(|(e, _)| self.chain_root_of(e).worktree_path == root_path)
            .map(|(e, _)| e)
            .collect()
    }

    fn find_by_path(&self, path: &std::path::Path) -> Option<&WorktreeEntry> {
        self.worktrees.iter().find(|w| w.worktree_path == path)
    }

    fn chain_root_of<'a>(&'a self, entry: &'a WorktreeEntry) -> &'a WorktreeEntry {
        let mut current = entry;
        for _ in 0..=self.worktrees.len() {
            match self.resolve_parent_layer(current) {
                Some(p) => current = p,
                None => break,
            }
        }
        current
    }

    /// Order worktrees for stacked display and restack: parents come before
    /// children (DFS), and each entry carries its stack depth. Entries whose
    /// parent is not a layer in this workspace (including entries with no
    /// parent at all) are roots at depth 0. Cycles are broken by treating
    /// already-visited entries as terminals, so this always returns every
    /// worktree exactly once.
    pub fn stacked_order(&self) -> Vec<(&WorktreeEntry, usize)> {
        let mut ordered = Vec::with_capacity(self.worktrees.len());
        let mut visited = vec![false; self.worktrees.len()];

        // Depth-first from each root, preserving the original file order for
        // roots and for the children of any given parent.
        fn visit<'a>(
            toml: &'a WorktreeToml,
            idx: usize,
            depth: usize,
            visited: &mut Vec<bool>,
            ordered: &mut Vec<(&'a WorktreeEntry, usize)>,
        ) {
            if visited[idx] {
                return;
            }
            visited[idx] = true;
            let entry = &toml.worktrees[idx];
            ordered.push((entry, depth));
            for (child_idx, child) in toml.worktrees.iter().enumerate() {
                if child.repo == entry.repo && child.parent.as_deref() == Some(&entry.branch) {
                    visit(toml, child_idx, depth + 1, visited, ordered);
                }
            }
        }

        for (idx, entry) in self.worktrees.iter().enumerate() {
            if self.resolve_parent_layer(entry).is_none() {
                visit(self, idx, 0, &mut visited, &mut ordered);
            }
        }
        // Anything still unvisited is part of a parent cycle; emit each as a
        // root so nothing is silently dropped.
        for idx in 0..self.worktrees.len() {
            visit(self, idx, 0, &mut visited, &mut ordered);
        }
        ordered
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

    /// Set the stack parent of the worktree identified by `key` (directory
    /// name, slug, or display name — see [`WorktreeToml::find_by_slug`]) and
    /// save. `parent_head` replaces the stored fork point only when `Some`;
    /// passing `None` keeps the existing one (retarget relies on this to
    /// preserve the fork point across a squash-merge).
    /// Returns false if no worktree matches `key`.
    pub fn set_parent(
        &mut self,
        key: &str,
        parent: String,
        parent_head: Option<String>,
    ) -> crate::Result<bool> {
        let Some(path) = self
            .config
            .find_by_slug(key)?
            .map(|w| w.worktree_path.clone())
        else {
            return Ok(false);
        };
        let entry = self
            .config
            .worktrees
            .iter_mut()
            .find(|w| w.worktree_path == path)
            .expect("entry vanished between lookup and update");
        entry.parent = Some(parent);
        if parent_head.is_some() {
            entry.parent_head = parent_head;
        }
        self.save()?;
        Ok(true)
    }

    /// Update only the fork point of the worktree identified by `key` and
    /// save. Used by restack after a layer lands on its parent.
    /// Returns false if no worktree matches `key`.
    pub fn set_parent_head(&mut self, key: &str, parent_head: String) -> crate::Result<bool> {
        let Some(path) = self
            .config
            .find_by_slug(key)?
            .map(|w| w.worktree_path.clone())
        else {
            return Ok(false);
        };
        let entry = self
            .config
            .worktrees
            .iter_mut()
            .find(|w| w.worktree_path == path)
            .expect("entry vanished between lookup and update");
        entry.parent_head = Some(parent_head);
        self.save()?;
        Ok(true)
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

    fn stacked_entry(repo: RepoRef, branch: &str, dir: &str, parent: &str) -> WorktreeEntry {
        entry(repo, branch, dir).with_parent(parent.to_string(), Some("abc123".to_string()))
    }

    /// A stack feat-1 <- feat-2 <- feat-3 plus an unrelated flat worktree.
    fn stacked_toml() -> WorktreeToml {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(entry(hosted("gh", "owner/other"), "main", "other"));
        toml.add_worktree(entry(hosted("gh", "owner/myrepo"), "feat-1", "myrepo"));
        toml.add_worktree(stacked_entry(
            hosted("gh", "owner/myrepo"),
            "feat-3",
            "myrepo@feat-3",
            "feat-2",
        ));
        toml.add_worktree(stacked_entry(
            hosted("gh", "owner/myrepo"),
            "feat-2",
            "myrepo@feat-2",
            "feat-1",
        ));
        toml
    }

    #[test]
    fn with_parent_sets_fields() {
        let e = stacked_entry(hosted("gh", "o/r"), "b", "r@b", "p");
        assert_eq!(e.parent.as_deref(), Some("p"));
        assert_eq!(e.parent_head.as_deref(), Some("abc123"));
    }

    #[test]
    fn parent_fields_roundtrip_and_stay_optional() {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(entry(hosted("gh", "o/flat"), "main", "flat"));
        toml.add_worktree(stacked_entry(hosted("gh", "o/r"), "b", "r@b", "p"));
        let text = toml::to_string_pretty(&toml).unwrap();
        // Flat entries must not gain parent keys in the file.
        assert_eq!(text.matches("parent =").count(), 1, "{}", text);
        let parsed: WorktreeToml = toml::from_str(&text).unwrap();
        assert_eq!(parsed.worktrees[0].parent, None);
        assert_eq!(parsed.worktrees[1].parent.as_deref(), Some("p"));
        assert_eq!(parsed.worktrees[1].parent_head.as_deref(), Some("abc123"));
    }

    #[test]
    fn resolve_parent_layer_matches_same_repo_branch() {
        let toml = stacked_toml();
        let feat2 = toml.find_by_slug("myrepo@feat-2").unwrap().unwrap().clone();
        let parent = toml.resolve_parent_layer(&feat2).unwrap();
        assert_eq!(parent.branch, "feat-1");
    }

    #[test]
    fn resolve_parent_layer_ignores_other_repos_and_plain_refs() {
        let mut toml = stacked_toml();
        // Same branch name exists in another repo — must not match.
        toml.add_worktree(stacked_entry(
            hosted("gh", "owner/unrelated"),
            "x",
            "unrelated@x",
            "feat-1",
        ));
        let x = toml.find_by_slug("unrelated@x").unwrap().unwrap().clone();
        assert!(toml.resolve_parent_layer(&x).is_none());
        // Plain-ref parent (e.g. origin/main) is not a layer either.
        let flat = entry(hosted("gh", "owner/myrepo"), "solo", "myrepo@solo")
            .with_parent("origin/main".to_string(), None);
        assert!(toml.resolve_parent_layer(&flat).is_none());
    }

    #[test]
    fn stacked_order_is_dfs_with_depths() {
        let toml = stacked_toml();
        let order: Vec<(&str, usize)> = toml
            .stacked_order()
            .into_iter()
            .map(|(e, d)| (e.branch.as_str(), d))
            .collect();
        assert_eq!(
            order,
            vec![("main", 0), ("feat-1", 0), ("feat-2", 1), ("feat-3", 2),]
        );
    }

    #[test]
    fn stacked_order_breaks_cycles_without_dropping_entries() {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(stacked_entry(hosted("gh", "o/r"), "a", "r@a", "b"));
        toml.add_worktree(stacked_entry(hosted("gh", "o/r"), "b", "r@b", "a"));
        let order = toml.stacked_order();
        assert_eq!(order.len(), 2);
        let branches: Vec<&str> = order.iter().map(|(e, _)| e.branch.as_str()).collect();
        assert!(branches.contains(&"a") && branches.contains(&"b"));
    }

    #[test]
    fn children_of_finds_direct_children_same_repo_only() {
        let mut toml = stacked_toml();
        // Same parent branch name in another repo — must not count.
        toml.add_worktree(stacked_entry(
            hosted("gh", "owner/unrelated"),
            "x",
            "unrelated@x",
            "feat-1",
        ));
        let feat1 = toml.find_by_slug("myrepo").unwrap().unwrap().clone();
        let children = toml.children_of(&feat1);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].branch, "feat-2");
    }

    #[test]
    fn would_create_cycle_detects_self_and_descendants() {
        let toml = stacked_toml();
        let feat1 = toml.find_by_slug("myrepo").unwrap().unwrap().clone();
        // feat-1 <- feat-2 <- feat-3: pointing feat-1 at any of them cycles.
        assert!(toml.would_create_cycle(&feat1, "feat-1"));
        assert!(toml.would_create_cycle(&feat1, "feat-2"));
        assert!(toml.would_create_cycle(&feat1, "feat-3"));
        // Unrelated refs and other repos' branches do not cycle.
        assert!(!toml.would_create_cycle(&feat1, "main"));
        assert!(!toml.would_create_cycle(&feat1, "origin/main"));
        let feat3 = toml.find_by_slug("myrepo@feat-3").unwrap().unwrap().clone();
        assert!(!toml.would_create_cycle(&feat3, "feat-1"));
    }

    #[test]
    fn would_create_cycle_survives_preexisting_cycle() {
        let mut toml = WorktreeToml::new();
        toml.add_worktree(stacked_entry(hosted("gh", "o/r"), "a", "r@a", "b"));
        toml.add_worktree(stacked_entry(hosted("gh", "o/r"), "b", "r@b", "a"));
        let other = entry(hosted("gh", "o/r"), "c", "r@c");
        // Attaching to a chain that already cycles must be refused, not hang.
        assert!(toml.would_create_cycle(&other, "a"));
    }

    #[test]
    fn chain_of_returns_whole_chain_from_any_member() {
        let toml = stacked_toml();
        for key in ["myrepo", "myrepo@feat-2", "myrepo@feat-3"] {
            let member = toml.find_by_slug(key).unwrap().unwrap().clone();
            let chain: Vec<&str> = toml
                .chain_of(&member)
                .into_iter()
                .map(|e| e.branch.as_str())
                .collect();
            assert_eq!(chain, vec!["feat-1", "feat-2", "feat-3"], "from {}", key);
        }
    }

    #[test]
    fn chain_of_flat_worktree_is_itself() {
        let toml = stacked_toml();
        let flat = toml.find_by_slug("other").unwrap().unwrap().clone();
        let chain: Vec<&str> = toml
            .chain_of(&flat)
            .into_iter()
            .map(|e| e.branch.as_str())
            .collect();
        assert_eq!(chain, vec!["main"]);
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
