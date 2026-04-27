//! Import worktree command
//!
//! Import an external git repository's worktree into the current workspace.
//! Must be run from within a workspace directory.

use clap::Args;
use colored::Colorize;
use std::path::PathBuf;

use super::{common, fuzzy};
use wtp_core::fence::Fence;
use wtp_core::{GitClient, RepoRef, WorkspaceManager, WorktreeManager};

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// Path to the repository (relative to host root or absolute)
    #[arg(value_name = "PATH")]
    path: Option<String>,

    /// Host alias to use for resolving the repository path
    #[arg(short = 'H', long, value_name = "ALIAS")]
    host: Option<String>,

    /// Full repository path (alternative to PATH)
    #[arg(short, long, value_name = "PATH", conflicts_with = "path")]
    repo: Option<String>,

    /// Branch name to use (defaults to workspace name)
    #[arg(short = 'b', long)]
    branch: Option<String>,

    /// Base reference to create branch from
    #[arg(short = 'B', long)]
    base: Option<String>,
}

pub async fn execute(args: ImportArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();
    git.check_git()?;

    // Determine target workspace — must be in a workspace directory
    let (workspace_name, workspace_path) = manager.require_current_workspace()?;

    println!(
        "Importing into workspace: {} at {}",
        workspace_name.cyan(),
        workspace_path.display().to_string().dimmed()
    );

    let fence = Fence::from_config(manager.global_config());
    common::check_workspace_boundary(&fence, &workspace_name, &workspace_path)?;

    // Resolve repository reference
    let repo_ref = if let Some(repo) = args.repo {
        // --repo flag provided
        let expanded = shellexpand::tilde(&repo).to_string();
        let path = PathBuf::from(expanded);
        if !path.exists() {
            anyhow::bail!("Repository not found: {}", path.display());
        }
        RepoRef::Absolute { path }
    } else if let Some(path) = args.path {
        // Positional path argument provided
        resolve_repo_ref(&manager, &path, args.host.as_deref())?
    } else {
        // No path or repo specified — interactive selection
        resolve_repo_interactively(&manager, args.host.as_deref())?
    };

    // Get absolute path to repository
    let repo_path = repo_ref.to_absolute_path(&manager.global_config().hosts);

    // Verify it's a git repository
    if !git.is_in_git_repo(&repo_path) {
        anyhow::bail!("{} is not a git repository", repo_path.display());
    }

    let repo_root = git.get_repo_root(Some(&repo_path))?;
    let is_bare = git.is_bare_repo(&repo_root);

    println!(
        "Repository: {} at {}{}",
        repo_ref.display().cyan(),
        repo_root.display().to_string().dimmed(),
        if is_bare {
            " (bare)".dimmed().to_string()
        } else {
            String::new()
        }
    );

    // Determine branch name
    let branch = args.branch.unwrap_or_else(|| workspace_name.clone());

    // Determine base reference
    let base = args
        .base
        .unwrap_or_else(|| match git.get_current_branch(&repo_root) {
            Ok(branch) => branch,
            Err(e) => {
                tracing::warn!(
                    "Could not detect current branch for {}: {}",
                    repo_root.display(),
                    e
                );
                eprintln!(
                    "{} Could not detect current branch ({}), using HEAD as base.",
                    "Warning:".yellow().bold(),
                    e
                );
                "HEAD".to_string()
            }
        });

    // Load existing worktrees
    let worktree_manager = WorktreeManager::load(&workspace_path)?;
    let (_worktree_path_abs, entry) = common::create_worktree_in_workspace(
        &git,
        &repo_root,
        &workspace_path,
        &repo_ref,
        &branch,
        &base,
        &worktree_manager,
    )?;

    // Record in worktree.toml
    let mut worktree_manager = WorktreeManager::load(&workspace_path)?;
    worktree_manager.add_worktree(entry)?;

    println!("{} Worktree imported successfully!", "✓".green().bold());

    Ok(())
}

/// Resolve repository interactively when no path is provided.
/// Determines host (--host > default_host > fuzzy select), then scans for repos.
fn resolve_repo_interactively(
    manager: &WorkspaceManager,
    host: Option<&str>,
) -> anyhow::Result<RepoRef> {
    // Determine host alias: --host > default_host > interactive selection
    let host_alias = if let Some(h) = host {
        // Verify the host exists
        manager
            .global_config()
            .get_host_root(h)
            .ok_or_else(|| anyhow::anyhow!("Host alias '{}' not found in config", h))?;
        h.to_string()
    } else if let Some(default) = manager.global_config().default_host_alias() {
        default.to_string()
    } else {
        fuzzy::resolve_host_interactively(manager, "wtp import")?
    };

    // Scan and select repo under this host
    let repo_path = fuzzy::resolve_repo_interactively(manager, &host_alias, "wtp import")?;

    Ok(RepoRef::Hosted {
        host: host_alias,
        path: repo_path,
    })
}

/// Resolve a repository reference from path and optional host
fn resolve_repo_ref(
    manager: &WorkspaceManager,
    path: &str,
    host: Option<&str>,
) -> anyhow::Result<RepoRef> {
    if let Some(host_alias) = host {
        // Explicit host specified
        let host_root = manager
            .global_config()
            .get_host_root(host_alias)
            .ok_or_else(|| anyhow::anyhow!("Host alias '{}' not found in config", host_alias))?;

        let full_path = host_root.join(path);
        wtp_core::fence::validate_within_boundary(host_root, &full_path)
            .map_err(|e| anyhow::anyhow!("Path traversal blocked: {}", e))?;

        Ok(RepoRef::Hosted {
            host: host_alias.to_string(),
            path: path.to_string(),
        })
    } else if let Some(default_host) = manager.global_config().default_host_alias() {
        if let Some(host_root) = manager.global_config().get_host_root(default_host) {
            let full_path = host_root.join(path);
            wtp_core::fence::validate_within_boundary(host_root, &full_path)
                .map_err(|e| anyhow::anyhow!("Path traversal blocked: {}", e))?;
        }

        // Use default host
        Ok(RepoRef::Hosted {
            host: default_host.to_string(),
            path: path.to_string(),
        })
    } else {
        // Treat as absolute/relative path
        let expanded = shellexpand::tilde(path).to_string();
        let path_buf = PathBuf::from(&expanded);

        // Convert to absolute path if it's relative
        let absolute_path = if path_buf.is_absolute() {
            path_buf
        } else {
            std::env::current_dir()?.join(path_buf)
        };

        Ok(RepoRef::Absolute {
            path: absolute_path,
        })
    }
}
