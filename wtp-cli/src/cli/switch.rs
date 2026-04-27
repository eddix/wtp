//! Switch command - Add current repo to a workspace
//!
//! This command "switches" the current git repository into a workspace by creating
//! a worktree in that workspace. The workspace must exist unless --create is used.

use clap::Args;
use colored::Colorize;

use super::{common, fuzzy};
use wtp_core::fence::Fence;
use wtp_core::{GitClient, RepoRef, WorkspaceManager, WorktreeManager};

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

pub async fn execute(args: SwitchArgs, mut manager: WorkspaceManager) -> anyhow::Result<()> {
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
    let target_workspace_path = if let Some(path) =
        manager.global_config().get_workspace_path(&workspace_name)
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
                let create_result = manager.create_workspace(&workspace_name, true).await?;
                if let Some(w) = &create_result.hook_warning {
                    eprintln!("{} {}", "Warning:".yellow().bold(), w);
                }
                if let Some(o) = &create_result.hook_output {
                    println!("{}", o);
                }
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
            let create_result = manager.create_workspace(&workspace_name, true).await?;
            if let Some(w) = &create_result.hook_warning {
                eprintln!("{} {}", "Warning:".yellow().bold(), w);
            }
            if let Some(o) = &create_result.hook_output {
                println!("{}", o);
            }
            create_result.path
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

    let fence = Fence::from_config(manager.global_config());
    common::check_workspace_boundary(&fence, &workspace_name, &target_workspace_path)?;

    // Try to match repository to a host alias
    let repo_ref = match manager.match_host_alias(&current_repo_root) {
        Some((host, rel_path)) => {
            println!(
                "Matched to host alias: {} ({})",
                host.cyan(),
                rel_path.dimmed()
            );
            RepoRef::Hosted {
                host,
                path: rel_path,
            }
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
    let base = args
        .base
        .unwrap_or_else(|| match git.get_current_branch(&current_repo_root) {
            Ok(branch) => branch,
            Err(e) => {
                tracing::warn!(
                    "Could not detect current branch for {}: {}",
                    current_repo_root.display(),
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

    // Load existing worktrees in target workspace
    let worktree_manager = WorktreeManager::load(&target_workspace_path)?;
    let (worktree_path_abs, entry) = common::create_worktree_in_workspace(
        &git,
        &current_repo_root,
        &target_workspace_path,
        &repo_ref,
        &branch,
        &base,
        &worktree_manager,
    )?;

    // Record in target workspace's worktree.toml
    let mut worktree_manager = WorktreeManager::load(&target_workspace_path)?;
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
