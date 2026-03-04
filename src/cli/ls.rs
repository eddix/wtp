//! List workspaces command

use clap::Args;
use colored::Colorize;

use crate::core::{GitClient, WorkspaceManager, WorktreeManager};

#[derive(Args, Debug)]
pub struct LsArgs {
    /// Show detailed information including repo status
    #[arg(short, long)]
    long: bool,

    /// Output only workspace names (for shell completion)
    #[arg(short, long)]
    short: bool,
}

pub async fn execute(args: LsArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let workspaces = manager.list_workspaces();

    if workspaces.is_empty() {
        if !args.short {
            println!("{}", "No workspaces found.".dimmed());
            println!();
            println!("Create a workspace with:");
            println!("  {}", "wtp create <workspace_name>".cyan());
            println!();
            println!("All workspaces are stored under workspace_root (default: ~/.wtp/workspaces)");
        }
        return Ok(());
    }

    if args.short {
        // Short format - just names, one per line (for shell completion)
        for ws in workspaces {
            println!("{}", ws.name);
        }
    } else if args.long {
        // Detailed listing with repo details
        let git = GitClient::new();

        for (i, ws) in workspaces.iter().enumerate() {
            if i > 0 {
                println!();
            }

            if !ws.exists || !ws.path.join(".wtp").exists() {
                println!(
                    "{}  {}",
                    ws.name.cyan().bold(),
                    "[missing]".red()
                );
                continue;
            }

            println!("{}", ws.name.cyan().bold());

            match WorktreeManager::load(&ws.path) {
                Ok(wt_manager) => {
                    let worktrees = wt_manager.list_worktrees();
                    if worktrees.is_empty() {
                        println!("  {}", "(no repos)".dimmed());
                    } else {
                        for wt in worktrees {
                            let wt_full_path = ws.path.join(&wt.worktree_path);
                            let repo_display = wt.repo.display();

                            if !wt_full_path.exists() {
                                println!(
                                    "  {:<30} {:<20} {}",
                                    repo_display,
                                    wt.branch.cyan(),
                                    "? missing".red()
                                );
                                continue;
                            }

                            let status_str = match git.get_status(&wt_full_path) {
                                Ok(s) => s.format_compact(),
                                Err(_) => "?".to_string(),
                            };

                            println!(
                                "  {:<30} {:<20} {}",
                                repo_display,
                                wt.branch.cyan(),
                                status_str
                            );
                        }
                    }
                }
                Err(_) => {
                    println!("  {}", "(error loading worktrees)".red());
                }
            }
        }
    } else {
        // Default listing: name + repo count
        for ws in workspaces {
            let name = ws.name.cyan().bold().to_string();

            if !ws.exists || !ws.path.join(".wtp").exists() {
                println!("{}  {}", name, "[missing]".red());
                continue;
            }

            let repo_info = match WorktreeManager::load(&ws.path) {
                Ok(wt_manager) => {
                    let count = wt_manager.list_worktrees().len();
                    match count {
                        0 => "(no repos)".to_string(),
                        1 => "(1 repo)".to_string(),
                        n => format!("({} repos)", n),
                    }
                }
                Err(_) => "(error)".to_string(),
            };

            println!("{}  {}", name, repo_info.dimmed());
        }
    }

    Ok(())
}
