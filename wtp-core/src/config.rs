//! Configuration management for wtp
//!
//! Handles global configuration loading with priority order:
//! 1. ~/.wtp.toml
//! 2. ~/.wtp/config.toml
//! 3. ~/.config/wtp/config.toml

use crate::error::{Result, WtpError};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// Runtime handle for a loaded configuration
///
/// This separates the configuration data (`GlobalConfig`) from runtime metadata
/// like the file path it was loaded from.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    /// The configuration data
    pub config: GlobalConfig,
    /// Path to the config file this was loaded from (runtime metadata, not serialized)
    pub source_path: Option<PathBuf>,
}

impl LoadedConfig {
    /// Load configuration from the first existing config file
    /// Returns the loaded config with source path, and an optional warning about multiple files
    pub fn load() -> Result<(Self, Option<String>)> {
        let paths = GlobalConfig::config_paths();
        let mut found_paths: Vec<PathBuf> = Vec::new();
        let mut loaded_path: Option<PathBuf> = None;
        let mut config: Option<GlobalConfig> = None;

        for path in &paths {
            if path.exists() {
                found_paths.push(path.clone());
                if config.is_none() {
                    let content = std::fs::read_to_string(path)?;
                    let mut cfg: GlobalConfig = toml::from_str(&content)?;
                    // Expand ~ in workspace_root
                    cfg.workspace_root = shellexpand::tilde(&cfg.workspace_root.to_string_lossy())
                        .to_string()
                        .into();
                    // Expand ~ in host roots
                    for host in cfg.hosts.values_mut() {
                        host.root = shellexpand::tilde(&host.root.to_string_lossy())
                            .to_string()
                            .into();
                    }
                    // Expand ~ in hook paths
                    if let Some(ref mut hook_path) = cfg.hooks.on_create {
                        *hook_path = shellexpand::tilde(&hook_path.to_string_lossy())
                            .to_string()
                            .into();
                        // Resolve relative hook paths against the config file location.
                        if hook_path.is_relative() {
                            if let Some(config_dir) = path.parent() {
                                *hook_path = config_dir.join(&hook_path);
                            }
                        }
                    }
                    config = Some(cfg);
                    loaded_path = Some(path.clone());
                }
            }
        }

        let warning = if found_paths.len() > 1 {
            let files: Vec<_> = found_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            Some(format!(
                "Warning: Multiple config files found: {}. Using {}",
                files.join(", "),
                loaded_path
                    .as_ref()
                    .expect("loaded_path must be Some when warning is generated")
                    .display()
            ))
        } else {
            None
        };

        let loaded = Self {
            config: config.unwrap_or_default(),
            source_path: loaded_path,
        };

        Ok((loaded, warning))
    }

    /// Save configuration to the file it was loaded from,
    /// or to the default location (~/.wtp/config.toml) if not loaded from file
    pub fn save(&self) -> Result<()> {
        let config_path = match &self.source_path {
            Some(path) => path.clone(),
            None => {
                // Default location: ~/.wtp/config.toml
                dirs::home_dir()
                    .ok_or_else(|| WtpError::config("Could not find home directory"))?
                    .join(".wtp")
                    .join("config.toml")
            }
        };

        // Refuse to write through symlinks.
        if config_path.exists() {
            let meta = std::fs::symlink_metadata(&config_path)?;
            if meta.is_symlink() {
                return Err(WtpError::config(format!(
                    "Refusing to write config through symlink: {}",
                    config_path.display()
                )));
            }
        }

        // Create parent directories if needed
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&self.config)?;
        let parent = config_path
            .parent()
            .ok_or_else(|| WtpError::config("Config path has no parent directory"))?;
        let mut tmp = NamedTempFile::new_in(parent)
            .map_err(|e| WtpError::config(format!("Failed to create temp file: {}", e)))?;
        tmp.write_all(content.as_bytes())
            .map_err(|e| WtpError::config(format!("Failed to write temp file: {}", e)))?;
        tmp.persist(&config_path)
            .map_err(|e| WtpError::config(format!("Failed to persist config: {}", e)))?;

        Ok(())
    }
}

/// Default workspace root directory name
pub const DEFAULT_WORKSPACE_ROOT: &str = ".wtp/workspaces";

/// Directory name for wtp metadata inside a workspace
pub const WTP_DIR: &str = ".wtp";

/// The global configuration structure
///
/// This contains only the serializable configuration data.
/// Use `LoadedConfig` for runtime access with metadata like source path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Root directory for all workspaces (default: ~/.wtp/workspaces)
    #[serde(default = "default_workspace_root")]
    pub workspace_root: PathBuf,

    /// Host aliases mapping host name to root directory
    #[serde(default)]
    pub hosts: IndexMap<String, HostConfig>,

    /// Default host alias to use when not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_host: Option<String>,

    /// Hooks configuration for workspace lifecycle events
    #[serde(default)]
    pub hooks: HooksConfig,

    /// Display / output preferences
    #[serde(default, skip_serializing_if = "DisplayConfig::is_default")]
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// Root directory for this host
    pub root: PathBuf,
}

/// Display / output preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// When to colorize repo names in `wtp ls --long` by hashing each repo to a
    /// stable, distinct color. See [`RepoColorMode`].
    #[serde(default)]
    pub repo_colors: RepoColorMode,
}

impl DisplayConfig {
    /// Whether this is the default config (used to skip serializing it so we
    /// don't sprinkle a `[display]` section into configs that never set one).
    fn is_default(&self) -> bool {
        *self == DisplayConfig::default()
    }
}

/// Controls when `wtp ls --long` paints repo names with their hashed color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoColorMode {
    /// Color when stdout is a terminal and `NO_COLOR` is unset (default).
    #[default]
    Auto,
    /// Always color, even when the output is piped or redirected.
    Always,
    /// Never color repo names.
    Never,
}

/// Hooks configuration for workspace lifecycle events
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Hook script to run after creating a workspace
    /// Receives environment variables:
    /// - WTP_WORKSPACE_NAME: Name of the created workspace
    /// - WTP_WORKSPACE_PATH: Full path to the workspace directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_create: Option<PathBuf>,
}

fn default_workspace_root() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(DEFAULT_WORKSPACE_ROOT))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_WORKSPACE_ROOT))
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            workspace_root: default_workspace_root(),
            hosts: IndexMap::new(),
            default_host: None,
            hooks: HooksConfig::default(),
            display: DisplayConfig::default(),
        }
    }
}

impl GlobalConfig {
    /// Get the list of possible config file paths in priority order
    pub fn config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. ~/.wtp.toml
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".wtp.toml"));
        }

        // 2. ~/.wtp/config.toml
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".wtp").join("config.toml"));
        }

        // 3. ~/.config/wtp/config.toml
        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("wtp").join("config.toml"));
        }

        paths
    }

    /// Get the path for a workspace by name
    ///
    /// The name is sanitized first so callers can pass either the original
    /// (`hotfix/foo`) or the on-disk (`hotfix_foo`) form interchangeably.
    pub fn get_workspace_path(&self, name: &str) -> Option<PathBuf> {
        let path = self.workspace_root.join(sanitize_workspace_name(name));
        if path.is_dir() && path.join(WTP_DIR).is_dir() {
            Some(path)
        } else {
            None
        }
    }

    /// Scan all workspaces in workspace_root
    /// Returns a map of workspace name to path for all valid workspaces
    pub fn scan_workspaces(&self) -> HashMap<String, PathBuf> {
        let mut workspaces = HashMap::new();

        if let Ok(entries) = std::fs::read_dir(&self.workspace_root) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let path = entry.path();
                        // Check if this directory has a .wtp subdirectory
                        if path.join(WTP_DIR).is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                workspaces.insert(name.to_string(), path);
                            }
                        }
                    }
                }
            }
        }

        workspaces
    }

    /// Get host root by alias
    pub fn get_host_root(&self, alias: &str) -> Option<&PathBuf> {
        self.hosts.get(alias).map(|h| &h.root)
    }

    /// Get default host alias
    pub fn default_host_alias(&self) -> Option<&str> {
        self.default_host.as_deref()
    }

    /// Get the absolute workspace path for a new workspace
    ///
    /// Sanitizes the name so a single workspace always lives directly under
    /// `workspace_root` (one path component), regardless of what the user typed.
    /// See [`sanitize_workspace_name`].
    pub fn resolve_workspace_path(&self, name: &str) -> PathBuf {
        self.workspace_root.join(sanitize_workspace_name(name))
    }
}

impl LoadedConfig {
    /// Scan all workspaces (delegates to config)
    pub fn scan_workspaces(&self) -> HashMap<String, PathBuf> {
        self.config.scan_workspaces()
    }
}

/// Rewrite a user-supplied workspace name to a safe single-segment directory name.
///
/// `wtp ls` only scans direct children of `workspace_root`, so a name like
/// `hotfix/update_task_issue_1234` would otherwise create a nested directory
/// that the listing never sees. We replace path separators and other
/// cross-platform-unsafe characters with `_`, collapse runs of underscores
/// produced by replacement, and trim leading/trailing dots and spaces (also a
/// Windows requirement).
///
/// The function is idempotent: calling it on an already-sanitized name returns
/// the same string.
pub fn sanitize_workspace_name(name: &str) -> String {
    // Pass 1: replace unsafe chars with `_`.
    let mut replaced = String::with_capacity(name.len());
    for ch in name.chars() {
        let unsafe_char = matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            || (ch as u32) < 0x20;
        replaced.push(if unsafe_char { '_' } else { ch });
    }

    // Pass 2: collapse consecutive underscores so `feat//foo` -> `feat_foo`,
    // not `feat__foo`.
    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_underscore = false;
    for ch in replaced.chars() {
        if ch == '_' {
            if !prev_underscore {
                collapsed.push(ch);
            }
            prev_underscore = true;
        } else {
            collapsed.push(ch);
            prev_underscore = false;
        }
    }

    // Pass 3: trim leading/trailing `.` and spaces (Windows reserves these and
    // they're ergonomically bad anyway).
    let trimmed = collapsed
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();

    // Pathological inputs that consisted entirely of unsafe chars (`/`, `///`,
    // mixed control chars, ...) collapse to a lone `_` after replacement.
    // Treat that as "no usable name" so the caller can reject creation.
    if trimmed.chars().all(|c| c == '_') {
        return String::new();
    }
    trimmed
}

#[cfg(test)]
mod sanitize_workspace_name_tests {
    use super::sanitize_workspace_name;

    #[test]
    fn passes_through_clean_names() {
        assert_eq!(sanitize_workspace_name("simple"), "simple");
        assert_eq!(sanitize_workspace_name("hotfix-1234"), "hotfix-1234");
        assert_eq!(sanitize_workspace_name("a_b_c"), "a_b_c");
        assert_eq!(sanitize_workspace_name("v1.2"), "v1.2");
    }

    #[test]
    fn replaces_path_separators() {
        assert_eq!(
            sanitize_workspace_name("hotfix/update_task_issue_1234"),
            "hotfix_update_task_issue_1234"
        );
        assert_eq!(sanitize_workspace_name("a\\b"), "a_b");
        assert_eq!(sanitize_workspace_name("feat/foo/bar"), "feat_foo_bar");
    }

    #[test]
    fn collapses_consecutive_separators() {
        assert_eq!(sanitize_workspace_name("a//b"), "a_b");
        assert_eq!(sanitize_workspace_name("a/\\b"), "a_b");
        assert_eq!(sanitize_workspace_name("a___b"), "a_b");
    }

    #[test]
    fn replaces_windows_reserved_chars() {
        assert_eq!(sanitize_workspace_name("a:b"), "a_b");
        assert_eq!(sanitize_workspace_name("a*b"), "a_b");
        assert_eq!(sanitize_workspace_name("a?b"), "a_b");
        assert_eq!(sanitize_workspace_name("a\"b"), "a_b");
        assert_eq!(sanitize_workspace_name("a<b"), "a_b");
        assert_eq!(sanitize_workspace_name("a>b"), "a_b");
        assert_eq!(sanitize_workspace_name("a|b"), "a_b");
    }

    #[test]
    fn replaces_control_characters() {
        assert_eq!(sanitize_workspace_name("a\nb"), "a_b");
        assert_eq!(sanitize_workspace_name("a\tb"), "a_b");
        assert_eq!(sanitize_workspace_name("a\x01b"), "a_b");
    }

    #[test]
    fn trims_leading_and_trailing_dots_and_spaces() {
        assert_eq!(sanitize_workspace_name(" foo "), "foo");
        assert_eq!(sanitize_workspace_name(".foo."), "foo");
        assert_eq!(sanitize_workspace_name(" .foo. "), "foo");
    }

    #[test]
    fn is_idempotent() {
        let once = sanitize_workspace_name("hotfix/update_task");
        let twice = sanitize_workspace_name(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn returns_empty_for_pathological_inputs() {
        assert_eq!(sanitize_workspace_name(""), "");
        assert_eq!(sanitize_workspace_name("/"), "");
        assert_eq!(sanitize_workspace_name("///"), "");
        assert_eq!(sanitize_workspace_name("..."), "");
        assert_eq!(sanitize_workspace_name("   "), "");
    }
}

#[cfg(test)]
mod display_config_tests {
    use super::{DisplayConfig, GlobalConfig, RepoColorMode};

    #[test]
    fn defaults_to_auto() {
        assert_eq!(DisplayConfig::default().repo_colors, RepoColorMode::Auto);
    }

    #[test]
    fn missing_display_section_uses_default() {
        let cfg: GlobalConfig = toml::from_str("workspace_root = \"/tmp/ws\"").unwrap();
        assert_eq!(cfg.display.repo_colors, RepoColorMode::Auto);
    }

    #[test]
    fn parses_repo_colors_modes() {
        for (text, expected) in [
            ("auto", RepoColorMode::Auto),
            ("always", RepoColorMode::Always),
            ("never", RepoColorMode::Never),
        ] {
            let toml = format!("workspace_root = \"/tmp/ws\"\n[display]\nrepo_colors = \"{text}\"");
            let cfg: GlobalConfig = toml::from_str(&toml).unwrap();
            assert_eq!(cfg.display.repo_colors, expected, "mode {text}");
        }
    }

    #[test]
    fn default_display_is_not_serialized() {
        // A config left at defaults should not emit a [display] section.
        let cfg = GlobalConfig::default();
        let out = toml::to_string_pretty(&cfg).unwrap();
        assert!(
            !out.contains("[display]"),
            "unexpected display section:\n{out}"
        );
    }

    #[test]
    fn customized_display_is_serialized() {
        let mut cfg = GlobalConfig::default();
        cfg.display.repo_colors = RepoColorMode::Never;
        let out = toml::to_string_pretty(&cfg).unwrap();
        assert!(out.contains("[display]"));
        assert!(out.contains("repo_colors = \"never\""));
    }
}
