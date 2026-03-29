//! Terminal formatting extension for GitStatus
//!
//! Provides colored terminal output for git status information.
//! This is CLI-specific; GUI will use its own rendering.

use colored::Colorize;
use wtp_core::git::GitStatus;

/// Extension trait for CLI-specific formatting of GitStatus
pub trait GitStatusFormat {
    /// Format status as a compact colored string (for default `wtp status` / `wtp ls`)
    fn format_compact(&self) -> String;

    /// Format detailed status info (for `wtp status --long`)
    fn format_detail_status(&self) -> String;

    /// Format remote tracking info (for `wtp status --long`)
    fn format_detail_remote(&self) -> String;
}

impl GitStatusFormat for GitStatus {
    fn format_compact(&self) -> String {
        if !self.dirty && self.ahead == 0 && self.behind == 0 {
            return format!("{}", "\u{2713} clean".green());
        }

        let mut parts: Vec<String> = Vec::new();

        if self.dirty {
            let mut detail = Vec::new();
            if self.staged > 0 {
                detail.push(format!("{} staged", self.staged));
            }
            if self.unstaged > 0 {
                detail.push(format!("{} unstaged", self.unstaged));
            }
            if self.untracked > 0 {
                detail.push(format!("{} untracked", self.untracked));
            }
            let status_str = format!("* {}", detail.join(", "));
            parts.push(status_str.yellow().to_string());
        }

        if self.ahead > 0 || self.behind > 0 {
            let mut remote_parts = Vec::new();
            if self.ahead > 0 {
                remote_parts.push(format!("+{}", self.ahead).green().to_string());
            }
            if self.behind > 0 {
                remote_parts.push(format!("-{}", self.behind).red().to_string());
            }
            parts.push(format!("({})", remote_parts.join(" ")));
        }

        parts.join("  ")
    }

    fn format_detail_status(&self) -> String {
        if !self.dirty {
            return format!("{}", "\u{2713} clean".green());
        }

        let mut detail = Vec::new();
        if self.staged > 0 {
            detail.push(format!("{} staged", self.staged));
        }
        if self.unstaged > 0 {
            detail.push(format!("{} unstaged", self.unstaged));
        }
        if self.untracked > 0 {
            detail.push(format!("{} untracked", self.untracked));
        }
        format!("{}", detail.join(", ").yellow())
    }

    fn format_detail_remote(&self) -> String {
        if self.ahead == 0 && self.behind == 0 {
            return format!("{}", "up to date".green());
        }

        let mut parts = Vec::new();
        if self.ahead > 0 {
            parts.push(format!("+{} ahead", self.ahead).green().to_string());
        }
        if self.behind > 0 {
            parts.push(format!("-{} behind", self.behind).red().to_string());
        }
        parts.join(", ")
    }
}
