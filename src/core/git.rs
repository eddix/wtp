//! Git command wrapper
//!
//! All git operations are performed through the git CLI to avoid
//! direct manipulation of .git internals.

use crate::core::error::{Result, WtpError};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git client for executing git commands
#[derive(Debug, Clone)]
pub struct GitClient;

impl GitClient {
    pub fn new() -> Self {
        Self
    }

    /// Check if git is available
    pub fn check_git(&self) -> Result<()> {
        match Command::new("git").arg("--version").output() {
            Ok(output) if output.status.success() => Ok(()),
            _ => Err(WtpError::git("Git is not installed or not in PATH")),
        }
    }

    /// Get the root directory of a git repository (supports both normal and bare repos).
    ///
    /// For normal repos: returns the work tree root (same as `--show-toplevel`).
    /// For bare repos: returns the git directory itself (the `.git`-less directory).
    pub fn get_repo_root(&self, cwd: Option<&Path>) -> Result<PathBuf> {
        let resolve_dir = |args: &[&str]| -> std::result::Result<String, WtpError> {
            let mut cmd = Command::new("git");
            for arg in args {
                cmd.arg(arg);
            }
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            let output = cmd.output()?;
            if !output.status.success() {
                return Err(WtpError::NotInGitRepo);
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        };

        // Try --show-toplevel first (works for normal repos)
        if let Ok(toplevel) = resolve_dir(&["rev-parse", "--show-toplevel"]) {
            return Ok(PathBuf::from(toplevel));
        }

        // Fall back to bare repo detection
        let is_bare = resolve_dir(&["rev-parse", "--is-bare-repository"])
            .map(|s| s == "true")
            .unwrap_or(false);

        if is_bare {
            // --git-dir returns "." for bare repos when cwd is the repo itself
            let git_dir = resolve_dir(&["rev-parse", "--git-dir"])?;
            let git_path = PathBuf::from(&git_dir);
            if git_path.is_absolute() {
                return Ok(git_path);
            }
            // Resolve relative path against cwd
            if let Some(dir) = cwd {
                return Ok(dir.join(git_path).canonicalize().map_err(|_| WtpError::NotInGitRepo)?);
            }
        }

        Err(WtpError::NotInGitRepo)
    }

    /// Check if a path is a bare git repository
    pub fn is_bare_repo(&self, path: &Path) -> bool {
        let output = Command::new("git")
            .current_dir(path)
            .arg("rev-parse")
            .arg("--is-bare-repository")
            .output();
        matches!(output, Ok(o) if o.status.success()
            && String::from_utf8_lossy(&o.stdout).trim() == "true")
    }

    /// Check if a directory is a git repository (normal or bare)
    pub fn is_in_git_repo(&self, path: &Path) -> bool {
        self.get_repo_root(Some(path)).is_ok()
    }

    /// Get the current branch name
    pub fn get_current_branch(&self, repo_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get current branch: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get the current HEAD commit hash (short)
    pub fn get_head_commit(&self, repo_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get HEAD commit: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get the full HEAD commit hash
    pub fn get_head_commit_full(&self, repo_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get HEAD commit: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if a branch exists in the repository
    pub fn branch_exists(&self, repo_path: &Path, branch: &str) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("show-ref")
            .arg("--verify")
            .arg(format!("refs/heads/{}", branch))
            .output()?;

        Ok(output.status.success())
    }

    /// Create a new worktree with a new branch
    pub fn create_worktree_with_branch(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        branch: &str,
        base: &str,
    ) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = worktree_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("worktree")
            .arg("add")
            .arg("-b")
            .arg(branch)
            .arg(worktree_path)
            .arg(base)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check for common errors
            if stderr.contains("already checked out") {
                return Err(WtpError::BranchAlreadyCheckedOut {
                    branch: branch.to_string(),
                    worktree_path: worktree_path.to_path_buf(),
                });
            }
            if stderr.contains("already exists") {
                return Err(WtpError::WorktreeAlreadyExists {
                    path: worktree_path.to_path_buf(),
                });
            }
            return Err(WtpError::git(format!(
                "Failed to create worktree: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Add a worktree for an existing branch
    pub fn add_worktree_for_branch(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = worktree_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("worktree")
            .arg("add")
            .arg(worktree_path)
            .arg(branch)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already checked out") {
                return Err(WtpError::BranchAlreadyCheckedOut {
                    branch: branch.to_string(),
                    worktree_path: worktree_path.to_path_buf(),
                });
            }
            if stderr.contains("already exists") {
                return Err(WtpError::WorktreeAlreadyExists {
                    path: worktree_path.to_path_buf(),
                });
            }
            return Err(WtpError::git(format!(
                "Failed to add worktree: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get detailed status of a repository
    pub fn get_status(&self, repo_path: &Path) -> Result<GitStatus> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("status")
            .arg("--porcelain")
            .arg("--branch")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get status: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut ahead = 0;
        let mut behind = 0;
        let mut staged = 0u32;
        let mut unstaged = 0u32;
        let mut untracked = 0u32;

        for line in output_str.lines() {
            if line.starts_with("## ") {
                // Parse branch info
                if let Some(ab) = line.find("[ahead ") {
                    let start = ab + 7;
                    if let Some(end) = line[start..].find(']') {
                        let ahead_str = &line[start..start + end];
                        ahead = ahead_str
                            .split(',')
                            .next()
                            .and_then(|s| s.trim().parse().ok())
                            .unwrap_or(0);
                    }
                }
                if let Some(bb) = line.find("behind ") {
                    let start = bb + 7;
                    if let Some(end) = line[start..].find(']') {
                        behind = line[start..start + end].trim().parse().unwrap_or(0);
                    }
                }
            } else if !line.is_empty() && line.len() >= 2 {
                let bytes = line.as_bytes();
                let x = bytes[0];
                let y = bytes[1];

                if x == b'?' && y == b'?' {
                    untracked += 1;
                } else {
                    if x != b' ' && x != b'?' {
                        staged += 1;
                    }
                    if y != b' ' && y != b'?' {
                        unstaged += 1;
                    }
                }
            }
        }

        let dirty = staged > 0 || unstaged > 0 || untracked > 0;

        Ok(GitStatus {
            dirty,
            ahead,
            behind,
            staged,
            unstaged,
            untracked,
        })
    }

    /// Get the subject line of the last commit
    pub fn get_last_commit_subject(&self, repo_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("log")
            .arg("-1")
            .arg("--format=%s")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get last commit subject: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get the relative time of the last commit (e.g., "2 hours ago")
    pub fn get_last_commit_relative_time(&self, repo_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("log")
            .arg("-1")
            .arg("--format=%cr")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get last commit time: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Remove a worktree from a repository
    pub fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        force: bool,
    ) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_path)
            .arg("worktree")
            .arg("remove");
        if force {
            cmd.arg("--force");
        }
        cmd.arg(worktree_path);

        let output = cmd.output()?;
        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to remove worktree: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get the stash count for a repository
    pub fn get_stash_count(&self, repo_path: &Path) -> Result<u32> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("stash")
            .arg("list")
            .output()?;

        if !output.status.success() {
            return Err(WtpError::git(format!(
                "Failed to get stash list: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let count = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .count() as u32;
        Ok(count)
    }
}

impl Default for GitClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Git status information
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    /// Has uncommitted changes
    pub dirty: bool,
    /// Commits ahead of remote
    pub ahead: u32,
    /// Commits behind remote
    pub behind: u32,
    /// Number of staged files
    pub staged: u32,
    /// Number of modified but unstaged files
    pub unstaged: u32,
    /// Number of untracked files
    pub untracked: u32,
}

impl GitStatus {
    /// Format status as a compact colored string
    pub fn format_compact(&self) -> String {
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

    /// Format detailed status info for the --long view
    pub fn format_detail_status(&self) -> String {
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

    /// Format remote tracking info for the --long view
    pub fn format_detail_remote(&self) -> String {
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
