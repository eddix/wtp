//! CLI module containing all subcommand implementations

pub mod cd;
pub mod completions;
pub mod config;
pub mod create;
pub mod eject;
pub mod fuzzy;
pub mod host;
pub mod import;
pub mod ls;
pub mod remove;
pub mod shell_init;
pub mod status;
pub mod switch;
pub mod theme;

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use wtp_derive::GroupedSubcommand;

/// Print custom help message with grouped subcommands
fn print_help() {
    let mut cmd = Cli::command().styles(theme::wtp_styles());
    cmd.build();
    Commands::print_help(&cmd);
}

/// Print styled help for any subcommand with consistent coloring
/// `cmd_path` is the full command path (e.g., "wtp host add")
fn print_styled_help(cmd: &clap::Command, cmd_path: &str) {
    use colored::Colorize;

    // About
    if let Some(about) = cmd.get_about() {
        println!("{}", about);
        println!();
    }

    // Usage line
    let name = cmd_path;
    let mut usage_parts = Vec::new();

    let has_options = cmd
        .get_arguments()
        .any(|a| a.get_short().is_some() || a.get_long().is_some());
    if has_options {
        usage_parts.push("[OPTIONS]".to_string());
    }

    for arg in cmd.get_arguments().filter(|a| a.is_positional()) {
        let display_name = arg
            .get_value_names()
            .and_then(|v| v.first())
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| arg.get_id().as_str().to_uppercase());
        if arg.is_required_set() {
            usage_parts.push(format!("<{}>", display_name));
        } else {
            usage_parts.push(format!("[{}]", display_name));
        }
    }

    if cmd.has_subcommands() {
        usage_parts.push("<COMMAND>".to_string());
    }

    let usage_str = usage_parts.join(" ");
    println!(
        "{}: {} {}",
        "Usage".bold(),
        name.cyan().bold(),
        usage_str.magenta()
    );
    println!();

    // Arguments section (positionals)
    let positionals: Vec<_> = cmd.get_arguments().filter(|a| a.is_positional()).collect();
    if !positionals.is_empty() {
        let mut entries: Vec<(String, String)> = Vec::new();
        for arg in &positionals {
            let display_name = arg
                .get_value_names()
                .and_then(|v| v.first())
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| arg.get_id().as_str().to_uppercase());
            let display = if arg.is_required_set() {
                format!("<{}>", display_name)
            } else {
                format!("[{}]", display_name)
            };
            let help = arg
                .get_help()
                .map(|s| s.to_string())
                .unwrap_or_default();
            entries.push((display, help));
        }
        let max_len = entries.iter().map(|(d, _)| d.len()).max().unwrap_or(0);
        println!("{}:", "Arguments".bold());
        for (display, help) in &entries {
            let padding = " ".repeat(max_len - display.len());
            println!("  {}{}  {}", display.magenta(), padding, help);
        }
        println!();
    }

    // Commands section (subcommands)
    if cmd.has_subcommands() {
        let subcmds: Vec<_> = cmd
            .get_subcommands()
            .filter(|s| s.get_name() != "help")
            .collect();
        if !subcmds.is_empty() {
            let max_len = subcmds
                .iter()
                .map(|s| s.get_name().len())
                .max()
                .unwrap_or(0);
            println!("{}:", "Commands".bold());
            for subcmd in &subcmds {
                let sname = subcmd.get_name();
                let about = subcmd
                    .get_about()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let padding = " ".repeat(max_len - sname.len());
                println!("  {}{}  {}", sname.cyan().bold(), padding, about);
            }
            println!();
        }
    }

    // Options section
    // Store (flag_part, value_part, help) to color flag and value separately
    let mut options: Vec<(String, String, String)> = Vec::new();
    for arg in cmd.get_arguments() {
        let short = arg.get_short();
        let long = arg.get_long();
        if short.is_none() && long.is_none() {
            continue;
        }
        let flag_part = match (short, long) {
            (Some(s), Some(l)) => format!("-{}, --{}", s, l),
            (Some(s), None) => format!("-{}", s),
            (None, Some(l)) => format!("    --{}", l),
            _ => unreachable!(),
        };
        let value_part = match arg.get_action() {
            clap::ArgAction::Set | clap::ArgAction::Append => {
                let val = arg
                    .get_value_names()
                    .and_then(|v| v.first())
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| arg.get_id().as_str().to_uppercase());
                format!(" <{}>", val)
            }
            _ => String::new(),
        };
        let help = arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        options.push((flag_part, value_part, help));
    }
    // Always add -h, --help since we handle it manually
    if !options.iter().any(|(f, _, _)| f.contains("--help")) {
        options.push(("-h, --help".to_string(), String::new(), "Print help".to_string()));
    }

    if !options.is_empty() {
        let max_len = options
            .iter()
            .map(|(f, v, _)| f.len() + v.len())
            .max()
            .unwrap_or(0);
        println!("{}:", "Options".bold());
        for (flag, value, help) in &options {
            let padding = " ".repeat(max_len - flag.len() - value.len());
            if value.is_empty() {
                println!("  {}{}  {}", flag.blue(), padding, help);
            } else {
                println!("  {}{}{}  {}", flag.blue(), value.magenta(), padding, help);
            }
        }
        println!();
    }
}

/// Walk the command tree following subcommand names.
/// Returns (target_command, full_command_path, matched_count).
fn walk_command_tree<'a>(
    root: &'a clap::Command,
    path_args: &[&str],
) -> (&'a clap::Command, Vec<&'a str>, usize) {
    let mut current = root;
    let mut cmd_path: Vec<&str> = vec![root.get_name()];
    let mut matched = 0;
    for &arg in path_args {
        match current.find_subcommand(arg) {
            Some(subcmd) => {
                current = subcmd;
                cmd_path.push(current.get_name());
                matched += 1;
            }
            None => break,
        }
    }
    (current, cmd_path, matched)
}

/// Parse argv to extract help intent and the subcommand path.
/// Returns (is_help_requested, is_help_subcommand_form, subcommand_path).
fn parse_help_intent(user_args: &[String]) -> (bool, bool, Vec<&str>) {
    // "wtp help [cmd...]"
    if user_args[0] == "help" {
        let path: Vec<&str> = user_args[1..]
            .iter()
            .filter(|a| !a.starts_with('-'))
            .map(|s| s.as_str())
            .collect();
        return (true, true, path);
    }

    // Check for --help/-h anywhere; collect non-flag args before it as the path
    let mut path = Vec::new();
    for arg in user_args {
        if arg == "--help" || arg == "-h" {
            return (true, false, path);
        }
        if !arg.starts_with('-') {
            path.push(arg.as_str());
        }
    }

    // No explicit help; collect all non-flag args for implicit help check
    (false, false, path)
}

/// Show help for a resolved command: root → main help, subcommand → styled help.
fn show_help(target: &clap::Command, cmd_path: &[&str]) {
    if cmd_path.len() <= 1 {
        print_help();
    } else {
        print_styled_help(target, &cmd_path.join(" "));
    }
}

/// Try to intercept and show custom help. Returns true if help was shown.
fn try_show_help(root: &clap::Command, args: &[String]) -> bool {
    let user_args = &args[1..];

    // No arguments at all → main help
    if user_args.is_empty() {
        print_help();
        return true;
    }

    let (explicit, is_help_subcmd, path_args) = parse_help_intent(user_args);
    let (target, cmd_path, matched) = walk_command_tree(root, &path_args);

    // "wtp help <unknown>" → report error for the first unmatched arg
    if is_help_subcmd && matched < path_args.len() {
        eprintln!(
            "{}: Unknown command '{}'",
            "Error".red().bold(),
            path_args[matched]
        );
        std::process::exit(1);
    }

    // Explicit help request (--help, -h, or "help" subcommand)
    if explicit {
        show_help(target, &cmd_path);
        return true;
    }

    // Implicit help: subcommand group invoked without a subcommand (e.g., "wtp host")
    if matched > 0 && target.has_subcommands() && matched == path_args.len() {
        show_help(target, &cmd_path);
        return true;
    }

    false
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
    /// Show current configuration
    #[cmd_group("Utilities")]
    Config(config::ConfigArgs),
    /// Eject a repository's worktree from a workspace
    #[cmd_group("Repository Operations")]
    Eject(eject::EjectArgs),
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
    /// Generate shell completion scripts
    #[cmd_group("Utilities")]
    Completions(completions::CompletionsArgs),
}

/// Run the CLI
pub async fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Build command for help display and intercept help requests
    let cmd = Cli::command().styles(theme::wtp_styles());
    if try_show_help(&cmd, &args) {
        return Ok(());
    }

    let cli = Cli::parse();

    // Initialize tracing for verbose mode
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .init();
    }

    // Load global config
    let (loaded_config, warning) = crate::core::LoadedConfig::load()?;

    // Print warning if multiple config files exist
    if let Some(warn) = warning {
        eprintln!("{}", warn.yellow());
    }

    // Initialize security fence
    crate::core::fence::init_global_fence(loaded_config.config.workspace_root.clone());

    // Execute command
    match cli.command {
        Commands::Cd(args) => cd::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Create(args) => create::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Ls(args) => ls::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Remove(args) => remove::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Status(args) => status::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Eject(args) => eject::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Import(args) => import::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Switch(args) => {
            let manager = crate::core::WorkspaceManager::new(loaded_config);
            switch::execute(args, manager).await?
        }
        Commands::Host(args) => host::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::Config(args) => config::execute(args, crate::core::WorkspaceManager::new(loaded_config)).await?,
        Commands::ShellInit(args) => shell_init::execute(args).await?,
        Commands::Completions(args) => completions::execute(args).await?,
    }

    Ok(())
}
