//! Eject worktree command
//!
//! Remove a repository's worktree from the current workspace.
//! This is the reverse of `wtp import`.

use clap::Args;
use colored::Colorize;

use super::fuzzy;
use crate::core::{GitClient, WorktreeManager, WorkspaceManager};

#[derive(Args, Debug)]
pub struct EjectArgs {
    /// Repository slug or display name to eject (e.g., "my-repo" or "gh:owner/repo")
    #[arg(value_name = "REPO")]
    pub repo: Option<String>,

    /// Force eject even if worktree has uncommitted changes
    #[arg(short, long)]
    force: bool,
}

pub async fn execute(args: EjectArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();
    git.check_git()?;

    // Detect current workspace
    let (workspace_name, workspace_path) = manager.require_current_workspace()?;

    // Load worktrees
    let worktree_manager = WorktreeManager::load(&workspace_path)?;
    let worktrees = worktree_manager.list_worktrees();

    if worktrees.is_empty() {
        anyhow::bail!("No worktrees in this workspace.");
    }

    // Determine which repo to eject
    let slug = if let Some(repo) = args.repo {
        repo
    } else {
        select_worktree_interactively(worktrees)?
    };

    // Find the worktree entry
    let entry = worktree_manager
        .config()
        .find_by_slug(&slug)
        .ok_or_else(|| {
            let available: Vec<String> = worktrees.iter().map(|w| w.repo.slug()).collect();
            anyhow::anyhow!(
                "Worktree '{}' not found in workspace.\nAvailable: {}",
                slug,
                available.join(", ")
            )
        })?;

    let repo_display = entry.repo.display();
    let branch = entry.branch.clone();
    let worktree_path_rel = entry.worktree_path.clone();
    let worktree_path_abs = workspace_path.join(&worktree_path_rel);
    // Determine slug used for removal (match by repo slug)
    let removal_slug = entry.repo.slug();

    println!("Ejecting from workspace: {}\n", workspace_name.cyan());
    println!("  {:<14} {}", "Repository:".bold(), repo_display.cyan());
    println!("  {:<14} {}", "Branch:".bold(), branch.cyan());
    println!(
        "  {:<14} {}",
        "Worktree:".bold(),
        worktree_path_abs.display().to_string().dimmed()
    );
    println!();

    if worktree_path_abs.exists() {
        // Safety check: is the worktree dirty?
        let status = git.get_status(&worktree_path_abs)?;
        if status.dirty && !args.force {
            anyhow::bail!(
                "Worktree has uncommitted changes:\n  {}\n\n\
                 Commit or stash your changes first, or use {} to eject anyway.",
                status.format_detail_status(),
                "--force".bold()
            );
        }
        if status.dirty && args.force {
            eprintln!(
                "{} Worktree has uncommitted changes ({}), proceeding with --force.",
                "Warning:".yellow().bold(),
                status.format_detail_status()
            );
        }

        // Resolve the repo root from the worktree path
        let repo_root = git.get_repo_root(Some(&worktree_path_abs))?;
        git.remove_worktree(&repo_root, &worktree_path_abs, args.force)?;
    } else {
        eprintln!(
            "{} Worktree directory not found at {}, cleaning up record only.",
            "Note:".yellow().bold(),
            worktree_path_abs.display()
        );
    }

    // Remove from worktree.toml
    let mut worktree_manager = WorktreeManager::load(&workspace_path)?;
    worktree_manager.remove_worktree(&removal_slug)?;

    println!("{} Worktree ejected successfully.", "✓".green().bold());

    Ok(())
}

/// Select a worktree interactively from the list
fn select_worktree_interactively(
    worktrees: &[crate::core::WorktreeEntry],
) -> anyhow::Result<String> {
    if !fuzzy::is_interactive() {
        anyhow::bail!(
            "No repository specified and not running in an interactive terminal.\n\
             Usage: wtp eject <repo>"
        );
    }

    let items: Vec<(String, String)> = worktrees
        .iter()
        .map(|w| {
            (
                w.repo.slug(),
                format!(
                    "{}    ({}, branch: {})",
                    w.repo.slug(),
                    w.repo.display(),
                    w.branch
                ),
            )
        })
        .collect();

    #[cfg(feature = "fuzzy")]
    {
        use skim::prelude::*;

        struct SelectItem {
            key: String,
            display_text: String,
        }

        impl SkimItem for SelectItem {
            fn text(&self) -> Cow<'_, str> {
                Cow::Borrowed(&self.key)
            }

            fn display<'a>(&'a self, context: DisplayContext) -> ratatui::text::Line<'a> {
                context.to_line(Cow::Borrowed(&self.display_text))
            }

            fn output(&self) -> Cow<'_, str> {
                Cow::Borrowed(&self.key)
            }
        }

        let options = SkimOptionsBuilder::default()
            .prompt("wtp eject (select repo) > ".to_string())
            .height("40%".to_string())
            .multi(false)
            .build()
            .unwrap();

        let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();
        let skim_items: Vec<Arc<dyn SkimItem>> = items
            .iter()
            .map(|(key, display)| {
                Arc::new(SelectItem {
                    key: key.clone(),
                    display_text: display.clone(),
                }) as Arc<dyn SkimItem>
            })
            .collect();
        let _ = tx.send(skim_items);
        drop(tx);

        let output = Skim::run_with(options, Some(rx))
            .ok()
            .ok_or_else(|| anyhow::anyhow!("Selection cancelled"))?;

        if output.is_abort {
            anyhow::bail!("Selection cancelled");
        }

        output
            .selected_items
            .first()
            .map(|item| item.output().to_string())
            .ok_or_else(|| anyhow::anyhow!("No selection made"))
    }

    #[cfg(not(feature = "fuzzy"))]
    {
        eprintln!("{}", "Available worktrees:".bold());
        for (_, display) in &items {
            eprintln!("  {}", display);
        }
        eprintln!();
        anyhow::bail!(
            "No repository specified. Provide a repo name, or rebuild with \
             --features fuzzy to enable interactive selection."
        );
    }
}
