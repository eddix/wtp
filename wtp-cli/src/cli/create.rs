//! Create workspace command

use clap::Args;
use colored::Colorize;

use wtp_core::WorkspaceManager;

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Name of the workspace to create
    pub name: String,

    /// Skip running the on_create hook script
    #[arg(long, help = "Skip running the on_create hook script")]
    pub no_hook: bool,
}

pub async fn execute(args: CreateArgs, mut manager: WorkspaceManager) -> anyhow::Result<()> {
    let result = manager.create_workspace(&args.name, !args.no_hook).await?;

    if let Some(warning) = &result.hook_warning {
        eprintln!("{} {}", "Warning:".yellow().bold(), warning);
    }
    if let Some(output) = &result.hook_output {
        println!("{}", output);
    }

    println!(
        "{} Created workspace '{}' at {}",
        "✓".green().bold(),
        args.name.cyan(),
        result.path.display().to_string().dimmed()
    );
    println!();
    println!("To use this workspace, run:");
    println!("  {}", format!("cd {}", result.path.display()).cyan());

    Ok(())
}
