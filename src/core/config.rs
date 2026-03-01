//! Configuration management for wtp
//!
//! Handles global configuration loading with priority order:
//! 1. ~/.wtp.toml
//! 2. ~/.wtp/config.toml
//! 3. ~/.config/wtp/config.toml

use crate::core::error::{Result, WtpError};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Default workspace root directory name
pub const DEFAULT_WORKSPACE_ROOT: &str = ".wtp/workspaces";

/// Directory name for wtp metadata inside a workspace
pub const WTP_DIR: &str = ".wtp";

/// The global configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Root directory for all workspaces (default: ~/.wtp/workspaces)
    #[serde(default = "default_workspace_root")]
    pub workspace_root: PathBuf,

    /// Map of workspace name to its path
    #[serde(default)]
    pub workspaces: IndexMap<String, PathBuf>,

    /// Host aliases mapping host name to root directory
    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,

    /// Default host alias to use when not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_host: Option<String>,

    /// Hooks configuration for workspace lifecycle events
    #[serde(default)]
    pub hooks: HooksConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// Root directory for this host
    pub root: PathBuf,
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
            workspaces: IndexMap::new(),
            hosts: HashMap::new(),
            default_host: None,
            hooks: HooksConfig::default(),
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

    /// Load configuration from the first existing config file
    /// Returns the config and an optional warning about multiple files
    pub fn load() -> Result<(Self, Option<String>)> {
        let paths = Self::config_paths();
        let mut found_paths: Vec<PathBuf> = Vec::new();
        let mut loaded_path: Option<PathBuf> = None;
        let mut config: Option<Self> = None;

        for path in &paths {
            if path.exists() {
                found_paths.push(path.clone());
                if config.is_none() {
                    let content = std::fs::read_to_string(path)?;
                    let mut cfg: Self = toml::from_str(&content)?;
                    // Expand ~ in workspace_root
                    cfg.workspace_root = shellexpand::tilde(&cfg.workspace_root.to_string_lossy())
                        .to_string()
                        .into();
                    // Expand ~ in all workspace paths
                    for path in cfg.workspaces.values_mut() {
                        *path = shellexpand::tilde(&path.to_string_lossy())
                            .to_string()
                            .into();
                    }
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
                    }
                    config = Some(cfg);
                    loaded_path = Some(path.clone());
                }
            }
        }

        let warning = if found_paths.len() > 1 {
            let files: Vec<_> = found_paths.iter().map(|p| p.display().to_string()).collect();
            Some(format!(
                "⚠️  Warning: Multiple config files found: {}. Using {}",
                files.join(", "),
                loaded_path.as_ref().unwrap().display()
            ))
        } else {
            None
        };

        match config {
            Some(cfg) => Ok((cfg, warning)),
            None => Ok((Self::default(), warning)),
        }
    }

    /// Save configuration to the default location (~/.wtp/config.toml)
    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| WtpError::config("Could not find home directory"))?
            .join(".wtp");

        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        Ok(())
    }

    /// Get the path for a workspace by name
    pub fn get_workspace_path(&self, name: &str) -> Option<&PathBuf> {
        self.workspaces.get(name)
    }

    /// Check if a workspace exists
    pub fn has_workspace(&self, name: &str) -> bool {
        self.workspaces.contains_key(name)
    }

    /// Add a new workspace
    pub fn add_workspace(&mut self, name: String, path: PathBuf) -> Result<()> {
        if let Some(existing_path) = self.workspaces.get(&name) {
            return Err(WtpError::WorkspaceAlreadyExists {
                name: name.clone(),
                path: existing_path.clone(),
            });
        }
        self.workspaces.insert(name, path);
        self.save()?;
        Ok(())
    }

    /// Remove a workspace from config
    pub fn remove_workspace(&mut self, name: &str) -> Result<Option<PathBuf>> {
        let path = self.workspaces.shift_remove(name);
        if path.is_some() {
            self.save()?;
        }
        Ok(path)
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
    pub fn resolve_workspace_path(&self, name: &str) -> PathBuf {
        self.workspace_root.join(name)
    }

    /// Expand workspace root with home directory
    pub fn expanded_workspace_root(&self) -> PathBuf {
        shellexpand::tilde(&self.workspace_root.to_string_lossy())
            .to_string()
            .into()
    }
}

/// Per-workspace configuration (stored in .wtp/config.toml)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Override default host for this workspace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_host: Option<String>,

    /// Additional host mappings specific to this workspace
    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,
}

impl WorkspaceConfig {
    /// Load workspace config from a workspace root
    pub fn load(workspace_root: &Path) -> Result<Self> {
        let config_path = workspace_root.join(WTP_DIR).join("config.toml");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&config_path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save workspace config
    pub fn save(&self, workspace_root: &Path) -> Result<()> {
        let config_dir = workspace_root.join(WTP_DIR);
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

/// Find all existing config files and their paths
pub fn find_all_config_files() -> Vec<(PathBuf, bool)> {
    GlobalConfig::config_paths()
        .into_iter()
        .map(|p| {
            let exists = p.exists();
            (p, exists)
        })
        .collect()
}
