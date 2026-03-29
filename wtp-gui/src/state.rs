//! Shared application state

use wtp_core::workspace::WorkspaceInfo;
use wtp_core::{LoadedConfig, WorkspaceManager};

/// Navigation state for the application
#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    WorkspaceList,
    WorkspaceDetail(String),
    CreateWorkspace,
    ImportRepo(String),
    Config,
}

/// Global application state, wrapped in `Entity<AppState>` for GPUI reactivity
pub struct AppState {
    /// The loaded wtp configuration
    pub loaded_config: LoadedConfig,
    /// Cached workspace list
    pub workspaces: Vec<WorkspaceInfo>,
    /// Current navigation state
    pub current_view: ViewState,
    /// Loading indicator
    pub loading: bool,
}

impl AppState {
    /// Load initial state from wtp configuration
    pub fn load() -> Self {
        let loaded_config = match LoadedConfig::load() {
            Ok((config, _warning)) => config,
            Err(e) => {
                eprintln!("Warning: failed to load config: {e}; using defaults");
                LoadedConfig {
                    config: Default::default(),
                    source_path: None,
                }
            }
        };

        let manager = WorkspaceManager::new(loaded_config.clone());
        let workspaces = manager.list_workspaces();

        Self {
            loaded_config,
            workspaces,
            current_view: ViewState::WorkspaceList,
            loading: false,
        }
    }

    /// Refresh workspace list from disk
    pub fn refresh_workspaces(&mut self) {
        let manager = WorkspaceManager::new(self.loaded_config.clone());
        self.workspaces = manager.list_workspaces();
    }
}
