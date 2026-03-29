//! WorkTree for Polyrepo (wtp)
//!
//! A CLI tool for managing git worktrees across multiple repositories.
//!
//! ## Quick Start
//!
//! ```bash
//! # Create a new workspace
//! wtp create my-feature
//!
//! # Switch current repo to the workspace
//! cd ~/projects/my-repo
//! wtp switch my-feature
//!
//! # View workspace status
//! wtp status
//! ```

use anstream::eprintln;
use anstyle::{AnsiColor, Style};

mod cli;

#[tokio::main]
async fn main() {
    // Define error style (red)
    let error_style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)));

    if let Err(e) = cli::run().await {
        eprintln!("{error_style}Error:{error_style:#} {e}");
        std::process::exit(1);
    }
}
