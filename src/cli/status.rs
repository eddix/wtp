//! Status command - Show worktree status
//!
//! This is a local command - it works on the current workspace if you're in one,
//! or requires --workspace to specify which workspace to show.

use clap::Args;
use colored::Colorize;
use std::env;
use std::path::PathBuf;

use crate::core::{GitClient, WorktreeManager, WorkspaceManager};

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Workspace to show status for (defaults to current workspace if in one)
    #[arg(short, long, value_name = "NAME")]
    workspace: Option<String>,

    /// Show detailed information
    #[arg(short, long)]
    long: bool,
}

pub async fn execute(args: StatusArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();

    // Determine target workspace
    let (workspace_name, workspace_path) = if let Some(name) = args.workspace {
        // Use explicitly specified workspace
        let path = manager
            .global_config()
            .get_workspace_path(&name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Workspace '{}' not found. Create it with: wtp create {}",
                    name, name
                )
            })?;
        (name, path)
    } else {
        // Try to detect current workspace from current directory
        detect_current_workspace(&manager)?
    };

    if !workspace_path.exists() {
        anyhow::bail!(
            "Workspace '{}' directory does not exist at {}",
            workspace_name,
            workspace_path.display()
        );
    }

    if !workspace_path.join(".wtp").exists() {
        anyhow::bail!(
            "Workspace '{}' exists in config but the directory is missing or corrupted.",
            workspace_name
        );
    }

    println!(
        "Workspace: {} at {}",
        workspace_name.cyan().bold(),
        workspace_path.display().to_string().dimmed()
    );
    println!();

    // Load worktrees
    let worktree_manager = WorktreeManager::load(&workspace_path)?;
    let worktrees = worktree_manager.list_worktrees();

    if worktrees.is_empty() {
        println!("{}", "No worktrees in this workspace.".dimmed());
        println!();
        println!("Import a worktree with:");
        println!(
            "  {}",
            "wtp import <repo_path>".cyan()
        );
        println!();
        println!("Or switch the current repo to this workspace:");
        println!("  {}", format!("wtp switch {}", workspace_name).cyan());
        return Ok(());
    }

    if args.long {
        print_detailed_status(&git, worktrees, &workspace_path).await?;
    } else {
        print_compact_status(&git, worktrees, &workspace_path).await?;
    }

    Ok(())
}

/// Detect current workspace from current directory
/// Returns (workspace_name, workspace_path) if found
fn detect_current_workspace(
    manager: &WorkspaceManager,
) -> anyhow::Result<(String, PathBuf)> {
    let current_dir = env::current_dir()?;
    let mut check_dir = current_dir.as_path();

    loop {
        // Check if this directory has a .wtp subdirectory
        if check_dir.join(".wtp").is_dir() {
            // Find which workspace this is
            for (name, path) in manager.global_config().scan_workspaces().iter() {
                if path == check_dir {
                    return Ok((name.clone(), path.clone()));
                }
            }
            // Directory has .wtp but not registered - might be an orphan
            // Return with the directory name as workspace name
            let name = check_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string();
            return Ok((name, check_dir.to_path_buf()));
        }

        // Move up
        match check_dir.parent() {
            Some(parent) => check_dir = parent,
            None => break,
        }
    }

    // Not in any workspace
    anyhow::bail!(
        "Not in a workspace. Either:\n\
         1. Run this command from within a workspace directory, or\n\
         2. Use --workspace <NAME> to specify the workspace"
    )
}

async fn print_compact_status(
    git: &GitClient,
    worktrees: &[crate::core::WorktreeEntry],
    workspace_path: &std::path::Path,
) -> anyhow::Result<()> {
    println!(
        "{:<30} {:<20} {}",
        "REPOSITORY".bold(),
        "BRANCH".bold(),
        "STATUS".bold()
    );

    for wt in worktrees {
        let wt_full_path = workspace_path.join(&wt.worktree_path);
        let repo_display = wt.repo.display();

        if !wt_full_path.exists() {
            println!(
                "{:<30} {:<20} {}",
                repo_display,
                wt.branch.cyan(),
                "missing".red().bold()
            );
            continue;
        }

        let status_str = match git.get_status(&wt_full_path) {
            Ok(s) => s.format_compact(),
            Err(_) => "?".to_string(),
        };

        println!(
            "{:<30} {:<20} {}",
            if repo_display.len() > 30 {
                format!("{}...", &repo_display[..27])
            } else {
                repo_display
            },
            wt.branch.cyan(),
            status_str
        );
    }

    Ok(())
}

async fn print_detailed_status(
    git: &GitClient,
    worktrees: &[crate::core::WorktreeEntry],
    workspace_path: &std::path::Path,
) -> anyhow::Result<()> {
    let separator = "\u{2500}".repeat(60);

    for wt in worktrees.iter() {
        let wt_full_path = workspace_path.join(&wt.worktree_path);
        let repo_display = wt.repo.display();

        println!("{}", separator.dimmed());
        println!("  {}", repo_display.cyan().bold());
        println!("{}", separator.dimmed());

        if !wt_full_path.exists() {
            println!(
                "  {:<10} {}",
                "Status:".bold(),
                "MISSING".red().bold()
            );
            println!();
            continue;
        }

        // Branch
        println!(
            "  {:<10} {}",
            "Branch:".bold(),
            wt.branch.cyan()
        );

        // HEAD: hash + subject + relative time
        let head_short = git.get_head_commit(&wt_full_path).unwrap_or_default();
        let subject = git
            .get_last_commit_subject(&wt_full_path)
            .unwrap_or_default();
        let rel_time = git
            .get_last_commit_relative_time(&wt_full_path)
            .unwrap_or_default();

        if !head_short.is_empty() {
            println!(
                "  {:<10} {} {} {}",
                "HEAD:".bold(),
                head_short.yellow(),
                subject,
                format!("({})", rel_time).dimmed()
            );
        }

        // Status
        match git.get_status(&wt_full_path) {
            Ok(status) => {
                println!(
                    "  {:<10} {}",
                    "Status:".bold(),
                    status.format_detail_status()
                );

                // Remote
                println!(
                    "  {:<10} {}",
                    "Remote:".bold(),
                    status.format_detail_remote()
                );
            }
            Err(e) => {
                println!(
                    "  {:<10} {}",
                    "Status:".bold(),
                    format!("error: {}", e).red()
                );
            }
        }

        // Stash
        match git.get_stash_count(&wt_full_path) {
            Ok(count) if count > 0 => {
                let entry_word = if count == 1 { "entry" } else { "entries" };
                println!(
                    "  {:<10} {}",
                    "Stash:".bold(),
                    format!("{} {}", count, entry_word).yellow()
                );
            }
            Ok(_) => {
                println!(
                    "  {:<10} {}",
                    "Stash:".bold(),
                    "none".dimmed()
                );
            }
            Err(_) => {}
        }

        println!();
    }
    println!("{}", separator.dimmed());

    Ok(())
}
