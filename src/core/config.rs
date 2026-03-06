//! Configuration management for wtp
//!
//! Handles global configuration loading with priority order:
//! 1. ~/.wtp.toml
//! 2. ~/.wtp/config.toml
//! 3. ~/.config/wtp/config.toml

use crate::core::error::{Result, WtpError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

        // Create parent directories if needed
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&config_path, content)?;

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

    /// Get the path for a workspace by name
    /// Scans the workspace_root for directories with .wtp subdirectory
    pub fn get_workspace_path(&self, name: &str) -> Option<PathBuf> {
        let path = self.workspace_root.join(name);
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
    pub fn resolve_workspace_path(&self, name: &str) -> PathBuf {
        self.workspace_root.join(name)
    }

}

impl LoadedConfig {
    /// Scan all workspaces (delegates to config)
    pub fn scan_workspaces(&self) -> HashMap<String, PathBuf> {
        self.config.scan_workspaces()
    }
}
