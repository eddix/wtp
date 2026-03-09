//! Status command - Show worktree status
//!
//! This is a local command - shows status of the current workspace.

use clap::Args;
use colored::Colorize;

use crate::core::{GitClient, WorktreeManager, WorkspaceManager};

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Show detailed information
    #[arg(short, long)]
    long: bool,
}

pub async fn execute(args: StatusArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();

    // Detect current workspace from current directory
    let (workspace_name, workspace_path) = manager.require_current_workspace()?;

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
            if repo_display.chars().count() > 30 {
                let truncated: String = repo_display.chars().take(27).collect();
                format!("{}...", truncated)
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
