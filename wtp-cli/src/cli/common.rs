//! Shared utilities for import and switch commands

use colored::Colorize;
use std::path::{Path, PathBuf};
use wtp_core::fence::Fence;
use wtp_core::{GitClient, RepoRef, WorktreeEntry, WorktreeManager};

/// Check that workspace path is within fence boundary, prompting if not.
pub fn check_workspace_boundary(
    fence: &Fence,
    workspace_name: &str,
    workspace_path: &Path,
) -> anyhow::Result<()> {
    if !fence.is_within_boundary(workspace_path) {
        eprintln!(
            "{} Warning: Workspace '{}' is outside workspace_root: {}",
            "⚠️".yellow(),
            workspace_name.yellow(),
            fence.boundary().display()
        );
        eprintln!(
            "Target path: {}",
            workspace_path.display().to_string().yellow()
        );
        eprint!("Are you sure you want to proceed? [y/N] ");
        std::io::Write::flush(&mut std::io::stderr())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            anyhow::bail!("Operation cancelled");
        }
    }
    Ok(())
}

/// Create a worktree for a repository in a workspace.
/// Returns (absolute_worktree_path, WorktreeEntry).
///
/// With `with_branch_name` the worktree directory is named `<repo>@<branch>`,
/// allowing multiple branches of the same repository to coexist in one
/// workspace. Without it, a repository can only have one worktree per
/// workspace and the directory is named after the repo slug alone.
pub fn create_worktree_in_workspace(
    git: &GitClient,
    repo_root: &Path,
    workspace_path: &Path,
    repo_ref: &RepoRef,
    branch: &str,
    base: &str,
    worktree_manager: &WorktreeManager,
    with_branch_name: bool,
) -> anyhow::Result<(PathBuf, WorktreeEntry)> {
    if let Some(existing) = worktree_manager
        .config()
        .find_by_repo_and_branch(repo_ref, branch)
    {
        anyhow::bail!(
            "Repository '{}' already has a worktree for branch '{}' in this workspace: {}",
            repo_ref.display().yellow(),
            branch.yellow(),
            existing.worktree_path.display()
        );
    }

    if !with_branch_name && let Some(existing) = worktree_manager.config().find_by_repo(repo_ref) {
        anyhow::bail!(
            "Repository '{}' is already in this workspace with branch '{}'.\n\
             Existing worktree: {}\n\
             To add another branch of the same repository, re-run with {} \
             and {} — the new worktree directory will be named '{}'.",
            repo_ref.display().yellow(),
            existing.branch.yellow(),
            existing.worktree_path.display(),
            "-b <branch>".bold(),
            "--with-branch-name".bold(),
            format!("{}@<branch>", repo_ref.slug()).cyan()
        );
    }

    let repo_slug = repo_ref.slug();
    let worktree_path_rel = if with_branch_name {
        worktree_manager.generate_worktree_path_with_branch(&repo_slug, branch)
    } else {
        worktree_manager.generate_worktree_path(&repo_slug)
    };
    let worktree_path_abs = workspace_path.join(&worktree_path_rel);

    println!(
        "Creating worktree at: {}",
        worktree_path_abs.display().to_string().cyan()
    );

    if worktree_path_abs.exists() {
        anyhow::bail!(
            "Worktree directory already exists at {}",
            worktree_path_abs.display()
        );
    }

    let branch_exists = git.branch_exists(repo_root, branch)?;

    if branch_exists {
        println!("Using existing branch: {}", branch.cyan());
        git.add_worktree_for_branch(repo_root, &worktree_path_abs, branch)?;
    } else {
        println!(
            "Creating new branch '{}' from {}",
            branch.cyan(),
            base.dimmed()
        );
        git.create_worktree_with_branch(repo_root, &worktree_path_abs, branch, base)?;
    }

    let head_commit = git.get_head_commit_full(&worktree_path_abs).ok();
    let entry = WorktreeEntry::new(
        repo_ref.clone(),
        branch.to_string(),
        worktree_path_rel,
        Some(base.to_string()),
        head_commit,
    );

    Ok((worktree_path_abs, entry))
}
