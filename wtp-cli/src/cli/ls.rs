//! List workspaces command

use clap::Args;
use colored::Colorize;

use crate::cli::git_status_fmt::GitStatusFormat;
use wtp_core::{GitClient, WorkspaceManager, WorktreeManager};

#[derive(Args, Debug)]
pub struct LsArgs {
    /// Show detailed information including repo status
    #[arg(short, long)]
    long: bool,

    /// Output only workspace names (for shell completion)
    #[arg(short, long)]
    short: bool,

    /// Only show workspaces containing a repo matching PATTERN
    /// (case-insensitive substring)
    #[arg(
        short,
        long,
        value_name = "PATTERN",
        help = "Only show workspaces containing a repo matching PATTERN (case-insensitive substring)"
    )]
    grep: Option<String>,
}

pub async fn execute(args: LsArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let mut workspaces = manager.list_workspaces();

    // Optional repo-name filter: keep a workspace only if at least one of its
    // worktrees references a repo matching the pattern. Loading each workspace's
    // worktree.toml here is cheap (no git commands), so the expensive `--long`
    // probing still only happens for the survivors — which then reload the same
    // tiny file. An empty pattern is treated as match-all (grep semantics), so
    // it leaves the listing untouched rather than dropping repo-less workspaces.
    if let Some(pattern) = &args.grep
        && !pattern.is_empty()
    {
        // A workspace whose worktree.toml can't be loaded can't be matched, so
        // it is excluded from filtered results (the unfiltered listing instead
        // surfaces such a workspace as an error).
        workspaces.retain(|ws| {
            WorktreeManager::load(&ws.path)
                .map(|m| m.config().has_repo_matching(pattern))
                .unwrap_or(false)
        });

        if workspaces.is_empty() {
            if !args.short {
                println!(
                    "{}",
                    format!("No workspaces contain a repo matching '{}'.", pattern).dimmed()
                );
            }
            return Ok(());
        }
    }

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
        // Resolve the TTY/NO_COLOR decision once for the whole listing rather
        // than re-checking per repo row.
        let colorize =
            crate::cli::repo_color::should_color(manager.global_config().display.repo_colors);

        for (i, ws) in workspaces.iter().enumerate() {
            if i > 0 {
                println!();
            }

            if !ws.exists || !ws.path.join(".wtp").exists() {
                println!("{}  {}", ws.name.cyan().bold(), "[missing]".red());
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
                            let repo_cell =
                                crate::cli::repo_color::paint_repo(&repo_display, 30, colorize);

                            if !wt_full_path.exists() {
                                println!(
                                    "  {} {:<20} {}",
                                    repo_cell,
                                    wt.branch.cyan(),
                                    "? missing".red()
                                );
                                continue;
                            }

                            let status_str = match git.get_status(&wt_full_path) {
                                Ok(s) => s.format_compact(),
                                Err(e) => format!("! {}", e).red().to_string(),
                            };

                            let base_str = match &wt.base {
                                Some(base) if base != "HEAD" => {
                                    match git.get_ahead_behind(&wt_full_path, base) {
                                        Ok(Some((ahead, behind))) => {
                                            if ahead > 0 || behind > 0 {
                                                let mut parts = Vec::new();
                                                if ahead > 0 {
                                                    parts.push(format!("+{}", ahead));
                                                }
                                                if behind > 0 {
                                                    parts.push(format!("-{}", behind));
                                                }
                                                format!(
                                                    "  {}",
                                                    format!("({}: {})", base, parts.join(" "))
                                                        .dimmed()
                                                )
                                            } else {
                                                String::new()
                                            }
                                        }
                                        Ok(None) => String::new(),
                                        _ => String::new(),
                                    }
                                }
                                _ => String::new(),
                            };

                            println!(
                                "  {} {:<20} {}{}",
                                repo_cell,
                                wt.branch.cyan(),
                                status_str,
                                base_str
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
