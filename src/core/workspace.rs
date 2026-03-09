//! Workspace management

use crate::core::config::{GlobalConfig, LoadedConfig, WTP_DIR};
use crate::core::error::{Result, WtpError};
use crate::core::fence::Fence;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Manages workspaces and their discovery
pub struct WorkspaceManager {
    loaded_config: LoadedConfig,
}

impl WorkspaceManager {
    pub fn new(loaded_config: LoadedConfig) -> Self {
        Self { loaded_config }
    }

    /// Get a reference to the global config
    pub fn global_config(&self) -> &GlobalConfig {
        &self.loaded_config.config
    }

    /// Get a mutable reference to the global config
    pub fn global_config_mut(&mut self) -> &mut GlobalConfig {
        &mut self.loaded_config.config
    }

    /// Get a reference to the loaded config (includes source path)
    pub fn loaded_config(&self) -> &LoadedConfig {
        &self.loaded_config
    }

    /// List all workspaces
    pub fn list_workspaces(&self) -> Vec<WorkspaceInfo> {
        self.loaded_config
            .scan_workspaces()
            .into_iter()
            .map(|(name, path)| {
                WorkspaceInfo {
                    name,
                    path,
                    exists: true, // Scanned workspaces always exist
                }
            })
            .collect()
    }

    /// Create a new workspace
    pub async fn create_workspace(&mut self, name: &str, run_hook: bool) -> Result<PathBuf> {
        let workspace_path = self.global_config().resolve_workspace_path(name);

        // Check if workspace already exists (directory with .wtp subdirectory)
        if workspace_path.join(WTP_DIR).exists() {
            return Err(WtpError::WorkspaceAlreadyExists {
                name: name.to_string(),
                path: workspace_path.clone(),
            });
        }

        // Check if directory exists but is not a workspace (no .wtp subdirectory)
        if workspace_path.exists() {
            return Err(WtpError::config(format!(
                "Directory '{}' already exists but is not a wtp workspace",
                workspace_path.display()
            )));
        }

        // Create workspace directory structure with fence protection
        let fence = Fence::from_config(&self.loaded_config.config);
        self.initialize_workspace_dir(&workspace_path, &fence)?;

        // Run post-create hook if configured and enabled
        if run_hook {
            if let Err(e) = self.run_create_hook(name, &workspace_path).await {
                eprintln!("Warning: Failed to run create hook: {}", e);
            }
        }

        Ok(workspace_path)
    }

    /// Run the on_create hook script
    async fn run_create_hook(&self, name: &str, path: &Path) -> Result<()> {
        let Some(hook_path) = &self.loaded_config.config.hooks.on_create else {
            return Ok(());
        };

        if !hook_path.exists() {
            return Err(WtpError::config(format!(
                "Create hook not found: {}",
                hook_path.display()
            )));
        }

        // Check if hook is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(hook_path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(WtpError::config(format!(
                    "Create hook is not executable: {}",
                    hook_path.display()
                )));
            }
        }

        // Run the hook with environment variables
        let mut cmd = tokio::process::Command::new(hook_path);
        cmd.env("WTP_WORKSPACE_NAME", name)
            .env("WTP_WORKSPACE_PATH", path.as_os_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            WtpError::config(format!("Failed to execute create hook: {}", e))
        })?;

        // Print hook stdout/stderr for user visibility
        if !output.stdout.is_empty() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", stdout);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WtpError::config(format!(
                "Create hook failed with exit code {}: {}",
                output.status.code().unwrap_or(-1),
                stderr
            )));
        }

        Ok(())
    }

    /// Initialize workspace directory structure
    fn initialize_workspace_dir(&self, path: &Path, fence: &Fence) -> Result<()> {
        // Create main directory
        fence.create_dir_all(path)?;

        // Create .wtp directory
        let wtp_dir = path.join(WTP_DIR);
        fence.create_dir_all(&wtp_dir)?;

        // Create empty worktree.toml
        let worktree_toml = crate::core::worktree::WorktreeToml::new();
        worktree_toml.save_with_fence(&wtp_dir.join("worktree.toml"), fence)?;

        Ok(())
    }

    /// Remove a workspace directory.
    /// Caller is responsible for ejecting worktrees before calling this.
    pub fn remove_workspace(&mut self, name: &str, delete_dir: bool) -> Result<Option<PathBuf>> {
        let path = self.global_config().get_workspace_path(name);

        if let Some(ref p) = path {
            if delete_dir && p.exists() {
                crate::core::fence::ensure_fence(&self.loaded_config.config).remove_dir_all(p)?;
            }
        }

        Ok(path)
    }

    /// Try to match a repository path to a host alias
    pub fn match_host_alias(&self, repo_path: &Path) -> Option<(String, String)> {
        for (alias, host_config) in &self.global_config().hosts {
            if let Ok(rel) = repo_path.strip_prefix(&host_config.root) {
                return Some((alias.clone(), rel.to_string_lossy().to_string()));
            }
        }
        None
    }

    /// Get all host aliases
    pub fn get_hosts(&self) -> &HashMap<String, crate::core::config::HostConfig> {
        &self.global_config().hosts
    }

    /// Detect and validate the current workspace from the working directory.
    ///
    /// Walks up from `cwd` looking for a `.wtp` directory, then verifies the
    /// workspace directory and its metadata exist. Returns `(name, path)`.
    pub fn require_current_workspace(&self) -> Result<(String, PathBuf)> {
        let (name, path) = self.detect_current_workspace()?;
        if !path.exists() {
            return Err(WtpError::config(format!(
                "Workspace '{}' directory does not exist at {}",
                name,
                path.display()
            )));
        }
        if !path.join(WTP_DIR).exists() {
            return Err(WtpError::config(format!(
                "Workspace '{}' is missing its .wtp directory. It may be corrupted.",
                name
            )));
        }
        Ok((name, path))
    }

    /// Detect the current workspace from the working directory.
    ///
    /// Walks up from `cwd` looking for a `.wtp` directory, then matches it
    /// against registered workspaces. Returns `(name, path)`.
    pub fn detect_current_workspace(&self) -> Result<(String, PathBuf)> {
        let current_dir = std::env::current_dir()?;
        let mut check_dir = current_dir.as_path();

        loop {
            if check_dir.join(WTP_DIR).is_dir() {
                // Try to match against registered workspaces
                for (name, path) in self.loaded_config.scan_workspaces().iter() {
                    if path == check_dir {
                        return Ok((name.clone(), path.clone()));
                    }
                }
                // Directory has .wtp but is not registered — use dir name
                let name = check_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string();
                return Ok((name, check_dir.to_path_buf()));
            }

            match check_dir.parent() {
                Some(parent) => check_dir = parent,
                None => break,
            }
        }

        Err(WtpError::NotInWorkspace)
    }
}

/// Information about a workspace
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub name: String,
    pub path: PathBuf,
    pub exists: bool,
}
