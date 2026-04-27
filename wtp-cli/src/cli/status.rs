//! Status command - Show worktree status
//!
//! This is a local command - shows status of the current workspace.

use clap::Args;
use colored::Colorize;

use crate::cli::git_status_fmt::GitStatusFormat;
use wtp_core::{GitClient, WorkspaceManager, WorktreeManager};

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
        println!("  {}", "wtp import <repo_path>".cyan());
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
    worktrees: &[wtp_core::WorktreeEntry],
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
            Err(e) => format!("! {}", e).red().to_string(),
        };

        println!(
            "{:<30} {:<20} {}",
            truncate_display(&repo_display, 30),
            wt.branch.cyan(),
            status_str
        );
    }

    Ok(())
}

async fn print_detailed_status(
    git: &GitClient,
    worktrees: &[wtp_core::WorktreeEntry],
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
            println!("  {:<10} {}", "Status:".bold(), "MISSING".red().bold());
            println!();
            continue;
        }

        // Branch
        println!("  {:<10} {}", "Branch:".bold(), wt.branch.cyan());

        // Base branch divergence
        if let Some(base) = &wt.base {
            if base != "HEAD" {
                let base_info = match git.get_ahead_behind(&wt_full_path, base) {
                    Ok(Some((ahead, behind))) => {
                        if ahead > 0 || behind > 0 {
                            let mut parts = Vec::new();
                            if ahead > 0 {
                                parts.push(format!("{}", format!("+{} ahead", ahead).green()));
                            }
                            if behind > 0 {
                                parts.push(format!("{}", format!("-{} behind", behind).red()));
                            }
                            format!("{} ({})", base.cyan(), parts.join(", "))
                        } else {
                            format!("{} {}", base.cyan(), "up to date".green())
                        }
                    }
                    Ok(None) => format!("{} {}", base.cyan(), "up to date".green()),
                    Err(_) => format!("{} {}", base.cyan(), "unknown".dimmed()),
                };
                println!("  {:<10} {}", "Base:".bold(), base_info);
            }
        }

        // HEAD: hash + subject + relative time
        let (head_short, subject, rel_time) = git.get_head_info(&wt_full_path).unwrap_or_default();

        if !head_short.is_empty() {
            println!(
                "  {:<10} {} {} {}",
                "HEAD:".bold(),
                head_short.yellow(),
                subject,
                format!("({})", rel_time).dimmed()
            );
        }

        // Status + Stash (combined query)
        match git.get_full_status(&wt_full_path) {
            Ok(full) => {
                println!(
                    "  {:<10} {}",
                    "Status:".bold(),
                    full.status.format_detail_status()
                );
                println!(
                    "  {:<10} {}",
                    "Remote:".bold(),
                    full.status.format_detail_remote()
                );
                if full.stash_count > 0 {
                    let entry_word = if full.stash_count == 1 {
                        "entry"
                    } else {
                        "entries"
                    };
                    println!(
                        "  {:<10} {}",
                        "Stash:".bold(),
                        format!("{} {}", full.stash_count, entry_word).yellow()
                    );
                } else {
                    println!("  {:<10} {}", "Stash:".bold(), "none".dimmed());
                }
            }
            Err(e) => {
                println!(
                    "  {:<10} {}",
                    "Status:".bold(),
                    format!("error: {}", e).red()
                );
            }
        }

        println!();
    }
    println!("{}", separator.dimmed());

    Ok(())
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
/// Uses `char_indices` for a single pass instead of `chars().count()` + `chars().take()`.
fn truncate_display(s: &str, max_len: usize) -> String {
    let suffix_len = 3; // "..."
    if max_len <= suffix_len {
        return s.to_string();
    }
    let cut = max_len - suffix_len;
    for (i, (idx, _)) in s.char_indices().enumerate() {
        if i == cut {
            return format!("{}...", &s[..idx]);
        }
    }
    s.to_string()
}
