//! Fuzzy finder integration for interactive selection
//!
//! Provides interactive selection for workspaces, hosts, and repositories
//! using skim (fuzzy feature) or fallback listing.

use colored::Colorize;
use wtp_core::WorkspaceManager;

/// Check if stdin and stderr are connected to a TTY (interactive terminal)
pub fn is_interactive() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Launch skim fuzzy finder to select from a list of items.
///
/// Each item is a `(key, display_text)` pair. The `key` is used for filtering
/// and returned as the selection result. The `display_text` is shown in the UI.
#[cfg(feature = "fuzzy")]
fn select_from_list(items: &[(String, String)], prompt: &str) -> Option<String> {
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
        .prompt(format!("{} > ", prompt))
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

    let output = Skim::run_with(options, Some(rx)).ok()?;

    if output.is_abort {
        return None;
    }

    output
        .selected_items
        .first()
        .map(|item| item.output().to_string())
}

/// Resolve workspace name interactively when no argument is provided.
///
/// Used by `cd` and `switch` commands. Tries fuzzy finder if available,
/// otherwise falls back to listing workspaces with an error message.
pub fn resolve_workspace_interactively(
    manager: &WorkspaceManager,
    command: &str,
) -> anyhow::Result<String> {
    let workspaces = manager.list_workspaces();

    if workspaces.is_empty() {
        anyhow::bail!(
            "No workspaces found. Create one with: {}",
            "wtp create <name>".cyan()
        );
    }

    if !is_interactive() {
        anyhow::bail!(
            "No workspace specified and not running in an interactive terminal.\n\
             Usage: {} <workspace>",
            command
        );
    }

    let items: Vec<(String, String)> = workspaces
        .iter()
        .map(|ws| {
            (
                ws.name.clone(),
                format!("{}    ({})", ws.name, ws.path.display()),
            )
        })
        .collect();

    #[cfg(feature = "fuzzy")]
    {
        match select_from_list(&items, command) {
            Some(name) => Ok(name),
            None => anyhow::bail!("Selection cancelled"),
        }
    }

    #[cfg(not(feature = "fuzzy"))]
    {
        eprintln!("{}", "Available workspaces:".bold());
        for (_name, display) in &items {
            eprintln!("  {}", display);
        }
        eprintln!();
        anyhow::bail!(
            "No workspace specified. Provide a workspace name, or rebuild with \
             --features fuzzy to enable interactive selection."
        );
    }
}

/// Resolve host alias interactively when no host is specified.
///
/// - No hosts configured → error suggesting `wtp host add`
/// - Single host → return it directly
/// - Multiple hosts → fuzzy select (or list + error without fuzzy feature)
/// - Non-TTY → error
pub fn resolve_host_interactively(
    manager: &WorkspaceManager,
    command: &str,
) -> anyhow::Result<String> {
    let hosts = manager.get_hosts();

    if hosts.is_empty() {
        anyhow::bail!(
            "No hosts configured. Add one with: {}",
            "wtp host add <alias> <path>".cyan()
        );
    }

    // Single host → return directly
    if hosts.len() == 1 {
        let alias = hosts.keys().next().unwrap().clone();
        return Ok(alias);
    }

    if !is_interactive() {
        anyhow::bail!(
            "No host specified and not running in an interactive terminal.\n\
             Usage: {} -H <host>",
            command
        );
    }

    let mut items: Vec<(String, String)> = hosts
        .iter()
        .map(|(alias, config)| {
            (
                alias.clone(),
                format!("{}    ({})", alias, config.root.display()),
            )
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));

    #[cfg(feature = "fuzzy")]
    {
        match select_from_list(&items, &format!("{} (select host)", command)) {
            Some(alias) => Ok(alias),
            None => anyhow::bail!("Selection cancelled"),
        }
    }

    #[cfg(not(feature = "fuzzy"))]
    {
        eprintln!("{}", "Available hosts:".bold());
        for (_, display) in &items {
            eprintln!("  {}", display);
        }
        eprintln!();
        anyhow::bail!(
            "No host specified. Use -H <host>, or rebuild with \
             --features fuzzy to enable interactive selection."
        );
    }
}

/// Resolve a repository interactively by scanning the host root.
///
/// Returns `(host_alias, repo_relative_path)` for constructing a `RepoRef::Hosted`.
pub fn resolve_repo_interactively(
    manager: &WorkspaceManager,
    host_alias: &str,
    command: &str,
) -> anyhow::Result<String> {
    let host_root = manager
        .global_config()
        .get_host_root(host_alias)
        .ok_or_else(|| anyhow::anyhow!("Host alias '{}' not found in config", host_alias))?
        .clone();

    let repos = wtp_core::git::scan_git_repos(&host_root);

    if repos.is_empty() {
        anyhow::bail!(
            "No git repositories found under host '{}' ({})",
            host_alias.cyan(),
            host_root.display()
        );
    }

    if !is_interactive() {
        anyhow::bail!(
            "No repository specified and not running in an interactive terminal.\n\
             Usage: {} <path>",
            command
        );
    }

    #[cfg(feature = "fuzzy")]
    {
        let items: Vec<(String, String)> = repos.iter().map(|r| (r.clone(), r.clone())).collect();
        match select_from_list(&items, &format!("{} (select repo)", command)) {
            Some(path) => Ok(path),
            None => anyhow::bail!("Selection cancelled"),
        }
    }

    #[cfg(not(feature = "fuzzy"))]
    {
        eprintln!(
            "{} (host: {}):",
            "Available repositories".bold(),
            host_alias.cyan()
        );
        for repo in &repos {
            eprintln!("  {}", repo);
        }
        eprintln!();
        anyhow::bail!(
            "No repository specified. Provide a path, or rebuild with \
             --features fuzzy to enable interactive selection."
        );
    }
}
