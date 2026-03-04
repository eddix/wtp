//! Remove workspace command

use clap::Args;
use colored::Colorize;

use crate::core::WorkspaceManager;

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Name of the workspace to remove
    pub name: String,

    /// Also delete the workspace directory
    #[arg(long)]
    delete_dir: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    force: bool,
}

pub async fn execute(args: RemoveArgs, mut manager: WorkspaceManager) -> anyhow::Result<()> {
    // Check if workspace exists
    if !manager.global_config().has_workspace(&args.name) {
        anyhow::bail!("Workspace '{}' not found", args.name);
    }

    let path = manager
        .global_config()
        .get_workspace_path(&args.name)
        .unwrap();

    if args.delete_dir && !args.force {
        println!(
            "{} This will permanently delete:\n  {}",
            "⚠ WARNING:".red().bold(),
            path.display().to_string().yellow()
        );
        println!();
        print!("Are you sure? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    manager.remove_workspace(&args.name, args.delete_dir)?;

    println!(
        "{} Removed workspace '{}' from configuration",
        "✓".green().bold(),
        args.name.cyan()
    );

    if args.delete_dir && path.exists() {
        println!(
            "{} Deleted directory: {}",
            "✓".green().bold(),
            path.display().to_string().dimmed()
        );
    } else if args.delete_dir {
        println!(
            "{} Directory already removed: {}",
            "ℹ".yellow(),
            path.display().to_string().dimmed()
        );
    }

    Ok(())
}
