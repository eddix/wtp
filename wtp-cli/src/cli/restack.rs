//! Restack command - cascade-rebase stack layers onto their parents
//!
//! Stateless and idempotent: layers already based on their parent are
//! skipped, a conflict stops the run with instructions, and re-running
//! after the conflict is resolved continues with the remaining layers.
//! wtp never pushes; a force-push checklist is printed at the end.

use clap::Args;
use colored::Colorize;
use std::path::Path;

use super::common;
use wtp_core::{GitClient, RebaseOutcome, WorkspaceManager, WorktreeEntry, WorktreeManager};

#[derive(Args, Debug)]
pub struct RestackArgs {}

pub async fn execute(_args: RestackArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let git = GitClient::new();
    git.check_git()?;

    let (_, workspace_path) = manager.require_current_workspace()?;
    let mut worktree_manager = WorktreeManager::load(&workspace_path)?;

    // Scope: inside a worktree directory restack that worktree's whole
    // chain; from the workspace root restack every chain.
    let current = common::detect_current_worktree(&workspace_path, worktree_manager.config())?;
    let targets: Vec<WorktreeEntry> = match &current {
        Some(cur) => worktree_manager
            .config()
            .chain_of(cur)
            .into_iter()
            .filter(|e| e.parent.is_some())
            .cloned()
            .collect(),
        None => worktree_manager
            .config()
            .stacked_order()
            .into_iter()
            .map(|(e, _)| e)
            .filter(|e| e.parent.is_some())
            .cloned()
            .collect(),
    };

    if targets.is_empty() {
        println!(
            "{}",
            "No stacked worktrees to restack in this scope.".dimmed()
        );
        return Ok(());
    }

    preflight(&git, &targets, &workspace_path)?;

    let mut rebased: Vec<String> = Vec::new();
    let mut skipped = 0usize;

    for entry in &targets {
        let wt_path = workspace_path.join(&entry.worktree_path);
        let parent = entry.parent.as_deref().expect("targets are filtered");
        let key = entry.worktree_path.display().to_string();

        // Parent commit is resolved at execution time so a layer sees the
        // result of its parent's rebase earlier in this same run.
        let parent_commit = git
            .rev_parse(&wt_path, parent)?
            .expect("preflight verified the parent resolves");

        // Already based on the parent: skip, and heal a stale fork point
        // (e.g. after a conflict was resolved manually with
        // `git rebase --continue`).
        let fork_point = git.merge_base(&wt_path, &entry.branch, parent)?;
        if fork_point.as_deref() == Some(parent_commit.as_str()) {
            if entry.parent_head.as_deref() != Some(parent_commit.as_str()) {
                worktree_manager.set_parent_head(&key, parent_commit.clone())?;
            }
            println!(
                "  {} {} {}",
                "=".dimmed(),
                entry.branch.cyan(),
                "up to date".green()
            );
            skipped += 1;
            continue;
        }

        // The recorded fork point bounds the replayed range so commits the
        // parent already contains (e.g. via squash-merge) are not replayed.
        // Fall back to the live merge-base when it was never recorded.
        let upstream = entry.parent_head.clone().or(fork_point).ok_or_else(|| {
            anyhow::anyhow!(
                "'{}' shares no history with parent '{}' and has no recorded \
                     fork point; cannot restack",
                entry.branch,
                parent
            )
        })?;

        println!(
            "  {} {} onto {}",
            "~".yellow(),
            entry.branch.cyan(),
            parent.cyan()
        );
        match git.rebase_onto(&wt_path, parent, &upstream)? {
            RebaseOutcome::Completed => {
                worktree_manager.set_parent_head(&key, parent_commit)?;
                rebased.push(entry.branch.clone());
            }
            RebaseOutcome::Conflict { conflicts } => {
                report_conflict(entry, &wt_path, parent, &conflicts);
                anyhow::bail!("restack stopped on conflicts in '{}'", entry.branch);
            }
        }
    }

    println!();
    println!(
        "{} Restack complete: {} rebased, {} already up to date.",
        "✓".green().bold(),
        rebased.len(),
        skipped
    );

    if !rebased.is_empty() {
        println!();
        println!(
            "{} Rewritten branches (wtp never pushes — when ready, run):",
            "Note:".yellow().bold()
        );
        for branch in &rebased {
            println!("  git push --force-with-lease origin {}", branch.cyan());
        }
    }

    Ok(())
}

/// Fail fast before touching anything: every target layer must have an
/// existing worktree directory, a clean working tree, no rebase already in
/// progress, and a resolvable parent ref.
fn preflight(
    git: &GitClient,
    targets: &[WorktreeEntry],
    workspace_path: &Path,
) -> anyhow::Result<()> {
    let mut problems: Vec<String> = Vec::new();

    for entry in targets {
        let wt_path = workspace_path.join(&entry.worktree_path);
        let name = entry.worktree_path.display();

        if !wt_path.exists() {
            problems.push(format!("{}: worktree directory missing", name));
            continue;
        }
        if git.has_rebase_in_progress(&wt_path) {
            problems.push(format!(
                "{}: a rebase is already in progress — resolve it (git rebase \
                 --continue or --abort) first",
                name
            ));
            continue;
        }
        match git.get_status(&wt_path) {
            Ok(status) if status.dirty => {
                problems.push(format!(
                    "{}: uncommitted changes — commit or stash first",
                    name
                ));
            }
            Ok(_) => {}
            Err(e) => problems.push(format!("{}: cannot read status: {}", name, e)),
        }
        if let Some(parent) = entry.parent.as_deref() {
            match git.rev_parse(&wt_path, parent) {
                Ok(Some(_)) => {}
                _ => problems.push(format!(
                    "{}: parent '{}' does not resolve — branch deleted? \
                     reparent with: wtp retarget {} <new-parent>",
                    name, parent, name
                )),
            }
        }
    }

    if !problems.is_empty() {
        anyhow::bail!(
            "Cannot restack — fix these first:\n{}",
            problems
                .iter()
                .map(|p| format!("  ✗ {}", p))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    Ok(())
}

/// Structured conflict report: everything a human or agent needs to resolve
/// the conflict in place and resume.
fn report_conflict(entry: &WorktreeEntry, wt_path: &Path, parent: &str, conflicts: &[String]) {
    eprintln!();
    eprintln!(
        "{} Conflict while restacking '{}' onto '{}'",
        "✗".red().bold(),
        entry.branch.yellow(),
        parent.yellow()
    );
    eprintln!();
    eprintln!("  {:<10} {}", "Worktree:".bold(), wt_path.display());
    if conflicts.is_empty() {
        eprintln!(
            "  {:<10} {}",
            "Conflicts:".bold(),
            "(see git status)".dimmed()
        );
    } else {
        eprintln!("  {}", "Conflicts:".bold());
        for file in conflicts {
            eprintln!("    {}", file.red());
        }
    }
    eprintln!();
    eprintln!(
        "  Resolve the conflicts in that directory, run {}, then re-run {} —\n\
         \x20 already-finished layers are skipped automatically.",
        "git rebase --continue".bold(),
        "wtp restack".bold()
    );
}
