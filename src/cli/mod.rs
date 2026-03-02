//! CLI module containing all subcommand implementations

pub mod cd;
pub mod create;
pub mod host;
pub mod import;
pub mod ls;
pub mod remove;
pub mod shell_init;
pub mod status;
pub mod switch;

use clap::{Command, CommandFactory, Parser, Subcommand};
use colored::Colorize;
use wtp_derive::GroupedSubcommand;

/// Print custom help message with grouped subcommands
fn print_help() {
    let version = env!("CARGO_PKG_VERSION");
    let app_name = "wtp";
    let about = "WorkTree for Polyrepo - Manage git worktrees across multiple repositories";
    
    Commands::print_help(app_name, version, about);
}

/// WorkTree for Polyrepo - Manage multiple git worktrees across repositories
#[derive(Parser, Debug)]
#[command(name = "wtp")]
#[command(disable_help_flag = true, disable_help_subcommand = true)]
#[command(version)]
#[command(color = clap::ColorChoice::Always)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(GroupedSubcommand, Subcommand, Debug)]
pub enum Commands {
    /// Change to a workspace directory (requires shell integration)
    #[cmd_group("Workspace Management")]
    Cd(cd::CdArgs),
    /// Create a new workspace
    #[cmd_group("Workspace Management")]
    Create(create::CreateArgs),
    /// List all workspaces
    #[cmd_group("Workspace Management")]
    Ls(ls::LsArgs),
    /// Remove a workspace
    #[cmd_group("Workspace Management")]
    Remove(remove::RemoveArgs),
    /// Show status of a workspace
    #[cmd_group("Workspace Management")]
    Status(status::StatusArgs),
    /// Import a repository's worktree into a workspace
    #[cmd_group("Repository Operations")]
    Import(import::ImportArgs),
    /// Switch current repo to a workspace
    #[cmd_group("Repository Operations")]
    Switch(switch::SwitchArgs),
    /// Manage host aliases
    #[cmd_group("Utilities")]
    Host(host::HostArgs),
    /// Generate shell integration script
    #[cmd_group("Utilities")]
    ShellInit(shell_init::ShellInitArgs),
}

/// Run the CLI
pub async fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Build command for help display
    let cmd = Cli::command();
    
    // Handle --help and -h for main command
    if args.len() == 1 || (args.len() == 2 && (args[1] == "-h" || args[1] == "--help")) {
        print_help();
        return Ok(());
    }
    
    // Handle help subcommand: wtp help [command]
    if args.len() >= 2 && args[1] == "help" {
        if args.len() >= 3 {
            // Show help for specific subcommand
            let subcmd_name = &args[2];
            if let Some(subcmd) = cmd.find_subcommand(subcmd_name) {
                let mut subcmd_clone = subcmd.clone();
                subcmd_clone.print_help()?;
                println!();
            } else {
                eprintln!("{}: Unknown command '{}'", "Error".red().bold(), subcmd_name);
                std::process::exit(1);
            }
        } else {
            print_help();
        }
        return Ok(());
    }
    
    // Handle <cmd> --help for subcommands
    if args.len() >= 3 && (args[2] == "--help" || args[2] == "-h") {
        let subcmd_name = &args[1];
        if let Some(subcmd) = cmd.find_subcommand(subcmd_name) {
            let mut subcmd_clone = subcmd.clone();
            subcmd_clone.print_help()?;
            println!();
            return Ok(());
        }
    }

    let cli = Cli::parse();

    // Initialize tracing for verbose mode
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .init();
    }

    // Load global config
    let (global_config, warning) = crate::core::GlobalConfig::load()?;

    // Print warning if multiple config files exist
    if let Some(warn) = warning {
        eprintln!("{}", warn.yellow());
    }

    // Initialize security fence
    crate::core::fence::init_global_fence(global_config.workspace_root.clone());

    // Execute command
    match cli.command {
        Commands::Cd(args) => cd::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::Create(args) => create::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::Ls(args) => ls::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::Remove(args) => remove::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::Status(args) => status::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::Import(args) => import::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::Switch(args) => {
            let mut manager = crate::core::WorkspaceManager::new(global_config);
            switch::execute(args, manager).await?
        }
        Commands::Host(args) => host::execute(args, crate::core::WorkspaceManager::new(global_config)).await?,
        Commands::ShellInit(args) => shell_init::execute(args).await?,
    }

    Ok(())
}
