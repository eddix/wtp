//! Host management subcommand
//!
//! Manage host aliases for repository path mapping.

use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use crate::core::WorkspaceManager;

#[derive(Args, Debug)]
pub struct HostArgs {
    #[command(subcommand)]
    command: HostCommands,
}

#[derive(Subcommand, Debug)]
pub enum HostCommands {
    /// Add a new host alias
    Add {
        /// Alias name (e.g., "gh", "gl", "bb")
        alias: String,
        /// Root path for repositories under this host
        path: String,
    },
    /// List all configured hosts
    Ls,
    /// Remove a host alias
    Rm {
        /// Alias name to remove
        alias: String,
    },
    /// Set the default host
    SetDefault {
        /// Alias name to set as default
        alias: String,
    },
}

pub async fn execute(args: HostArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    match args.command {
        HostCommands::Add { alias, path } => add_host(alias, path, manager).await,
        HostCommands::Ls => list_hosts(manager).await,
        HostCommands::Rm { alias } => remove_host(alias, manager).await,
        HostCommands::SetDefault { alias } => set_default_host(alias, manager).await,
    }
}

async fn add_host(
    alias: String,
    path: String,
    mut manager: WorkspaceManager,
) -> anyhow::Result<()> {
    // Validate alias name (should be simple, no spaces)
    if alias.contains(' ') || alias.contains('/') || alias.contains('\\') {
        anyhow::bail!(
            "Host alias '{}' contains invalid characters. \
             Use simple names like 'gh', 'gl', 'bb'.",
            alias.red()
        );
    }

    // Expand path and convert to absolute
    let expanded = shellexpand::tilde(&path).to_string();
    let path_buf = PathBuf::from(&expanded);
    let path_buf = if path_buf.is_relative() {
        std::env::current_dir()?.join(&path_buf)
    } else {
        path_buf
    };

    // Check if path exists
    if !path_buf.exists() {
        eprintln!(
            "{} Warning: Path '{}' does not exist yet.",
            "⚠️".yellow(),
            path_buf.display()
        );
        eprintln!("The host will be added, but repositories under it won't be accessible until the directory is created.");
    }

    // Check if host already exists
    if manager.global_config().get_host_root(&alias).is_some() {
        anyhow::bail!(
            "Host alias '{}' already exists. Use 'wtp host rm {}' first if you want to replace it.",
            alias.yellow(),
            alias
        );
    }

    // Add host to config
    manager
        .global_config_mut()
        .hosts
        .insert(alias.clone(), crate::core::config::HostConfig { root: path_buf });
    
    // Save config
    manager.loaded_config().save()?;

    println!(
        "{} Added host alias '{}' -> {}",
        "✓".green().bold(),
        alias.cyan(),
        expanded.dimmed()
    );

    Ok(())
}

async fn list_hosts(manager: WorkspaceManager) -> anyhow::Result<()> {
    let hosts = &manager.global_config().hosts;
    let default = manager.global_config().default_host_alias();

    if hosts.is_empty() {
        println!("{}", "No host aliases configured.".dimmed());
        println!();
        println!("Add a host with:");
        println!("  {}", "wtp host add <alias> <path>".cyan());
        println!();
        println!("Example:");
        println!("  {}", "wtp host add gh ~/codes/github.com".cyan());
        return Ok(());
    }

    println!("{}", "Configured hosts:".bold());
    println!();

    for (alias, config) in hosts.iter() {
        let is_default = default == Some(alias.as_str());
        let marker = if is_default { " (default)".green() } else { "".into() };
        
        println!(
            "  {} -> {}{}",
            alias.cyan().bold(),
            config.root.display().to_string().dimmed(),
            marker
        );
    }

    if default.is_none() {
        println!();
        println!("{}", "No default host set. Use 'wtp host set-default <alias>' to set one.".dimmed());
    }

    Ok(())
}

async fn remove_host(
    alias: String,
    mut manager: WorkspaceManager,
) -> anyhow::Result<()> {
    // Check if host exists
    if manager.global_config().get_host_root(&alias).is_none() {
        anyhow::bail!("Host alias '{}' not found.", alias.red());
    }

    // Check if this is the default host
    let is_default = manager.global_config().default_host_alias() == Some(&alias);
    if is_default {
        eprintln!(
            "{} '{}' is currently the default host. It will be unset.",
            "ℹ".yellow(),
            alias
        );
        manager.global_config_mut().default_host = None;
    }

    // Remove host
    manager.global_config_mut().hosts.remove(&alias);
    
    // Save config
    manager.loaded_config().save()?;

    println!(
        "{} Removed host alias '{}'",
        "✓".green().bold(),
        alias.cyan()
    );

    Ok(())
}

async fn set_default_host(
    alias: String,
    mut manager: WorkspaceManager,
) -> anyhow::Result<()> {
    // Special case: "none" to unset
    if alias == "none" || alias == "null" || alias == "-" {
        manager.global_config_mut().default_host = None;
        manager.loaded_config().save()?;
        println!("{} Unset default host", "✓".green().bold());
        return Ok(());
    }

    // Check if host exists
    if manager.global_config().get_host_root(&alias).is_none() {
        anyhow::bail!(
            "Host alias '{}' not found. Add it first with 'wtp host add {} <path>'",
            alias.red(),
            alias
        );
    }

    // Set as default
    manager.global_config_mut().default_host = Some(alias.clone());
    manager.loaded_config().save()?;

    println!(
        "{} Set '{}' as default host",
        "✓".green().bold(),
        alias.cyan()
    );

    Ok(())
}
