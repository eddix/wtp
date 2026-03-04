//! Show current configuration

use crate::core::WorkspaceManager;
use colored::Colorize;

#[derive(clap::Args, Debug)]
pub struct ConfigArgs {}

pub async fn execute(_args: ConfigArgs, manager: WorkspaceManager) -> anyhow::Result<()> {
    let config = manager.global_config();
    let loaded_config = manager.loaded_config();

    println!("{}", "Current Configuration".green().bold());
    println!();

    // Config file source
    if let Some(source) = &loaded_config.source_path {
        println!("{}: {}", "Config file".cyan(), source.display());
    } else {
        println!("{}: {}", "Config file".cyan(), "(default, not saved)".dimmed());
    }
    println!();

    // Workspace root
    println!("{}: {}", "Workspace root".cyan(), config.workspace_root.display());

    // Scan and list workspaces
    let workspaces = config.scan_workspaces();
    println!("{}: {} found", "Workspaces".cyan(), workspaces.len());
    if workspaces.is_empty() {
        println!("  {}", "(none)".dimmed());
    } else {
        for (name, path) in workspaces {
            println!("  {}: {}", name.green(), path.display());
        }
    }
    println!();

    // Hosts
    println!("{}: {}", "Hosts".cyan(), config.hosts.len());
    if config.hosts.is_empty() {
        println!("  {}", "(none)".dimmed());
    } else {
        for (alias, host_config) in &config.hosts {
            println!("  {}: {}", alias.green(), host_config.root.display());
        }
    }

    // Default host
    if let Some(default) = &config.default_host {
        println!("{}: {}", "Default host".cyan(), default.green());
    }
    println!();

    // Hooks
    println!("{}", "Hooks".cyan());
    if let Some(on_create) = &config.hooks.on_create {
        println!("  {}: {}", "on_create".green(), on_create.display());
    } else {
        println!("  {}: {}", "on_create".green(), "(not set)".dimmed());
    }

    Ok(())
}
