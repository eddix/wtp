//! Retarget command - change the stack parent of a worktree
//!
//! Metadata-only: updates the `parent` edge in worktree.toml and leaves
//! git history untouched. Run `wtp restack` afterwards to apply.

use clap::Args;
use colored::Colorize;

use super::common;
use wtp_core::{GitClient, WorkspaceManager, WorktreeManager};

#[derive(Args, Debug)]
pub struct RetargetArgs {
    /// NEW_PARENT (when run inside a worktree directory), or the worktree
    /// to retarget (directory name, slug, or display name) when NEW_PARENT
    /// is also given
    #[arg(value_name = "WORKTREE_OR_PARENT")]
    first: String,

    /// New parent ref, when the first argument names a worktree
    #[arg(value_name = "NEW_PARENT")]
    second: Option<String>,
}

pub async fn execute(args: RetargetArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();
    git.check_git()?;

    let (_, workspace_path) = manager.require_current_workspace()?;
    let mut worktree_manager = WorktreeManager::load(&workspace_path)?;

    // Resolve target worktree and the new parent from the two calling forms.
    let (entry, new_parent) = if let Some(new_parent) = args.second {
        let entry = worktree_manager
            .config()
            .find_by_slug(&args.first)?
            .ok_or_else(|| anyhow::anyhow!("Worktree '{}' not found in workspace", args.first))?
            .clone();
        (entry, new_parent)
    } else {
        let entry = common::detect_current_worktree(&workspace_path, worktree_manager.config())?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Not inside a worktree directory.\n\
                     Run from a worktree, or name it explicitly: wtp retarget <worktree> <new-parent>"
                )
            })?;
        (entry, args.first)
    };

    if new_parent == entry.branch {
        anyhow::bail!("Cannot set '{}' as its own parent", entry.branch.yellow());
    }
    if entry.parent.as_deref() == Some(new_parent.as_str()) {
        println!(
            "Worktree '{}' already has parent '{}'; nothing to do.",
            entry.worktree_path.display(),
            new_parent.cyan()
        );
        return Ok(());
    }
    if worktree_manager
        .config()
        .would_create_cycle(&entry, &new_parent)
    {
        anyhow::bail!(
            "Setting parent of '{}' to '{}' would create a cycle in the stack",
            entry.branch.yellow(),
            new_parent.yellow()
        );
    }

    // The new parent must resolve to a commit.
    let worktree_path_abs = workspace_path.join(&entry.worktree_path);
    if !worktree_path_abs.exists() {
        anyhow::bail!(
            "Worktree directory missing at {}",
            worktree_path_abs.display()
        );
    }
    if git.rev_parse(&worktree_path_abs, &new_parent)?.is_none() {
        anyhow::bail!(
            "New parent ref '{}' not found in repository",
            new_parent.yellow()
        );
    }

    // Preserve an existing fork point (a retarget after the old parent was
    // squash-merged must keep it so restack can transplant precisely).
    // Only a worktree gaining a parent for the first time computes one now.
    let fork_point = if entry.parent.is_none() {
        git.merge_base(&worktree_path_abs, &entry.branch, &new_parent)?
    } else {
        None
    };

    let old_parent = entry.parent.clone();
    let key = entry.worktree_path.display().to_string();
    worktree_manager.set_parent(&key, new_parent.clone(), fork_point)?;

    match old_parent {
        Some(old) => println!(
            "{} Parent of '{}' changed: {} -> {}",
            "✓".green().bold(),
            entry.branch.cyan(),
            old.dimmed(),
            new_parent.cyan()
        ),
        None => println!(
            "{} '{}' is now stacked on '{}'",
            "✓".green().bold(),
            entry.branch.cyan(),
            new_parent.cyan()
        ),
    }
    println!("Run {} to apply.", "wtp restack".bold());

    Ok(())
}
