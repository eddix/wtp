//! Switch command - Add current repo to a workspace
//!
//! This command "switches" the current git repository into a workspace by creating
//! a worktree in that workspace. The workspace must exist unless --create is used.

use clap::Args;
use colored::Colorize;

use super::fuzzy;
use crate::core::{
    fence::Fence, GitClient, RepoRef, WorktreeEntry, WorktreeManager, WorkspaceManager,
};

#[derive(Args, Debug)]
pub struct SwitchArgs {
    /// Name of the workspace to switch to
    pub workspace_name: Option<String>,

    /// Create the workspace if it doesn't exist
    #[arg(short, long)]
    create: bool,

    /// Branch name to use (defaults to workspace name)
    #[arg(short, long)]
    branch: Option<String>,

    /// Base reference to create branch from
    #[arg(short = 'B', long)]
    base: Option<String>,
}

pub async fn execute(
    args: SwitchArgs,
    mut manager: WorkspaceManager,
) -> anyhow::Result<()> {
    let git = GitClient::new();
    git.check_git()?;

    // Verify we're in a git repository
    let current_repo_root = match git.get_repo_root(Some(&std::env::current_dir()?)) {
        Ok(path) => path,
        Err(_) => {
            anyhow::bail!(
                "Current directory is not in a git repository. \
                Please run this command from within a git repository."
            );
        }
    };

    println!(
        "Current repository: {}",
        current_repo_root.display().to_string().cyan()
    );

    let workspace_name = match args.workspace_name {
        Some(name) => name,
        None => fuzzy::resolve_workspace_interactively(&manager, "wtp switch")?,
    };

    // Get or create target workspace
    let target_workspace_path = if let Some(path) = manager
        .global_config()
        .get_workspace_path(&workspace_name)
    {
        // Workspace exists in config
        if !path.exists() {
            if args.create {
                // Recreate the workspace directory
                println!(
                    "{} Workspace '{}' exists in config but directory is missing. Recreating...",
                    "ℹ".yellow(),
                    workspace_name
                );
                manager.create_workspace(&workspace_name, true).await?;
            } else {
                anyhow::bail!(
                    "Workspace '{}' directory does not exist at {}. \
                    Use --create to recreate it.",
                    workspace_name,
                    path.display()
                );
            }
        }
        path
    } else {
        // Workspace doesn't exist
        if args.create {
            // Create new workspace
            println!(
                "{} Creating new workspace '{}'...",
                "ℹ".yellow(),
                workspace_name.cyan()
            );
            manager.create_workspace(&workspace_name, true).await?
        } else {
            anyhow::bail!(
                "Workspace '{}' does not exist. \
                Create it with: wtp create {}\n\
                Or use: wtp switch --create {}",
                workspace_name,
                workspace_name,
                workspace_name
            );
        }
    };

    if !target_workspace_path.join(".wtp").exists() {
        anyhow::bail!(
            "Workspace '{}' is missing its .wtp directory. It may be corrupted.",
            workspace_name
        );
    }

    println!(
        "Target workspace: {} at {}",
        workspace_name.cyan(),
        target_workspace_path.display().to_string().dimmed()
    );

    // Security: Check that workspace_path is within workspace_root
    let fence = Fence::from_config(manager.global_config());
    if !fence.is_within_boundary(&target_workspace_path) {
        eprintln!(
            "{} Warning: Workspace '{}' is outside workspace_root: {}",
            "⚠️".yellow(),
            workspace_name.yellow(),
            fence.boundary().display()
        );
        eprintln!(
            "Target path: {}",
            target_workspace_path.display().to_string().yellow()
        );
        eprint!("Are you sure you want to proceed? [y/N] ");
        std::io::Write::flush(&mut std::io::stderr())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            anyhow::bail!("Operation cancelled");
        }
    }

    // Try to match repository to a host alias
    let repo_ref = match manager.match_host_alias(&current_repo_root) {
        Some((host, rel_path)) => {
            println!(
                "Matched to host alias: {} ({})",
                host.cyan(),
                rel_path.dimmed()
            );
            RepoRef::Hosted { host, path: rel_path }
        }
        None => {
            println!(
                "{} Using absolute path (no matching host alias found)",
                "ℹ".yellow()
            );
            RepoRef::Absolute {
                path: current_repo_root.clone(),
            }
        }
    };

    // Determine branch name
    let branch = args.branch.unwrap_or_else(|| workspace_name.clone());

    // Determine base reference
    let base = args.base.unwrap_or_else(|| {
        // Default to current HEAD
        git.get_current_branch(&current_repo_root)
            .unwrap_or_else(|_| "HEAD".to_string())
    });

    // Load existing worktrees in target workspace
    let worktree_manager = WorktreeManager::load(&target_workspace_path)?;

    // Check if this repo already has a worktree in this workspace
    if let Some(existing) = worktree_manager.config().find_by_repo(&repo_ref) {
        anyhow::bail!(
            "Repository '{}' is already in this workspace with branch '{}'.\n\
             Each repository can only have one worktree per workspace.\n\
             Existing worktree: {}",
            repo_ref.display().yellow(),
            existing.branch.yellow(),
            existing.worktree_path.display()
        );
    }

    // Generate worktree path (format: <repo_slug>/)
    let repo_slug = repo_ref.slug();
    let worktree_path_rel = worktree_manager.generate_worktree_path(&repo_slug);
    let worktree_path_abs = target_workspace_path.join(&worktree_path_rel);

    println!(
        "Creating worktree at: {}",
        worktree_path_abs.display().to_string().cyan()
    );

    // Check if worktree directory already exists
    if worktree_path_abs.exists() {
        anyhow::bail!(
            "Worktree directory already exists at {}",
            worktree_path_abs.display()
        );
    }

    // Create the worktree
    let branch_exists = git.branch_exists(&current_repo_root, &branch)?;

    if branch_exists {
        // Use existing branch
        println!("Using existing branch: {}", branch.cyan());
        git.add_worktree_for_branch(&current_repo_root, &worktree_path_abs, &branch)?;
    } else {
        // Create new branch
        println!(
            "Creating new branch '{}' from {}",
            branch.cyan(),
            base.dimmed()
        );
        git.create_worktree_with_branch(
            &current_repo_root,
            &worktree_path_abs,
            &branch,
            &base,
        )?;
    }

    // Get HEAD commit
    let head_commit = git.get_head_commit_full(&worktree_path_abs).ok();

    // Record in target workspace's worktree.toml
    let mut worktree_manager = WorktreeManager::load(&target_workspace_path)?;
    let entry = WorktreeEntry::new(
        repo_ref,
        branch,
        worktree_path_rel,
        Some(base),
        head_commit,
    );
    worktree_manager.add_worktree(entry)?;

    println!(
        "{} Successfully switched '{}' to workspace '{}'",
        "✓".green().bold(),
        current_repo_root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .cyan(),
        workspace_name.cyan()
    );
    println!();
    println!(
        "Worktree created at: {}",
        worktree_path_abs.display().to_string().cyan()
    );
    println!();
    println!("To start working:");
    println!("  {}", format!("cd {}", worktree_path_abs.display()).cyan());

    Ok(())
}
