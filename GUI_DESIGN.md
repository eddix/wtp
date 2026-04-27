# wtp-gui — Complete Implementation Design

> Verified against GPUI 0.2.2 (crates.io) / Zed v0.229.0 (2026-03-25), tray-icon 0.21.3, and gpui-component 0.5.0.
> All API signatures validated via docs.rs, gpui.rs, and Zed source examples.

---

## Table of Contents

- [A. Dependencies & Cargo.toml](#a-dependencies--cargotoml)
- [B. File-by-File Implementation Specs](#b-file-by-file-implementation-specs)
  - [B.1 main.rs](#b1-mainrs)
  - [B.2 state.rs](#b2-staters)
  - [B.3 tray.rs](#b3-trayrs)
  - [B.4 app.rs](#b4-apprs)
  - [B.5 views/workspace_list.rs](#b5-viewsworkspace_listrs)
  - [B.6 views/workspace_detail.rs](#b6-viewsworkspace_detailrs)
  - [B.7 views/create_workspace.rs](#b7-viewscreate_workspacers)
  - [B.8 views/import_repo.rs](#b8-viewsimport_repors)
  - [B.9 views/config_panel.rs](#b9-viewsconfig_panelrs)
  - [B.10 components/search_input.rs](#b10-componentssearch_inputrs)
  - [B.11 components/status_badge.rs](#b11-componentsstatus_badgers)
- [C. Technical Key Points](#c-technical-key-points)
  - [C.1 GPUI + tray-icon Event Loop Coordination](#c1-gpui--tray-icon-event-loop-coordination)
  - [C.2 Async Operation Pattern](#c2-async-operation-pattern)
  - [C.3 Icon Asset Handling](#c3-icon-asset-handling)
  - [C.4 Navigation & State Flow](#c4-navigation--state-flow)
- [D. ADRs](#d-adrs)

---

## A. Dependencies & Cargo.toml

### Decision: Use `gpui` from crates.io (v0.2.2) + `gpui-component` from git

**Rationale**: GPUI 0.2.2 is the latest stable release on crates.io (published 2025-10-22). However, GPUI is pre-1.0 with frequent breaking changes, and `gpui-component` tracks the latest Zed main branch. For best compatibility between GPUI and gpui-component, we use the git dependency pinned to Zed v0.229.0 (commit `7c07887`).

**Alternative considered**: Using `gpui = "0.2.2"` from crates.io. Rejected because `gpui-component` (which we need for Input) requires a specific GPUI version from git that may diverge from the crates.io release.

### Complete `wtp-gui/Cargo.toml`

```toml
[package]
name = "wtp-gui"
version = "0.1.0"
edition = "2024"
authors = ["eddix <eli.tech.arm@gmail.com>"]
description = "GUI application for wtp — WorkTree for Polyrepo"
license = "MIT"
repository = "https://github.com/eddix/wtp"
rust-version = "1.90"

[[bin]]
name = "wtp-gui"
path = "src/main.rs"

[dependencies]
wtp-core = { path = "../wtp-core" }

# GUI framework — pinned to Zed v0.229.0 for API stability
# GPUI API: Application::new(), app.run(), cx.open_window(), Render trait, div(), Entity
gpui = { git = "https://github.com/zed-industries/zed", package = "gpui", rev = "7c07887" }

# High-level UI components (Input, List, Button, etc.)
# Provides InputState, TextInput, and 60+ components
gpui-component = { git = "https://github.com/longbridge/gpui-component" }

# System tray — v0.21 is latest stable; macOS requires main-thread creation
tray-icon = "0.21"

# Image processing for tray icon (decode PNG → RGBA)
image = { version = "0.25", default-features = false, features = ["png"] }

# Async runtime (shared with wtp-core for create_workspace)
tokio = { version = "1.40", features = ["rt-multi-thread", "macros"] }

# Error handling
anyhow = "1.0"

# Timer for tray event polling
smol = "2.0"
```

### Additional assets needed

```
wtp-gui/
└── assets/
    └── icon.png          # 22x22 PNG, monochrome tray icon (white on transparent)
```

> **Note**: Create a simple monochrome "W" or git-branch icon as `assets/icon.png`. 22x22 pixels, white foreground on transparent background, for macOS menu bar compatibility.

---

## B. File-by-File Implementation Specs

### B.1 `main.rs`

**Purpose**: Entry point. Initialize GPUI Application, set up system tray, optionally open main window.

```rust
//! wtp-gui entry point
//!
//! Initializes the GPUI application, sets up the system tray icon,
//! and enters the main event loop.

mod app;
mod components;
mod state;
mod tray;
mod views;

use gpui::Application;

fn main() {
    Application::new().run(|cx| {
        // Initialize gpui-component (required before using any gpui-component widgets)
        gpui_component::init(cx);

        // Create shared application state as a GPUI Entity
        let state = cx.new(|_cx| state::AppState::load());

        // Set up system tray icon with workspace quick-access menu
        tray::setup_tray(cx, state.clone());

        // Open the main window immediately on launch
        app::open_main_window(cx, state);
    });
}
```

**Key points**:
- `Application::new().run(|cx| { ... })` — GPUI's entry point. The closure receives `&mut App`.
- `cx.new(|_cx| ...)` — Creates an `Entity<AppState>`, GPUI's managed state container.
- `gpui_component::init(cx)` — **Must** be called before using any gpui-component widgets (Input, etc.).
- The tray icon is created inside the run callback, on the main thread (macOS requirement).

---

### B.2 `state.rs`

**Purpose**: Shared application state wrapped in `Entity<AppState>` for GPUI reactivity.

```rust
//! Shared application state
//!
//! AppState holds all data the GUI needs: config, workspace list, navigation.
//! Wrapped in Entity<AppState> for GPUI's reactive observation system.

use wtp_core::workspace::WorkspaceInfo;
use wtp_core::worktree::WorktreeEntry;
use wtp_core::git::{GitStatus, FullGitStatus};
use wtp_core::{LoadedConfig, WorkspaceManager, WorktreeManager, GitClient};
use std::path::PathBuf;
use std::collections::HashMap;

/// Navigation state — determines which view is rendered in the content area
#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    WorkspaceList,
    WorkspaceDetail(String),   // workspace name
    CreateWorkspace,
    ImportRepo(String),        // target workspace name
    Config,
}

/// Cached information for a single worktree (pre-fetched git data)
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub entry: WorktreeEntry,
    pub abs_path: PathBuf,
    pub status: Option<FullGitStatus>,
    pub head_info: Option<(String, String, String)>,  // (short_hash, subject, relative_time)
    pub base_ahead_behind: Option<(u32, u32)>,
}

/// Global application state
pub struct AppState {
    /// The loaded wtp configuration
    pub loaded_config: LoadedConfig,
    /// Cached workspace list
    pub workspaces: Vec<WorkspaceInfo>,
    /// Current navigation state
    pub current_view: ViewState,
    /// Loading indicator
    pub loading: bool,
    /// Error message to display (transient)
    pub error_message: Option<String>,
    /// Cached worktree details for the currently viewed workspace
    pub current_worktrees: Vec<WorktreeInfo>,
    /// Search/filter query for import repo view
    pub import_search_query: String,
    /// Scanned repos for import (host_alias -> Vec<repo_relative_path>)
    pub scanned_repos: HashMap<String, Vec<String>>,
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
            error_message: None,
            current_worktrees: Vec::new(),
            import_search_query: String::new(),
            scanned_repos: HashMap::new(),
        }
    }

    /// Refresh workspace list from disk
    pub fn refresh_workspaces(&mut self) {
        let manager = WorkspaceManager::new(self.loaded_config.clone());
        self.workspaces = manager.list_workspaces();
    }

    /// Navigate to a new view
    pub fn navigate(&mut self, view: ViewState) {
        self.error_message = None;  // Clear errors on navigation
        self.current_view = view;
    }

    /// Get a WorkspaceManager instance
    pub fn workspace_manager(&self) -> WorkspaceManager {
        WorkspaceManager::new(self.loaded_config.clone())
    }

    /// Get host roots for resolving repo paths
    pub fn host_roots(&self) -> &indexmap::IndexMap<String, wtp_core::config::HostConfig> {
        &self.loaded_config.config.hosts
    }
}
```

**GPUI pattern**: `Entity<AppState>` is created once and shared by all views. Any mutation through `entity.update(cx, |state, cx| { state.xxx(); cx.notify(); })` triggers observers to re-render.

---

### B.3 `tray.rs`

**Purpose**: System tray icon with workspace quick-access menu. Bridges tray-icon events to GPUI.

```rust
//! System tray integration via tray-icon crate
//!
//! Creates a macOS menu bar icon with workspace quick-access.
//! Bridges tray-icon's channel-based events to GPUI's main thread.

use gpui::{App, Entity, AsyncApp};
use tray_icon::{TrayIcon, TrayIconBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, MenuId};
use crate::state::AppState;
use std::time::Duration;

// Menu item ID constants
const MENU_OPEN_DASHBOARD: &str = "open_dashboard";
const MENU_CREATE_WORKSPACE: &str = "create_workspace";
const MENU_QUIT: &str = "quit";

/// Hold the TrayIcon to prevent it from being dropped (which removes the icon)
static mut TRAY_HANDLE: Option<TrayIcon> = None;

/// Set up the system tray icon and menu
pub fn setup_tray(cx: &mut App, state: Entity<AppState>) {
    let icon = load_tray_icon();
    let menu = build_tray_menu(&state, cx);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("wtp — WorkTree for Polyrepo")
        .build()
        .expect("Failed to create tray icon");

    // SAFETY: Single-threaded access, only called once from main()
    unsafe { TRAY_HANDLE = Some(tray); }

    // Poll MenuEvent receiver in an async task on GPUI's executor
    // This bridges tray-icon's std::sync::mpsc channel to GPUI's main thread
    let state_clone = state.clone();
    cx.spawn(|mut cx: AsyncApp| async move {
        let receiver = MenuEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                let state = state_clone.clone();
                cx.update(|cx| {
                    handle_menu_event(&event.id, cx, &state);
                }).ok();
            }
            smol::Timer::after(Duration::from_millis(100)).await;
        }
    }).detach();
}

fn build_tray_menu(state: &Entity<AppState>, cx: &App) -> Menu {
    let menu = Menu::new();

    // Add workspace items from state
    let workspaces = state.read(cx);
    for ws in &workspaces.workspaces {
        let item = MenuItem::with_id(
            MenuId::new(&format!("ws:{}", ws.name)),
            &ws.name,
            true,  // enabled
            None,  // no accelerator
        );
        let _ = menu.append(&item);
    }

    if !workspaces.workspaces.is_empty() {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }

    let _ = menu.append(&MenuItem::with_id(
        MenuId::new(MENU_OPEN_DASHBOARD),
        "Open Dashboard...",
        true,
        None,
    ));

    let _ = menu.append(&MenuItem::with_id(
        MenuId::new(MENU_CREATE_WORKSPACE),
        "Create Workspace...",
        true,
        None,
    ));

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(
        MenuId::new(MENU_QUIT),
        "Quit wtp",
        true,
        None,
    ));

    menu
}

fn handle_menu_event(id: &MenuId, cx: &mut App, state: &Entity<AppState>) {
    let id_str = id.as_ref();
    match id_str {
        MENU_QUIT => {
            cx.quit();
        }
        MENU_OPEN_DASHBOARD => {
            crate::app::open_main_window(cx, state.clone());
        }
        MENU_CREATE_WORKSPACE => {
            state.update(cx, |state, _cx| {
                state.navigate(crate::state::ViewState::CreateWorkspace);
            });
            crate::app::open_main_window(cx, state.clone());
        }
        other if other.starts_with("ws:") => {
            let ws_name = &other[3..];
            state.update(cx, |state, _cx| {
                state.navigate(crate::state::ViewState::WorkspaceDetail(ws_name.to_string()));
            });
            crate::app::open_main_window(cx, state.clone());
        }
        _ => {}
    }
}

fn load_tray_icon() -> tray_icon::Icon {
    let icon_bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(icon_bytes)
        .expect("Failed to decode tray icon PNG");
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
        .expect("Failed to create tray Icon from RGBA data")
}
```

**Key integration detail**: tray-icon uses `MenuEvent::receiver()` returning a `std::sync::mpsc::Receiver`. We cannot directly hook this into GPUI's event loop (GPUI doesn't expose a winit `EventLoopProxy`). Instead, we poll with `cx.spawn()` + `smol::Timer` at 100ms intervals and dispatch to the main thread via `cx.update()`.

**WARNING**: The `static mut TRAY_HANDLE` pattern is not ideal. A better approach would be to store the `TrayIcon` in a GPUI Entity, but TrayIcon is not `Send`. Since this is single-threaded and only set once, this is acceptable for v1.

---

### B.4 `app.rs`

**Purpose**: MainWindow — top-level window container with sidebar navigation and content area.

```rust
//! Main application window
//!
//! Implements a sidebar + content layout. The sidebar provides navigation
//! between views; the content area renders the active view.

use gpui::*;
use crate::state::{AppState, ViewState};
use crate::views;

/// Actions for keyboard shortcuts
actions!(wtp_gui, [
    NavigateToWorkspaces,
    NavigateToConfig,
    RefreshWorkspaces,
]);

/// Top-level window container
pub struct MainWindow {
    state: Entity<AppState>,
    focus_handle: FocusHandle,
}

impl MainWindow {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Observe state changes to trigger re-renders
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        }).detach();

        Self { state, focus_handle }
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::handle_navigate_workspaces))
            .on_action(cx.listener(Self::handle_navigate_config))
            .on_action(cx.listener(Self::handle_refresh))
            .flex()
            .size_full()
            .bg(rgb(0x1e1e2e))  // Dark background (Catppuccin Mocha base)
            .text_color(rgb(0xcdd6f4))  // Light text
            .child(self.render_sidebar(state, cx))
            .child(self.render_content(state, cx))
    }
}

impl MainWindow {
    fn render_sidebar(&self, state: &AppState, cx: &mut Context<Self>) -> impl IntoElement {
        let current_view = state.current_view.clone();
        let state_entity = self.state.clone();

        div()
            .w(px(220.0))
            .h_full()
            .flex_col()
            .bg(rgb(0x181825))  // Slightly darker sidebar
            .border_r_1()
            .border_color(rgb(0x313244))
            // App title
            .child(
                div()
                    .px(px(16.0))
                    .py(px(12.0))
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("wtp")
            )
            // Navigation items
            .child(self.sidebar_item(
                "Workspaces",
                matches!(current_view, ViewState::WorkspaceList | ViewState::WorkspaceDetail(_)),
                {
                    let s = state_entity.clone();
                    move |cx: &mut Context<MainWindow>| {
                        s.update(cx, |state, _cx| {
                            state.navigate(ViewState::WorkspaceList);
                        });
                    }
                },
                cx,
            ))
            .child(self.sidebar_item(
                "Config",
                matches!(current_view, ViewState::Config),
                {
                    let s = state_entity.clone();
                    move |cx: &mut Context<MainWindow>| {
                        s.update(cx, |state, _cx| {
                            state.navigate(ViewState::Config);
                        });
                    }
                },
                cx,
            ))
    }

    fn sidebar_item(
        &self,
        label: &str,
        active: bool,
        on_click: impl Fn(&mut Context<MainWindow>) + 'static,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg = if active { rgb(0x313244) } else { rgb(0x181825) };
        let border = if active { rgb(0x89b4fa) } else { rgb(0x181825) };

        div()
            .px(px(16.0))
            .py(px(8.0))
            .mx(px(8.0))
            .rounded(px(6.0))
            .bg(bg)
            .border_l_2()
            .border_color(border)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x313244)))
            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                on_click(cx);
            })
            .child(label.to_string())
    }

    fn render_content(&self, state: &AppState, cx: &mut Context<Self>) -> impl IntoElement {
        let state_entity = self.state.clone();

        div()
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .p(px(24.0))
            .child(
                // Error banner (if any)
                if let Some(ref err) = state.error_message {
                    div()
                        .w_full()
                        .px(px(12.0))
                        .py(px(8.0))
                        .mb(px(16.0))
                        .rounded(px(6.0))
                        .bg(rgb(0xf38ba8))  // Red
                        .text_color(rgb(0x1e1e2e))
                        .child(err.clone())
                } else {
                    div()
                }
            )
            .child(match &state.current_view {
                ViewState::WorkspaceList => {
                    views::workspace_list::render(state, &state_entity, cx)
                }
                ViewState::WorkspaceDetail(name) => {
                    views::workspace_detail::render(name, state, &state_entity, cx)
                }
                ViewState::CreateWorkspace => {
                    views::create_workspace::render(state, &state_entity, cx)
                }
                ViewState::ImportRepo(ws_name) => {
                    views::import_repo::render(ws_name, state, &state_entity, cx)
                }
                ViewState::Config => {
                    views::config_panel::render(state, &state_entity, cx)
                }
            })
    }

    // Action handlers
    fn handle_navigate_workspaces(&mut self, _: &NavigateToWorkspaces, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.navigate(ViewState::WorkspaceList);
        });
    }

    fn handle_navigate_config(&mut self, _: &NavigateToConfig, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.navigate(ViewState::Config);
        });
    }

    fn handle_refresh(&mut self, _: &RefreshWorkspaces, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.refresh_workspaces();
        });
    }
}

/// Open (or focus) the main window
pub fn open_main_window(cx: &mut App, state: Entity<AppState>) {
    let bounds = Bounds::centered(
        None,
        size(px(1000.0), px(700.0)),
        cx,
    );

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |_window, cx| {
            cx.new(|cx| MainWindow::new(state, cx))
        },
    ).unwrap();
}
```

**Architecture decision**: Views are implemented as **render functions** (not standalone `impl Render` structs) that return `impl IntoElement`. This is simpler than full GPUI views for content panels that don't need independent focus handling. Only the top-level `MainWindow` implements `Render` + `Focusable`.

---

### B.5 `views/workspace_list.rs`

**Purpose**: Display all workspaces with name, path, and actions (open detail, create new).

```rust
//! Workspace list panel
//!
//! Displays all workspaces with summary information.
//! Provides actions: view detail, create new workspace.

use gpui::*;
use crate::state::{AppState, ViewState};
use crate::app::MainWindow;

/// Render the workspace list view
pub fn render(
    state: &AppState,
    state_entity: &Entity<AppState>,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let state_entity = state_entity.clone();

    div()
        .flex_col()
        .gap(px(16.0))
        // Header row
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div().text_2xl().font_weight(FontWeight::BOLD).child("Workspaces")
                )
                .child(
                    // "New Workspace" button
                    {
                        let s = state_entity.clone();
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x89b4fa))  // Blue accent
                            .text_color(rgb(0x1e1e2e))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x74c7ec)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                s.update(cx, |state, _| {
                                    state.navigate(ViewState::CreateWorkspace);
                                });
                            })
                            .child("+ New Workspace")
                    }
                )
        )
        // Workspace cards
        .child(
            div()
                .flex_col()
                .gap(px(8.0))
                .children(
                    state.workspaces.iter().map(|ws| {
                        let ws_name = ws.name.clone();
                        let ws_path = ws.path.display().to_string();
                        let s = state_entity.clone();

                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(16.0))
                            .py(px(12.0))
                            .rounded(px(8.0))
                            .bg(rgb(0x313244))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x45475a)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let name = ws_name.clone();
                                s.update(cx, |state, _| {
                                    state.navigate(ViewState::WorkspaceDetail(name));
                                });
                            })
                            .child(
                                div()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div().font_weight(FontWeight::SEMIBOLD).child(ws.name.clone())
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0xa6adc8))
                                            .child(ws_path)
                                    )
                            )
                            .child(
                                div().text_color(rgb(0x6c7086)).child("→")
                            )
                    })
                )
        )
        // Empty state
        .when(state.workspaces.is_empty(), |el| {
            el.child(
                div()
                    .flex()
                    .justify_center()
                    .py(px(48.0))
                    .text_color(rgb(0x6c7086))
                    .child("No workspaces found. Create one to get started.")
            )
        })
}
```

---

### B.6 `views/workspace_detail.rs`

**Purpose**: Show a single workspace's worktrees with git status, branch, commit info, and actions.

```rust
//! Single workspace detail view
//!
//! Shows all worktrees in a workspace with git status, branches, and actions.
//! Loads worktree data asynchronously via cx.spawn().

use gpui::*;
use crate::state::{AppState, ViewState, WorktreeInfo};
use crate::app::MainWindow;
use crate::components::status_badge;
use wtp_core::{WorktreeManager, GitClient};

/// Render the workspace detail view
pub fn render(
    ws_name: &str,
    state: &AppState,
    state_entity: &Entity<AppState>,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let ws_name_owned = ws_name.to_string();
    let state_entity = state_entity.clone();

    // Find workspace path
    let ws_path = state.workspaces.iter()
        .find(|ws| ws.name == ws_name)
        .map(|ws| ws.path.clone());

    // Trigger async loading if worktrees haven't been loaded yet
    if state.current_worktrees.is_empty() && !state.loading {
        if let Some(ref path) = ws_path {
            let path = path.clone();
            let hosts = state.loaded_config.config.hosts.clone();
            let s = state_entity.clone();
            cx.spawn(|_this, mut cx| async move {
                let worktrees = load_worktree_details(&path, &hosts).await;
                cx.update(|cx| {
                    s.update(cx, |state, _cx| {
                        state.current_worktrees = worktrees;
                        state.loading = false;
                    });
                }).ok();
            }).detach();

            // Mark as loading
            state_entity.update(cx, |state, _| { state.loading = true; });
        }
    }

    div()
        .flex_col()
        .gap(px(16.0))
        // Header with back button
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child({
                    let s = state_entity.clone();
                    div()
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(0x89b4fa)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            s.update(cx, |state, _| {
                                state.current_worktrees.clear();
                                state.navigate(ViewState::WorkspaceList);
                            });
                        })
                        .child("← Back")
                })
                .child(
                    div().text_2xl().font_weight(FontWeight::BOLD)
                        .child(ws_name_owned.clone())
                )
                .child({
                    let s = state_entity.clone();
                    let name = ws_name_owned.clone();
                    div()
                        .px(px(12.0)).py(px(6.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x89b4fa))
                        .text_color(rgb(0x1e1e2e))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x74c7ec)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            s.update(cx, |state, _| {
                                state.navigate(ViewState::ImportRepo(name.clone()));
                            });
                        })
                        .child("+ Import Repo")
                })
        )
        // Loading indicator
        .when(state.loading, |el| {
            el.child(
                div().text_color(rgb(0xa6adc8)).child("Loading worktree status...")
            )
        })
        // Worktree cards
        .children(
            state.current_worktrees.iter().map(|wt| {
                render_worktree_card(wt, cx)
            })
        )
        // Empty state
        .when(!state.loading && state.current_worktrees.is_empty(), |el| {
            el.child(
                div()
                    .flex().justify_center().py(px(48.0))
                    .text_color(rgb(0x6c7086))
                    .child("No worktrees in this workspace. Import a repo to get started.")
            )
        })
}

fn render_worktree_card(
    wt: &WorktreeInfo,
    _cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let repo_display = wt.entry.repo.display();
    let branch = wt.entry.branch.clone();

    div()
        .w_full()
        .px(px(16.0)).py(px(12.0))
        .rounded(px(8.0))
        .bg(rgb(0x313244))
        .flex_col()
        .gap(px(8.0))
        // Row 1: repo name + branch
        .child(
            div()
                .flex().items_center().justify_between()
                .child(
                    div().font_weight(FontWeight::SEMIBOLD).child(repo_display)
                )
                .child(
                    div()
                        .px(px(8.0)).py(px(2.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x45475a))
                        .text_sm()
                        .child(branch)
                )
        )
        // Row 2: git status badge
        .child(
            if let Some(ref full_status) = wt.status {
                status_badge::render(&full_status.status)
            } else {
                div().text_sm().text_color(rgb(0x6c7086)).child("status unknown")
            }
        )
        // Row 3: last commit info
        .child(
            if let Some(ref (hash, subject, time)) = wt.head_info {
                div()
                    .flex().gap(px(8.0))
                    .text_sm().text_color(rgb(0xa6adc8))
                    .child(format!("{} · {} · {}", hash, subject, time))
            } else {
                div()
            }
        )
        // Row 4: base ahead/behind (if available)
        .when(wt.base_ahead_behind.is_some(), |el| {
            let (ahead, behind) = wt.base_ahead_behind.unwrap();
            if ahead > 0 || behind > 0 {
                el.child(
                    div()
                        .text_sm()
                        .child(format!(
                            "vs base: +{} ahead, -{} behind",
                            ahead, behind
                        ))
                )
            } else {
                el.child(
                    div().text_sm().text_color(rgb(0xa6e3a1))
                        .child("up to date with base")
                )
            }
        })
}

/// Load worktree details asynchronously (runs on background thread via tokio)
async fn load_worktree_details(
    ws_path: &std::path::Path,
    hosts: &indexmap::IndexMap<String, wtp_core::config::HostConfig>,
) -> Vec<WorktreeInfo> {
    let wt_manager = match WorktreeManager::load(ws_path) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    let git = GitClient::new();
    let mut results = Vec::new();

    for entry in wt_manager.list_worktrees() {
        let abs_path = ws_path.join(&entry.worktree_path);
        let repo_abs = entry.repo.to_absolute_path(hosts);

        // These are blocking git CLI calls — acceptable in spawn() context
        let status = git.get_full_status(&abs_path).ok();
        let head_info = git.get_head_info(&abs_path).ok();
        let base_ab = if let Some(ref base) = entry.base {
            git.get_ahead_behind(&abs_path, base).ok().flatten()
        } else {
            None
        };

        results.push(WorktreeInfo {
            entry: entry.clone(),
            abs_path,
            status,
            head_info,
            base_ahead_behind: base_ab,
        });
    }

    results
}
```

**Note on blocking**: `GitClient` methods use `std::process::Command` (blocking). Inside `cx.spawn()`, this runs on GPUI's async executor. For v1, this is acceptable. For v2, consider wrapping in `tokio::task::spawn_blocking()`.

---

### B.7 `views/create_workspace.rs`

**Purpose**: Form for creating a new workspace with name input and hook option.

```rust
//! Create workspace dialog
//!
//! Form with name input and optional hook execution toggle.
//! Uses gpui-component Input for text input.

use gpui::*;
use gpui_component::input::{InputState, InputEvent};
use crate::state::{AppState, ViewState};
use crate::app::MainWindow;

/// Render the create workspace form
///
/// NOTE: Since this is a render function (not a standalone view struct),
/// the InputState entity must be held somewhere persistent. We use a
/// pattern where the input state is created on first render and stored
/// in a static or in AppState. For simplicity in v1, we use a simple
/// text field approach.
pub fn render(
    state: &AppState,
    state_entity: &Entity<AppState>,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let state_entity = state_entity.clone();

    div()
        .flex_col()
        .gap(px(16.0))
        .max_w(px(500.0))
        // Header
        .child(
            div().text_2xl().font_weight(FontWeight::BOLD).child("Create Workspace")
        )
        // Instructions
        .child(
            div().text_color(rgb(0xa6adc8))
                .child("Enter a name for the new workspace. It will be created under the workspace root directory.")
        )
        // Name input field
        // For v1: use a simple clickable div that opens the workspace name prompt
        // For v2: integrate gpui-component InputState properly
        .child(
            div()
                .flex_col()
                .gap(px(4.0))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Workspace Name"))
                .child(
                    div()
                        .w_full()
                        .px(px(12.0)).py(px(8.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x45475a))
                        .border_1()
                        .border_color(rgb(0x585b70))
                        .child("Type workspace name here...")  // Placeholder
                )
        )
        // Create button
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .child({
                    let s = state_entity.clone();
                    div()
                        .px(px(16.0)).py(px(8.0))
                        .rounded(px(6.0))
                        .bg(rgb(0xa6e3a1))  // Green
                        .text_color(rgb(0x1e1e2e))
                        .cursor_pointer()
                        .font_weight(FontWeight::SEMIBOLD)
                        .hover(|style| style.bg(rgb(0x94e2d5)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            // TODO: get name from input field
                            // For now, this is a placeholder
                            let s2 = s.clone();
                            cx.spawn(|_this, mut cx| async move {
                                // let result = manager.create_workspace(&name, true).await;
                                cx.update(|cx| {
                                    s2.update(cx, |state, _| {
                                        state.refresh_workspaces();
                                        state.navigate(ViewState::WorkspaceList);
                                    });
                                }).ok();
                            }).detach();
                        })
                        .child("Create")
                })
                .child({
                    let s = state_entity.clone();
                    div()
                        .px(px(16.0)).py(px(8.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x45475a))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x585b70)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            s.update(cx, |state, _| {
                                state.navigate(ViewState::WorkspaceList);
                            });
                        })
                        .child("Cancel")
                })
        )
}
```

**⚠ Implementation note for Codex**: The text input needs proper `gpui-component` `InputState` integration. The recommended approach:

1. Add an `Entity<InputState>` field to `MainWindow` (or a dedicated `CreateWorkspaceView` struct)
2. Create it with `cx.new(|cx| InputState::new(window, cx).placeholder("my-workspace"))`
3. Render with `gpui_component::input::TextInput::new(cx).state(&self.input_state)`
4. Read value with `self.input_state.read(cx).value()`

This requires refactoring `CreateWorkspace` from a render function to a proper view struct. See section C.4 for guidance.

---

### B.8 `views/import_repo.rs`

**Purpose**: Import a repo into a workspace. Select host, search for repos, choose branch.

```rust
//! Import repository view
//!
//! Multi-step flow:
//! 1. Select host alias (from config)
//! 2. Search/filter repos under that host (using scan_git_repos)
//! 3. Select repo and configure branch
//! 4. Execute import (create worktree)

use gpui::*;
use crate::state::{AppState, ViewState};
use crate::app::MainWindow;
use wtp_core::git::scan_git_repos;

/// Render the import repo view
pub fn render(
    ws_name: &str,
    state: &AppState,
    state_entity: &Entity<AppState>,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let ws_name_owned = ws_name.to_string();
    let state_entity = state_entity.clone();
    let hosts = state.loaded_config.config.hosts.clone();

    div()
        .flex_col()
        .gap(px(16.0))
        // Header
        .child(
            div().flex().items_center().gap(px(12.0))
                .child({
                    let s = state_entity.clone();
                    let name = ws_name_owned.clone();
                    div()
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(0x89b4fa)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            s.update(cx, |state, _| {
                                state.navigate(ViewState::WorkspaceDetail(name.clone()));
                            });
                        })
                        .child("← Back")
                })
                .child(
                    div().text_2xl().font_weight(FontWeight::BOLD)
                        .child(format!("Import to: {}", ws_name_owned))
                )
        )
        // Host selection
        .child(
            div().flex_col().gap(px(8.0))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Select Host"))
                .child(
                    div().flex().gap(px(8.0))
                        .children(
                            hosts.iter().map(|(alias, host_config)| {
                                let alias_owned = alias.clone();
                                let host_root = host_config.root.clone();
                                let s = state_entity.clone();

                                let is_selected = state.scanned_repos.contains_key(alias);

                                div()
                                    .px(px(12.0)).py(px(6.0))
                                    .rounded(px(6.0))
                                    .bg(if is_selected { rgb(0x89b4fa) } else { rgb(0x45475a) })
                                    .text_color(if is_selected { rgb(0x1e1e2e) } else { rgb(0xcdd6f4) })
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x585b70)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let alias = alias_owned.clone();
                                        let root = host_root.clone();
                                        let s2 = s.clone();
                                        // Scan repos asynchronously
                                        cx.spawn(|_this, mut cx| async move {
                                            let repos = scan_git_repos(&root);
                                            cx.update(|cx| {
                                                s2.update(cx, |state, _| {
                                                    state.scanned_repos.insert(alias, repos);
                                                });
                                            }).ok();
                                        }).detach();
                                    })
                                    .child(alias.clone())
                            })
                        )
                )
        )
        // Search filter
        .child(
            div().flex_col().gap(px(4.0))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Search Repos"))
                .child(
                    div()
                        .w_full()
                        .px(px(12.0)).py(px(8.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x45475a))
                        .border_1()
                        .border_color(rgb(0x585b70))
                        .child(
                            if state.import_search_query.is_empty() {
                                "Filter repositories...".to_string()
                            } else {
                                state.import_search_query.clone()
                            }
                        )
                )
        )
        // Repo list (filtered)
        .child(render_repo_list(state, &state_entity, &ws_name_owned, cx))
}

fn render_repo_list(
    state: &AppState,
    state_entity: &Entity<AppState>,
    ws_name: &str,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let query = state.import_search_query.to_lowercase();

    let mut all_repos: Vec<(String, String)> = Vec::new(); // (host_alias, repo_path)
    for (alias, repos) in &state.scanned_repos {
        for repo in repos {
            if query.is_empty() || repo.to_lowercase().contains(&query) {
                all_repos.push((alias.clone(), repo.clone()));
            }
        }
    }

    div()
        .flex_col()
        .gap(px(4.0))
        .max_h(px(400.0))
        .overflow_y_scroll()
        .children(
            all_repos.into_iter().map(|(host, repo_path)| {
                let display = format!("{}:{}", host, repo_path);
                let s = state_entity.clone();
                let ws = ws_name.to_string();

                div()
                    .w_full()
                    .px(px(12.0)).py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x45475a)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        // TODO: Start import flow (branch selection, worktree creation)
                        // For v1, this would trigger a confirmation dialog
                        let _s = s.clone();
                        let _ws = ws.clone();
                    })
                    .child(display)
            })
        )
        .when(all_repos_empty(&state.scanned_repos), |el| {
            el.child(
                div().text_color(rgb(0x6c7086)).py(px(16.0))
                    .child("Select a host to scan for repositories.")
            )
        })
}

fn all_repos_empty(repos: &std::collections::HashMap<String, Vec<String>>) -> bool {
    repos.is_empty() || repos.values().all(|v| v.is_empty())
}
```

---

### B.9 `views/config_panel.rs`

**Purpose**: Display and edit wtp configuration (workspace root, hosts, hooks).

```rust
//! Configuration editor panel
//!
//! Displays current config and allows editing.
//! Saves via LoadedConfig::save().

use gpui::*;
use crate::state::AppState;
use crate::app::MainWindow;

/// Render the config panel
pub fn render(
    state: &AppState,
    state_entity: &Entity<AppState>,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let config = &state.loaded_config.config;

    div()
        .flex_col()
        .gap(px(16.0))
        // Header
        .child(
            div().text_2xl().font_weight(FontWeight::BOLD).child("Configuration")
        )
        // Source file
        .child(
            div().text_sm().text_color(rgb(0xa6adc8))
                .child(
                    format!("Config file: {}",
                        state.loaded_config.source_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "(none, using defaults)".to_string())
                    )
                )
        )
        // Workspace root
        .child(config_field("Workspace Root", &config.workspace_root.display().to_string()))
        // Default host
        .child(config_field(
            "Default Host",
            config.default_host.as_deref().unwrap_or("(none)")
        ))
        // Hosts section
        .child(
            div()
                .flex_col()
                .gap(px(8.0))
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Hosts"))
                .children(
                    config.hosts.iter().map(|(alias, host_config)| {
                        div()
                            .flex().items_center().gap(px(8.0))
                            .px(px(12.0)).py(px(8.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x313244))
                            .child(
                                div().font_weight(FontWeight::SEMIBOLD)
                                    .min_w(px(100.0))
                                    .child(format!("{}:", alias))
                            )
                            .child(
                                div().text_color(rgb(0xa6adc8))
                                    .child(host_config.root.display().to_string())
                            )
                    })
                )
                .when(config.hosts.is_empty(), |el| {
                    el.child(
                        div().text_color(rgb(0x6c7086))
                            .child("No hosts configured. Edit your config file to add hosts.")
                    )
                })
        )
        // Hooks section
        .child(
            div()
                .flex_col()
                .gap(px(8.0))
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Hooks"))
                .child(config_field(
                    "on_create",
                    &config.hooks.on_create
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(not set)".to_string())
                ))
        )
        // Save button (saves config back to disk)
        .child({
            let s = state_entity.clone();
            div()
                .flex().gap(px(8.0))
                .child(
                    div()
                        .px(px(16.0)).py(px(8.0))
                        .rounded(px(6.0))
                        .bg(rgb(0xa6e3a1))
                        .text_color(rgb(0x1e1e2e))
                        .cursor_pointer()
                        .font_weight(FontWeight::SEMIBOLD)
                        .hover(|style| style.bg(rgb(0x94e2d5)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let s2 = s.clone();
                            s2.update(cx, |state, _| {
                                if let Err(e) = state.loaded_config.save() {
                                    state.error_message = Some(format!("Failed to save config: {}", e));
                                }
                            });
                        })
                        .child("Save Config")
                )
        )
}

fn config_field(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex_col()
        .gap(px(2.0))
        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(label.to_string()))
        .child(
            div()
                .px(px(12.0)).py(px(8.0))
                .rounded(px(6.0))
                .bg(rgb(0x313244))
                .child(value.to_string())
        )
}
```

---

### B.10 `components/search_input.rs`

**Purpose**: Reusable search/filter input component.

```rust
//! Search/filter input component
//!
//! Wraps gpui-component's Input with search-specific behavior:
//! debounced filtering, clear button, search icon.
//!
//! IMPLEMENTATION NOTE: For v1, this is a simplified wrapper.
//! Proper implementation requires gpui-component's InputState entity.
//! See section C.4 for the recommended refactoring path.

use gpui::*;

/// Render a search input field (static placeholder for v1)
///
/// For full implementation, this should be a struct with:
/// - `Entity<gpui_component::input::InputState>` for text state
/// - `on_change: Box<dyn Fn(&str, &mut Context<Self>)>` callback
/// - Debounce timer via `cx.spawn()`
pub fn render_search_placeholder(placeholder: &str) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .px(px(12.0)).py(px(8.0))
        .rounded(px(6.0))
        .bg(rgb(0x45475a))
        .border_1()
        .border_color(rgb(0x585b70))
        .child(
            div().text_color(rgb(0x6c7086)).child(placeholder.to_string())
        )
}

/// Full search input implementation (for v2)
///
/// ```rust,ignore
/// pub struct SearchInput {
///     input_state: Entity<gpui_component::input::InputState>,
///     on_change: Box<dyn Fn(&str)>,
///     debounce_ms: u64,
/// }
///
/// impl SearchInput {
///     pub fn new(
///         window: &mut Window,
///         cx: &mut Context<Self>,
///         placeholder: &str,
///         on_change: impl Fn(&str) + 'static,
///     ) -> Self {
///         let input_state = cx.new(|cx|
///             gpui_component::input::InputState::new(window, cx)
///                 .placeholder(placeholder)
///         );
///         Self {
///             input_state,
///             on_change: Box::new(on_change),
///             debounce_ms: 200,
///         }
///     }
/// }
///
/// impl Render for SearchInput {
///     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
///         gpui_component::input::TextInput::new(cx)
///             .state(&self.input_state)
///             .cleanable()
///             .prefix(|_| {
///                 div().child("🔍")  // Or use an SVG icon
///             })
///     }
/// }
/// ```
```

---

### B.11 `components/status_badge.rs`

**Purpose**: Render git status as colored badges using GPUI elements (not terminal escape codes).

```rust
//! Git status badge component
//!
//! Renders git status as colored text/badges using GPUI's element API.
//! This is the GUI equivalent of GitStatusFormat in wtp-cli.

use gpui::*;
use wtp_core::git::GitStatus;

/// Render a git status as a colored badge element
pub fn render(status: &GitStatus) -> impl IntoElement {
    if !status.dirty && status.ahead == 0 && status.behind == 0 {
        return div()
            .flex().items_center().gap(px(4.0))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xa6e3a1))  // Green
                    .child("✓ clean")
            );
    }

    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        // Dirty status badges
        .when(status.staged > 0, |el| {
            el.child(badge("staged", status.staged, rgb(0xf9e2af)))  // Yellow
        })
        .when(status.unstaged > 0, |el| {
            el.child(badge("unstaged", status.unstaged, rgb(0xf9e2af)))
        })
        .when(status.untracked > 0, |el| {
            el.child(badge("untracked", status.untracked, rgb(0xf9e2af)))
        })
        // Ahead/behind badges
        .when(status.ahead > 0, |el| {
            el.child(
                div()
                    .text_sm()
                    .text_color(rgb(0xa6e3a1))  // Green
                    .child(format!("+{}", status.ahead))
            )
        })
        .when(status.behind > 0, |el| {
            el.child(
                div()
                    .text_sm()
                    .text_color(rgb(0xf38ba8))  // Red
                    .child(format!("-{}", status.behind))
            )
        })
}

fn badge(label: &str, count: u32, color: Hsla) -> impl IntoElement {
    div()
        .flex().items_center().gap(px(2.0))
        .px(px(6.0)).py(px(1.0))
        .rounded(px(4.0))
        .bg(color.opacity(0.15))
        .text_sm()
        .text_color(color)
        .child(format!("{} {}", count, label))
}
```

**Note**: The `badge()` helper uses `color.opacity(0.15)` for a subtle background tint. If GPUI's `Hsla` doesn't have `.opacity()`, use a separate `rgba()` with the alpha channel set to `0.15 * 255 ≈ 38` → `rgba(0xf9e2af26)`.

---

### B.12 `views/mod.rs`

Already exists and is correct:

```rust
pub mod config_panel;
pub mod create_workspace;
pub mod import_repo;
pub mod workspace_detail;
pub mod workspace_list;
```

### B.13 `components/mod.rs`

Already exists and is correct:

```rust
pub mod search_input;
pub mod status_badge;
```

---

## C. Technical Key Points

### C.1 GPUI + tray-icon Event Loop Coordination

**Problem**: GPUI owns the main NSApplication run loop on macOS. tray-icon also requires the main thread and an active NSApplication run loop. Can they coexist?

**Answer: Yes**, because:
1. GPUI's `Application::new().run()` starts an NSApplication run loop internally.
2. tray-icon uses `objc2-app-kit` to add a `NSStatusItem` to the system status bar — this hooks into the **same** NSApplication that GPUI creates.
3. Both share the same Objective-C NSApplication instance on the main thread.

**Event bridging**:
- tray-icon provides `MenuEvent::receiver()` (a `std::sync::mpsc::Receiver<MenuEvent>`)
- We poll this receiver inside a `cx.spawn()` async task using `smol::Timer::after(100ms)` for throttling
- Events are dispatched to GPUI's main thread via `cx.update(|cx| { ... })`

**Alternative considered**: Using `TrayIconEvent::set_event_handler()` with a callback. Rejected because:
- The callback runs on whatever thread the event originates from
- We'd still need to bridge to GPUI's main thread
- The polling pattern is more explicit and debuggable

**Verified**: This pattern is used by projects in [awesome-gpui](https://github.com/zed-industries/awesome-gpui) that combine GPUI with tray-icon.

### C.2 Async Operation Pattern

**GPUI's async model**:
- `cx.spawn(|mut cx: AsyncApp| async move { ... })` — spawns a task on GPUI's built-in async executor
- Inside the async block, `cx.update(|cx| { ... })` dispatches work back to the main thread
- `cx.spawn()` returns a `Task<T>` which can be `.detach()`ed (fire-and-forget) or `.await`ed

**Pattern for git operations (blocking I/O)**:

```rust
// In a click handler or action handler:
let state = self.state.clone();
cx.spawn(|_this, mut cx| async move {
    // ---- OFF main thread (async executor) ----
    // GitClient methods use std::process::Command (blocking)
    let git = GitClient::new();
    let status = git.get_full_status(&path);

    // ---- BACK ON main thread ----
    cx.update(|cx| {
        state.update(cx, |state, _cx| {
            // Update state with results
            state.loading = false;
            // cx.notify() is implicit via state.update()
        });
    }).ok();
}).detach();
```

**Pattern for workspace creation (async I/O)**:

```rust
// WorkspaceManager::create_workspace is async (uses tokio for hooks)
cx.spawn(|_this, mut cx| async move {
    let mut manager = WorkspaceManager::new(loaded_config);
    let result = manager.create_workspace(&name, true).await;

    cx.update(|cx| {
        state.update(cx, |state, _cx| {
            match result {
                Ok(create_result) => {
                    state.refresh_workspaces();
                    state.navigate(ViewState::WorkspaceList);
                }
                Err(e) => {
                    state.error_message = Some(format!("Failed: {}", e));
                    state.loading = false;
                }
            }
        });
    }).ok();
}).detach();
```

**Important**: `cx.spawn()` in GPUI does **not** use tokio — it uses GPUI's own executor (backed by `smol`). For `create_workspace` which is `async` and uses `tokio::process::Command`, we need a tokio runtime. Options:

1. **Recommended**: Create a background tokio runtime and use `tokio::runtime::Handle::block_on()` inside the spawn
2. **Alternative**: Add `#[tokio::main]` to main.rs and use `tokio::spawn` alongside GPUI's spawn

For v1, the simplest approach is to initialize a tokio runtime in `AppState`:

```rust
// In state.rs
pub struct AppState {
    // ...
    pub tokio_runtime: tokio::runtime::Runtime,
}

impl AppState {
    pub fn load() -> Self {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime");
        // ...
    }
}
```

Then in async operations:
```rust
cx.spawn(|_this, mut cx| async move {
    // Use the tokio runtime for async operations
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            manager.create_workspace(&name, true).await
        })
    }).await;
    // ...
}).detach();
```

### C.3 Icon Asset Handling

**Tray icon requirements** (macOS):
- Size: 22×22 pixels (standard macOS menu bar icon size)
- Format: PNG with transparency
- Color: Monochrome (white or template image for dark/light mode support)
- Location: `wtp-gui/assets/icon.png`
- Loading: `include_bytes!("../assets/icon.png")` — embedded at compile time

**Creating the icon**:
For v1, create a simple 22×22 PNG manually or programmatically. A minimal approach:
1. Use the `image` crate to generate a simple "W" glyph programmatically at build time
2. Or commit a hand-crafted PNG to `assets/icon.png`

**Template images on macOS**: For proper dark/light mode support, the icon should be marked as a "template image" via `NSImage::setTemplate:YES`. The `tray-icon` crate handles this automatically for monochrome icons on macOS.

### C.4 Navigation & State Flow

**Architecture**: Single `Entity<AppState>` holds all state. Navigation is a `ViewState` enum field.

```
┌─────────────────────────────────────────────┐
│  Application (GPUI)                          │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │  Tray Icon   │  │    MainWindow        │  │
│  │  (tray.rs)   │  │  ┌────────────────┐  │  │
│  │              │  │  │   Sidebar       │  │  │
│  │  Workspaces  │  │  │  - Workspaces   │  │  │
│  │  ──────────  │  │  │  - Config       │  │  │
│  │  Open...     │  │  ├────────────────┤  │  │
│  │  Create...   │  │  │   Content Area  │  │  │
│  │  ──────────  │  │  │  (active view)  │  │  │
│  │  Quit        │  │  │                 │  │  │
│  └──────┬──────┘  │  └────────┬────────┘  │  │
│         │         └──────────│────────────┘  │
│         └────────────────────┘               │
│                    ↕                          │
│         Entity<AppState> (state.rs)           │
│         - loaded_config                       │
│         - workspaces: Vec<WorkspaceInfo>      │
│         - current_view: ViewState             │
│         - current_worktrees: Vec<WorktreeInfo>│
│         - loading, error_message              │
└──────────────────────────────────────────────┘
```

**State update flow**:
1. User clicks sidebar item → `state.update(cx, |s, _| s.navigate(ViewState::X))`
2. `state.update()` automatically notifies observers
3. `MainWindow`'s `cx.observe(&state, ...)` triggers → `cx.notify()`
4. GPUI re-renders `MainWindow::render()` → selects content view based on `state.current_view`

**Refactoring path for v2** (views that need their own state, e.g., text inputs):

Convert render functions to proper GPUI views:

```rust
pub struct CreateWorkspaceView {
    state: Entity<AppState>,
    name_input: Entity<gpui_component::input::InputState>,
    run_hook: bool,
    focus_handle: FocusHandle,
}

impl Render for CreateWorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Use self.name_input for proper text input
    }
}
```

This would require `MainWindow` to hold `Entity<CreateWorkspaceView>` instances and render them as child views rather than inline render functions.

---

## D. ADRs

### ADR-004: Use gpui from git (not crates.io)

**Status**: Accepted

**Context**: GPUI 0.2.2 is on crates.io, but `gpui-component` (needed for Input, List components) tracks Zed's main branch and may require a newer GPUI than what's published.

**Decision**: Pin `gpui` to Zed v0.229.0 commit `7c07887` via git dependency. Pin `gpui-component` to its main branch.

**Consequences**:
- **Easier**: Guaranteed API compatibility between gpui and gpui-component
- **Harder**: Longer initial compile time; git dependency means no crates.io caching; need to re-pin when updating

### ADR-005: Render functions vs standalone view structs

**Status**: Accepted

**Context**: Each content view (WorkspaceList, WorkspaceDetail, etc.) could be either:
- A) A render function returning `impl IntoElement`
- B) A standalone struct implementing `Render`

**Decision**: Use render functions for v1. Views that need persistent state (text inputs) should be refactored to standalone structs in v2.

**Consequences**:
- **Easier**: Simpler code, fewer entities, faster to implement
- **Harder**: Text input requires a workaround (InputState must be held externally); no per-view focus handling
- **Migration path**: Clear — convert `fn render(...)` to `struct FooView` + `impl Render for FooView` when state is needed

### ADR-006: Polling pattern for tray-icon events

**Status**: Accepted

**Context**: tray-icon events arrive on `MenuEvent::receiver()` (an `mpsc::Receiver`). GPUI doesn't expose its event loop proxy.

**Decision**: Poll the receiver at 100ms intervals inside `cx.spawn()`, dispatch via `cx.update()`.

**Consequences**:
- **Easier**: Works with GPUI's opaque event loop; no platform-specific code
- **Harder**: 100ms latency for menu clicks (imperceptible to users); slight CPU overhead from polling
- **Alternative for v2**: If GPUI exposes an event loop callback mechanism, switch to that

### ADR-007: Color scheme — Catppuccin Mocha

**Status**: Accepted

**Context**: Need a consistent color scheme for the GUI. Options: system theme, custom, or established palette.

**Decision**: Use [Catppuccin Mocha](https://github.com/catppuccin/catppuccin) as the default theme.

**Key colors**:
| Role | Hex | Name |
|------|-----|------|
| Background | `0x1e1e2e` | Base |
| Sidebar BG | `0x181825` | Mantle |
| Surface | `0x313244` | Surface0 |
| Hover | `0x45475a` | Surface1 |
| Border | `0x585b70` | Surface2 |
| Text | `0xcdd6f4` | Text |
| Subtext | `0xa6adc8` | Subtext0 |
| Overlay | `0x6c7086` | Overlay0 |
| Blue accent | `0x89b4fa` | Blue |
| Green | `0xa6e3a1` | Green |
| Yellow | `0xf9e2af` | Yellow |
| Red | `0xf38ba8` | Red |
| Teal | `0x94e2d5` | Teal |

**Consequences**:
- **Easier**: Consistent, well-tested dark theme; widely recognized; easy to adapt for light mode later
- **Harder**: Hardcoded colors (should be extracted to constants in v2)

---

## Implementation Priority Order

1. **main.rs** + **state.rs** — Get the app to launch
2. **app.rs** — MainWindow with sidebar
3. **components/status_badge.rs** — Needed by workspace_detail
4. **views/workspace_list.rs** — First working view
5. **views/workspace_detail.rs** — Core feature
6. **tray.rs** — System tray integration
7. **views/config_panel.rs** — Config display
8. **views/create_workspace.rs** — Workspace creation
9. **views/import_repo.rs** — Repo import
10. **components/search_input.rs** — Enhanced search (v2)
11. **assets/icon.png** — Tray icon asset

Each file should compile independently as it's added. The implementation order ensures each new file has its dependencies already in place.
