//! Fuzzy finder integration for interactive selection
//!
//! Provides interactive selection for workspaces, hosts, and repositories
//! using skim (fuzzy feature) or fallback listing.

use crate::core::WorkspaceManager;
use colored::Colorize;
use std::path::Path;

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

/// Check if a directory looks like a bare git repository.
///
/// A bare repo has `HEAD`, `objects/`, and `refs/` directly in the directory,
/// but no `.git` subdirectory.
fn is_bare_git_repo(path: &Path) -> bool {
    !path.join(".git").exists()
        && path.join("HEAD").is_file()
        && path.join("objects").is_dir()
        && path.join("refs").is_dir()
}

/// Scan a directory for git repositories (normal and bare).
///
/// Walks the directory tree looking for:
/// - Normal repos: directories that contain `.git` (file or directory)
/// - Bare repos: directories that contain `HEAD` + `objects/` + `refs/`
///
/// Returns paths relative to `root`.
///
/// - Skips hidden directories, except those ending with `.git` (bare repo convention)
/// - Does not follow symlinks
/// - Limits depth to 4 levels (covers `owner/repo` structure)
/// - Stops recursing into directories that are themselves git repos
pub fn scan_git_repos(root: &Path) -> Vec<String> {
    use walkdir::WalkDir;

    let mut repos = Vec::new();

    let walker = WalkDir::new(root)
        .min_depth(1)
        .max_depth(4)
        .follow_links(false)
        .sort_by_file_name();

    let mut skip_prefixes: Vec<std::path::PathBuf> = Vec::new();

    for entry in walker.into_iter().filter_entry(|e| {
        // Skip hidden directories, but allow `.git`-suffixed dirs (bare repos)
        if e.depth() > 0 {
            if let Some(name) = e.file_name().to_str() {
                if name.starts_with('.') && !name.ends_with(".git") {
                    return false;
                }
            }
        }
        true
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Only look at directories
        if !entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();

        // Skip if we're inside a previously found git repo
        if skip_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
            continue;
        }

        // Check for normal repo (.git exists) or bare repo
        let is_repo = path.join(".git").exists() || is_bare_git_repo(path);
        if is_repo {
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().to_string();
                if !rel_str.is_empty() {
                    repos.push(rel_str);
                    // Don't recurse into this repo's subdirectories
                    skip_prefixes.push(path.to_path_buf());
                }
            }
        }
    }

    repos
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

    let repos = scan_git_repos(&host_root);

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
        let items: Vec<(String, String)> = repos
            .iter()
            .map(|r| (r.clone(), r.clone()))
            .collect();
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
