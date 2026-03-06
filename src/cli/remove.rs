//! Remove workspace command
//!
//! Eject all worktrees from a workspace, then remove the workspace directory.

use clap::Args;
use colored::Colorize;
use std::path::Path;

use crate::core::{GitClient, WorkspaceManager, WorktreeManager};

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Name of the workspace to remove
    pub name: String,

    /// Force removal even if worktrees have uncommitted changes
    #[arg(short, long)]
    force: bool,
}

pub async fn execute(args: RemoveArgs, mut manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();
    git.check_git()?;

    // Check if workspace exists
    let workspace_path = manager
        .global_config()
        .get_workspace_path(&args.name)
        .ok_or_else(|| anyhow::anyhow!("Workspace '{}' not found", args.name))?;

    println!("Removing workspace: {}\n", args.name.cyan());

    // Phase 1: Eject all worktrees
    let worktree_manager = WorktreeManager::load(&workspace_path)?;
    let worktrees = worktree_manager.list_worktrees().to_vec();

    if !worktrees.is_empty() {
        println!("{}:", "Ejecting worktrees".bold());

        // Pre-check: if not --force, check for dirty worktrees first
        if !args.force {
            let mut dirty_repos = Vec::new();
            for entry in &worktrees {
                let wt_path = workspace_path.join(&entry.worktree_path);
                if wt_path.exists() {
                    if let Ok(status) = git.get_status(&wt_path) {
                        if status.dirty {
                            dirty_repos.push((
                                entry.repo.display(),
                                status.format_detail_status(),
                            ));
                        }
                    }
                }
            }
            if !dirty_repos.is_empty() {
                eprintln!(
                    "\n{} The following worktrees have uncommitted changes:\n",
                    "Error:".red().bold()
                );
                for (repo, detail) in &dirty_repos {
                    eprintln!("  {}  ({})", repo.cyan(), detail);
                }
                eprintln!(
                    "\nCommit or stash your changes first, or use {} to remove anyway.",
                    "--force".bold()
                );
                std::process::exit(1);
            }
        }

        for entry in &worktrees {
            let wt_path = workspace_path.join(&entry.worktree_path);
            let slug = entry.repo.slug();

            if wt_path.exists() {
                if let Ok(status) = git.get_status(&wt_path) {
                    if status.dirty {
                        eprintln!(
                            "  {} {} ({}), proceeding with --force.",
                            "Warning:".yellow().bold(),
                            slug.cyan(),
                            status.format_detail_status()
                        );
                    }
                }

                match git.get_repo_root(Some(&wt_path)) {
                    Ok(repo_root) => {
                        match git.remove_worktree(&repo_root, &wt_path, args.force) {
                            Ok(()) => {
                                println!("  {} {}", "✓".green().bold(), slug.cyan());
                            }
                            Err(e) => {
                                eprintln!(
                                    "  {} {} — {}",
                                    "✗".red().bold(),
                                    slug.cyan(),
                                    e
                                );
                                if !args.force {
                                    anyhow::bail!(
                                        "Failed to eject '{}'. Use {} to force removal.",
                                        slug,
                                        "--force".bold()
                                    );
                                }
                            }
                        }
                    }
                    Err(_) => {
                        eprintln!(
                            "  {} {} — could not resolve repo root, cleaning up record only.",
                            "!".yellow().bold(),
                            slug.cyan()
                        );
                    }
                }
            } else {
                eprintln!(
                    "  {} {} — directory not found, cleaning up record only.",
                    "!".yellow().bold(),
                    slug.cyan()
                );
            }
        }

        // Clear all entries from worktree.toml
        let mut worktree_manager = WorktreeManager::load(&workspace_path)?;
        for entry in &worktrees {
            worktree_manager.remove_worktree(&entry.repo.slug())?;
        }

        println!();
    }

    // Phase 2: Check remaining contents and remove workspace directory
    let remaining = list_remaining_contents(&workspace_path);

    if remaining.is_empty() {
        // Only .wtp directory left (or nothing) — safe to remove
        manager.remove_workspace(&args.name, true)?;
        println!(
            "{} Workspace '{}' removed.",
            "✓".green().bold(),
            args.name.cyan()
        );
    } else {
        // There are extra files/dirs beyond .wtp
        eprintln!(
            "{} Workspace directory has extra files besides worktrees:\n",
            "Note:".yellow().bold()
        );
        for item in &remaining {
            eprintln!("  {}", item.dimmed());
        }
        eprintln!();

        if args.force {
            manager.remove_workspace(&args.name, true)?;
            println!(
                "{} Workspace '{}' removed (including extra files).",
                "✓".green().bold(),
                args.name.cyan()
            );
        } else {
            eprintln!(
                "Use {} to remove anyway, or clean up these files first.",
                "--force".bold()
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

/// List non-.wtp contents remaining in the workspace directory.
fn list_remaining_contents(workspace_path: &Path) -> Vec<String> {
    let mut remaining = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workspace_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".wtp" {
                continue;
            }
            remaining.push(name);
        }
    }
    remaining
}
