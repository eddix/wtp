//! Shared application state.
//!
//! Holds domain data, long-lived UI selections, and async operation flags.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};
use wtp_core::git::FullGitStatus;
use wtp_core::workspace::WorkspaceInfo;
use wtp_core::worktree::WorktreeEntry;
use wtp_core::{LoadedConfig, WorkspaceManager};

/// Navigation state for the main content area.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    WorkspaceList,
    WorkspaceDetail(String),
    CreateWorkspace,
    ImportRepo(String),
    Config,
}

/// Flash banner styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLevel {
    Success,
    Warning,
    Error,
    Info,
}

/// Transient message shown in the content area.
#[derive(Debug, Clone)]
pub struct FlashMessage {
    pub level: FlashLevel,
    pub message: String,
}

/// Selected repository during import flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSelection {
    pub host: String,
    pub repo_path: String,
}

impl RepoSelection {
    pub fn display(&self) -> String {
        format!("{}:{}", self.host, self.repo_path)
    }
}

/// Cached information for a single worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub entry: WorktreeEntry,
    pub abs_path: PathBuf,
    pub status: Option<FullGitStatus>,
    pub head_info: Option<(String, String, String)>,
    pub base_ahead_behind: Option<(u32, u32)>,
}

/// Global application state.
pub struct AppState {
    pub loaded_config: LoadedConfig,
    pub workspaces: Vec<WorkspaceInfo>,
    pub workspace_list_cursor: usize,
    pub current_view: ViewState,
    pub flash: Option<FlashMessage>,
    pub loading_worktrees: bool,
    pub creating_workspace: bool,
    pub scanning_host: Option<String>,
    pub importing_repo: bool,
    pub current_detail_workspace: Option<String>,
    pub current_worktrees: Vec<WorktreeInfo>,
    pub create_run_hook: bool,
    pub selected_import_host: Option<String>,
    pub selected_import_repo: Option<RepoSelection>,
    pub import_base_hint: Option<String>,
    pub resolving_import_base: bool,
    pub scanned_repos: HashMap<String, Vec<String>>,
    runtime: Option<Arc<Runtime>>,
}

impl AppState {
    /// Load initial state from wtp configuration.
    pub fn load() -> Self {
        let mut flash = None;
        let loaded_config = match LoadedConfig::load() {
            Ok((config, warning)) => {
                if let Some(warning) = warning {
                    flash = Some(FlashMessage {
                        level: FlashLevel::Warning,
                        message: warning,
                    });
                }
                config
            }
            Err(error) => {
                eprintln!("Warning: failed to load config: {error}; using defaults");
                flash = Some(FlashMessage {
                    level: FlashLevel::Warning,
                    message: format!("Failed to load config, using defaults: {error}"),
                });
                LoadedConfig {
                    config: Default::default(),
                    source_path: None,
                }
            }
        };

        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .ok()
            .map(Arc::new);
        let workspaces = WorkspaceManager::new(loaded_config.clone()).list_workspaces();

        Self {
            loaded_config,
            workspaces,
            workspace_list_cursor: 0,
            current_view: ViewState::WorkspaceList,
            flash,
            loading_worktrees: false,
            creating_workspace: false,
            scanning_host: None,
            importing_repo: false,
            current_detail_workspace: None,
            current_worktrees: Vec::new(),
            create_run_hook: true,
            selected_import_host: None,
            selected_import_repo: None,
            import_base_hint: None,
            resolving_import_base: false,
            scanned_repos: HashMap::new(),
            runtime,
        }
    }

    /// Refresh workspace list from disk.
    pub fn refresh_workspaces(&mut self) {
        self.workspaces = WorkspaceManager::new(self.loaded_config.clone()).list_workspaces();
        if self.workspaces.is_empty() {
            self.workspace_list_cursor = 0;
        } else {
            self.workspace_list_cursor = self
                .workspace_list_cursor
                .min(self.workspaces.len().saturating_sub(1));
        }
    }

    /// Reload configuration from disk.
    pub fn reload_config(&mut self) -> Result<Option<String>, String> {
        let (loaded_config, warning) = LoadedConfig::load().map_err(|error| error.to_string())?;
        self.loaded_config = loaded_config;
        self.refresh_workspaces();

        if self
            .selected_import_host
            .as_ref()
            .is_some_and(|host| !self.loaded_config.config.hosts.contains_key(host))
        {
            self.selected_import_host = None;
        }

        Ok(warning)
    }

    /// Navigate to a new view and reset stale state that should not leak across screens.
    pub fn navigate(&mut self, view: ViewState) {
        self.flash = None;

        match view {
            ViewState::WorkspaceList => {
                self.reset_workspace_detail();
                self.reset_import_flow(true);
                if self.workspace_list_cursor >= self.workspaces.len()
                    && !self.workspaces.is_empty()
                {
                    self.workspace_list_cursor = self.workspaces.len() - 1;
                }
                self.current_view = ViewState::WorkspaceList;
            }
            ViewState::WorkspaceDetail(name) => {
                self.open_workspace_detail(name, false);
            }
            ViewState::CreateWorkspace => {
                self.reset_workspace_detail();
                self.reset_import_flow(true);
                self.current_view = ViewState::CreateWorkspace;
            }
            ViewState::ImportRepo(workspace_name) => {
                self.current_detail_workspace = Some(workspace_name.clone());
                self.loading_worktrees = false;
                self.current_view = ViewState::ImportRepo(workspace_name);
                self.selected_import_repo = None;
                self.import_base_hint = None;
                self.resolving_import_base = false;
                self.importing_repo = false;
                self.scanning_host = None;
                if self.selected_import_host.is_none() {
                    self.selected_import_host = self.default_host_alias();
                }
            }
            ViewState::Config => {
                self.reset_workspace_detail();
                self.reset_import_flow(true);
                self.current_view = ViewState::Config;
            }
        }
    }

    /// Navigate to a workspace detail screen and optionally force a reload.
    pub fn open_workspace_detail(&mut self, name: impl Into<String>, refresh: bool) {
        let name = name.into();
        let needs_reload = refresh
            || self.current_detail_workspace.as_deref() != Some(name.as_str())
            || self.current_worktrees.is_empty();

        if let Some(index) = self
            .workspaces
            .iter()
            .position(|workspace| workspace.name == name)
        {
            self.workspace_list_cursor = index;
        }

        self.current_detail_workspace = Some(name.clone());
        self.current_view = ViewState::WorkspaceDetail(name);
        self.reset_import_flow(false);

        if needs_reload {
            self.current_worktrees.clear();
            self.loading_worktrees = true;
        } else {
            self.loading_worktrees = false;
        }
    }

    pub fn set_flash(&mut self, level: FlashLevel, message: impl Into<String>) {
        self.flash = Some(FlashMessage {
            level,
            message: message.into(),
        });
    }

    pub fn clear_flash(&mut self) {
        self.flash = None;
    }

    pub fn tokio_runtime(&self) -> Option<Arc<Runtime>> {
        self.runtime.clone()
    }

    pub fn select_import_host(&mut self, alias: impl Into<String>) {
        self.selected_import_host = Some(alias.into());
        self.selected_import_repo = None;
        self.import_base_hint = None;
        self.resolving_import_base = false;
    }

    pub fn select_import_repo(&mut self, host: impl Into<String>, repo_path: impl Into<String>) {
        self.selected_import_repo = Some(RepoSelection {
            host: host.into(),
            repo_path: repo_path.into(),
        });
        self.import_base_hint = None;
        self.resolving_import_base = true;
    }

    pub fn move_workspace_cursor(&mut self, delta: isize) {
        if self.workspaces.is_empty() {
            self.workspace_list_cursor = 0;
            return;
        }

        let last_index = self.workspaces.len().saturating_sub(1) as isize;
        let next = (self.workspace_list_cursor as isize + delta).clamp(0, last_index);
        self.workspace_list_cursor = next as usize;
    }

    pub fn selected_workspace_name(&self) -> Option<String> {
        self.workspaces
            .get(self.workspace_list_cursor)
            .map(|workspace| workspace.name.clone())
    }

    fn reset_workspace_detail(&mut self) {
        self.loading_worktrees = false;
        self.current_detail_workspace = None;
        self.current_worktrees.clear();
    }

    fn reset_import_flow(&mut self, clear_scanned_repos: bool) {
        self.selected_import_repo = None;
        self.import_base_hint = None;
        self.resolving_import_base = false;
        self.scanning_host = None;
        self.importing_repo = false;
        if clear_scanned_repos {
            self.scanned_repos.clear();
        }
    }

    fn default_host_alias(&self) -> Option<String> {
        self.loaded_config
            .config
            .default_host
            .clone()
            .or_else(|| self.loaded_config.config.hosts.keys().next().cloned())
    }
}
