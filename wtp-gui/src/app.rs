//! Main application window.

use crate::components::layout::{LayoutExt, h_flex, v_flex};
use crate::components::primitives::{
    BadgeTone, Icon, IconName, info_card, message_banner, middle_truncate, nav_item,
    sidebar_section_label, status_bar, titlebar_brand_segment, titlebar_context_segment,
};
use crate::components::text_input::TextInput;
use crate::components::theme;
use crate::state::{AppState, FlashLevel, FlashMessage, ViewState};
use crate::views;
use gpui::prelude::*;
use gpui::{InteractiveElement, StatefulInteractiveElement, *};
use indexmap::IndexMap;

actions!(
    wtp_gui,
    [
        NavigateToWorkspaces,
        NavigateToConfig,
        RefreshWorkspaces,
        WorkspaceListUp,
        WorkspaceListDown,
        ConfirmWorkspaceSelection,
    ]
);

pub struct MainWindow {
    state: Entity<AppState>,
    create_name_input: Entity<TextInput>,
    import_search_input: Entity<TextInput>,
    import_branch_input: Entity<TextInput>,
    import_base_input: Entity<TextInput>,
    focus_handle: FocusHandle,
    workspace_list_focus: FocusHandle,
    detail_load_workspace: Option<String>,
    titlebar_should_move: bool,
}

struct DetailLoadRequest {
    workspace_name: String,
    workspace_path: std::path::PathBuf,
    hosts: IndexMap<String, wtp_core::config::HostConfig>,
}

impl MainWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let workspace_list_focus = cx.focus_handle();
        let entity = cx.entity();
        let create_name_input = cx.new(|cx| TextInput::new("workspace-name", cx));
        let import_search_input = cx.new(|cx| TextInput::new("filter repositories", cx));
        let import_branch_input = cx.new(|cx| TextInput::new("workspace branch", cx));
        let import_base_input = cx.new(|cx| TextInput::new("base ref", cx));

        cx.observe_in(&entity, window, |_this, _state, window, _cx| {
            window.refresh();
        })
        .detach();

        cx.observe_in(&state, window, |_this, _state, _window, cx| {
            cx.notify();
        })
        .detach();

        for input in [
            create_name_input.clone(),
            import_search_input.clone(),
            import_branch_input.clone(),
            import_base_input.clone(),
        ] {
            cx.observe_in(&input, window, |_this, _input, _window, cx| {
                cx.notify();
            })
            .detach();
        }

        Self {
            state,
            create_name_input,
            import_search_input,
            import_branch_input,
            import_base_input,
            focus_handle,
            workspace_list_focus,
            detail_load_workspace: None,
            titlebar_should_move: false,
        }
    }

    fn ensure_workspace_details_loaded(
        &mut self,
        detail_request: Option<DetailLoadRequest>,
        cx: &mut Context<Self>,
    ) {
        let Some(detail_request) = detail_request else {
            self.detail_load_workspace = None;
            return;
        };

        if self.detail_load_workspace.as_deref() == Some(detail_request.workspace_name.as_str()) {
            return;
        }

        self.detail_load_workspace = Some(detail_request.workspace_name.clone());

        let workspace_name = detail_request.workspace_name;
        let workspace_path = detail_request.workspace_path;
        let hosts = detail_request.hosts;
        let state_entity = self.state.clone();

        cx.spawn(async move |this, cx| {
            let result =
                views::workspace_detail::load_worktree_details(&workspace_path, &hosts).await;
            let workspace_name_for_state = workspace_name.clone();

            cx.update(|cx| {
                state_entity.update(cx, |state, cx| {
                    if state.current_detail_workspace.as_deref()
                        != Some(workspace_name_for_state.as_str())
                    {
                        return;
                    }

                    match result {
                        Ok(worktrees) => {
                            state.current_worktrees = worktrees;
                            state.loading_worktrees = false;
                        }
                        Err(error) => {
                            state.current_worktrees.clear();
                            state.loading_worktrees = false;
                            state.set_flash(FlashLevel::Error, error);
                        }
                    }
                    cx.notify();
                });
            });

            let workspace_name_for_window = workspace_name.clone();
            this.update(cx, |this, cx| {
                if this.detail_load_workspace.as_deref() == Some(workspace_name_for_window.as_str())
                {
                    this.detail_load_workspace = None;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state_entity = self.state.clone();
        let (flash, current_view, detail_request, workspace_root) = {
            let state = state_entity.read(cx);
            let detail_request = match &state.current_view {
                ViewState::WorkspaceDetail(workspace_name) if state.loading_worktrees => state
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.name == *workspace_name)
                    .map(|workspace| DetailLoadRequest {
                        workspace_name: workspace_name.clone(),
                        workspace_path: workspace.path.clone(),
                        hosts: state.loaded_config.config.hosts.clone(),
                    }),
                _ => None,
            };

            (
                state.flash.clone(),
                state.current_view.clone(),
                detail_request,
                state
                    .loaded_config
                    .config
                    .workspace_root
                    .display()
                    .to_string(),
            )
        };

        self.ensure_workspace_details_loaded(detail_request, cx);
        let show_custom_titlebar = self.should_render_custom_titlebar(window);

        v_flex()
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::handle_navigate_workspaces))
            .on_action(cx.listener(Self::handle_navigate_config))
            .on_action(cx.listener(Self::handle_refresh))
            .on_action(cx.listener(Self::handle_workspace_list_up))
            .on_action(cx.listener(Self::handle_workspace_list_down))
            .on_action(cx.listener(Self::handle_confirm_workspace_selection))
            .size_full()
            .bg(theme::surface_0())
            .font(theme::ui_font())
            .text_size(px(12.0))
            .line_height(relative(1.2))
            .text_color(theme::text_primary())
            .when(show_custom_titlebar, |el| {
                el.child(self.render_titlebar(window, &current_view, cx))
            })
            .child(self.render_shell_body(&state_entity, &current_view, workspace_root, &flash, cx))
            .child(self.render_status_bar(&current_view, cx))
    }
}

impl MainWindow {
    fn render_shell_body(
        &mut self,
        state_entity: &Entity<AppState>,
        current_view: &ViewState,
        workspace_root: String,
        flash: &Option<FlashMessage>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .flex_1()
            .min_h(px(0.0))
            .items_stretch()
            .child(self.render_sidebar(current_view, workspace_root))
            .child(self.render_main_column(state_entity, flash, cx))
    }

    fn render_main_column(
        &mut self,
        state_entity: &Entity<AppState>,
        flash: &Option<FlashMessage>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w(theme::content_min_width())
            .min_h(px(0.0))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .id("app-content-scroll")
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .max_w(theme::content_max_width())
                            .mx_auto()
                            .px(theme::page_padding())
                            .py(theme::page_padding())
                            .gap(theme::page_gap())
                            .child(self.render_flash_banner(flash))
                            .child(self.render_content(state_entity, cx)),
                    ),
            )
    }

    fn should_render_custom_titlebar(&self, window: &Window) -> bool {
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            true
        } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            matches!(window.window_decorations(), Decorations::Client { .. })
        } else {
            false
        }
    }

    fn render_titlebar(
        &mut self,
        window: &mut Window,
        current_view: &ViewState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let title = self.titlebar_title(current_view);
        let left_padding = if cfg!(target_os = "macos") {
            px(84.0)
        } else {
            theme::page_padding()
        };
        let active = window.is_window_active();
        let drag_region = h_flex()
            .flex_1()
            .h_full()
            .window_control_area(WindowControlArea::Drag)
            .when(
                cfg!(any(target_os = "linux", target_os = "freebsd")),
                |el| {
                    el.on_mouse_down_out(cx.listener(|this, _ev, _window, _cx| {
                        this.titlebar_should_move = false;
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _ev, _window, _cx| {
                            this.titlebar_should_move = false;
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _ev, _window, _cx| {
                            this.titlebar_should_move = true;
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, _ev, window, _cx| {
                        if this.titlebar_should_move {
                            this.titlebar_should_move = false;
                            window.start_window_move();
                        }
                    }))
                },
            )
            .when(!cfg!(target_os = "windows"), |el| {
                el.on_mouse_down(MouseButton::Left, |event, window, _cx| {
                    if event.click_count != 2 {
                        return;
                    }

                    if cfg!(target_os = "macos") {
                        window.titlebar_double_click();
                    } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
                        window.zoom_window();
                    }
                })
            })
            .child(titlebar_brand_segment("wtp", left_padding, active))
            .child(titlebar_context_segment(title, active));

        h_flex()
            .h(theme::titlebar_height())
            .border_b_1()
            .border_color(theme::border_soft())
            .child(drag_region)
            .when(!cfg!(target_os = "macos"), |el| {
                el.child(self.render_titlebar_controls(window))
            })
    }

    fn render_titlebar_controls(&self, window: &mut Window) -> impl IntoElement {
        let controls = window.window_controls();

        div()
            .h_full()
            .bg(theme::surface_0())
            .border_l_1()
            .border_color(theme::border_soft())
            .h_flex()
            .child(if controls.minimize {
                self.titlebar_control_button(WindowControlArea::Min, IconName::Minus, false)
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if controls.maximize {
                self.titlebar_control_button(WindowControlArea::Max, IconName::Square, false)
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(self.titlebar_control_button(WindowControlArea::Close, IconName::X, true))
    }

    fn titlebar_control_button(
        &self,
        area: WindowControlArea,
        icon: IconName,
        danger: bool,
    ) -> impl IntoElement {
        let hover_background = if danger {
            theme::danger_subtle()
        } else {
            theme::surface_2()
        };
        let hover_foreground = if danger {
            theme::danger()
        } else {
            theme::text_primary()
        };

        div()
            .h_full()
            .w(px(38.0))
            .h_flex()
            .justify_center()
            .cursor_pointer()
            .text_color(theme::text_secondary())
            .hover(move |style| style.bg(hover_background).text_color(hover_foreground))
            .when(cfg!(target_os = "windows"), |el| {
                el.window_control_area(area)
            })
            .when(
                cfg!(any(target_os = "linux", target_os = "freebsd")),
                |el| {
                    el.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        cx.stop_propagation();
                        match area {
                            WindowControlArea::Min => window.minimize_window(),
                            WindowControlArea::Max => window.zoom_window(),
                            WindowControlArea::Close => window.remove_window(),
                            WindowControlArea::Drag => {}
                        }
                    })
                },
            )
            .child(Icon::new(icon))
    }

    fn titlebar_title(&self, current_view: &ViewState) -> String {
        match current_view {
            ViewState::WorkspaceList => "Workspaces".to_string(),
            ViewState::WorkspaceDetail(name) => {
                format!("Workspace Detail  /  {}", middle_truncate(name, 36))
            }
            ViewState::CreateWorkspace => "Create Workspace".to_string(),
            ViewState::ImportRepo(name) => {
                format!("Import Repository  /  {}", middle_truncate(name, 36))
            }
            ViewState::Config => "Configuration".to_string(),
        }
    }

    fn render_sidebar(&self, current_view: &ViewState, workspace_root: String) -> impl IntoElement {
        let state_entity = self.state.clone();

        v_flex()
            .w(theme::sidebar_width())
            .flex_none()
            .h_full()
            .bg(theme::surface_1())
            .border_r_1()
            .border_color(theme::border())
            .justify_between()
            .child(
                v_flex()
                    .gap(theme::inline_gap())
                    .p(theme::page_padding())
                    .child(sidebar_section_label("NAVIGATION"))
                    .child(nav_item(
                        "sidebar-workspaces",
                        IconName::Folder,
                        "Workspaces",
                        matches!(
                            current_view,
                            ViewState::WorkspaceList
                                | ViewState::WorkspaceDetail(_)
                                | ViewState::CreateWorkspace
                                | ViewState::ImportRepo(_)
                        ),
                        {
                            let state = state_entity.clone();
                            let workspace_list_focus = self.workspace_list_focus.clone();
                            move |_event, window, cx| {
                                state.update(cx, |state, cx| {
                                    state.navigate(ViewState::WorkspaceList);
                                    cx.notify();
                                });
                                workspace_list_focus.focus(window, cx);
                            }
                        },
                    ))
                    .child(nav_item(
                        "sidebar-config",
                        IconName::Settings,
                        "Config",
                        matches!(current_view, ViewState::Config),
                        {
                            let state = state_entity.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.navigate(ViewState::Config);
                                    cx.notify();
                                });
                            }
                        },
                    )),
            )
            .child(div().m(theme::page_padding()).child(info_card(
                "Workspace Root",
                workspace_root,
                "Cmd/Ctrl+R refresh",
                34,
            )))
    }

    fn render_flash_banner(&self, flash: &Option<FlashMessage>) -> AnyElement {
        let Some(flash) = flash else {
            return div().into_any_element();
        };

        let (tone, label) = match flash.level {
            FlashLevel::Success => (BadgeTone::Success, "success"),
            FlashLevel::Warning => (BadgeTone::Warning, "warning"),
            FlashLevel::Error => (BadgeTone::Danger, "error"),
            FlashLevel::Info => (BadgeTone::Info, "info"),
        };

        message_banner(label, tone, flash.message.clone()).into_any_element()
    }

    fn render_content(
        &mut self,
        state_entity: &Entity<AppState>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let create_name = self.create_name_input.read(cx).text_trimmed().to_string();
        let import_search = self
            .import_search_input
            .read(cx)
            .text_trimmed()
            .to_lowercase();
        let import_branch = self.import_branch_input.read(cx).text_trimmed().to_string();
        let import_base = self.import_base_input.read(cx).text_trimmed().to_string();
        let state = state_entity.read(cx);

        match &state.current_view {
            ViewState::WorkspaceList => div()
                .key_context("WorkspaceListNav")
                .track_focus(&self.workspace_list_focus)
                .child(views::workspace_list::render(
                    &state,
                    state_entity,
                    &self.workspace_list_focus,
                ))
                .into_any_element(),
            ViewState::WorkspaceDetail(name) => {
                views::workspace_detail::render(name, &state, state_entity).into_any_element()
            }
            ViewState::CreateWorkspace => views::create_workspace::render(
                &state,
                state_entity,
                &self.create_name_input,
                create_name,
            )
            .into_any_element(),
            ViewState::ImportRepo(workspace_name) => views::import_repo::render(
                workspace_name,
                &state,
                state_entity,
                &self.import_search_input,
                &self.import_branch_input,
                &self.import_base_input,
                import_search,
                import_branch,
                import_base,
            )
            .into_any_element(),
            ViewState::Config => {
                views::config_panel::render(&state, state_entity).into_any_element()
            }
        }
    }

    fn render_status_bar(
        &self,
        current_view: &ViewState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let left = match current_view {
            ViewState::WorkspaceList => "WORKSPACES".to_string(),
            ViewState::WorkspaceDetail(name) => format!("DETAIL {}", name.to_uppercase()),
            ViewState::CreateWorkspace => "CREATE".to_string(),
            ViewState::ImportRepo(name) => format!("IMPORT {}", name.to_uppercase()),
            ViewState::Config => "CONFIG".to_string(),
        };

        let center = if let Some(flash) = &state.flash {
            middle_truncate(&flash.message, 44)
        } else if let Some(workspace) = &state.current_detail_workspace {
            format!("selected={workspace}")
        } else {
            "ready".to_string()
        };

        // Show only the shortcuts that are useful in the current view, so the
        // status bar reads as guidance rather than a key cheat-sheet.
        let mod_key = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        let right = match current_view {
            ViewState::WorkspaceList => {
                format!("↑↓ navigate  Enter open  {mod_key}+R refresh")
            }
            ViewState::WorkspaceDetail(_) => {
                format!("{mod_key}+R refresh  {mod_key}+1 back to list")
            }
            ViewState::CreateWorkspace | ViewState::ImportRepo(_) => {
                format!("{mod_key}+1 cancel")
            }
            ViewState::Config => format!("{mod_key}+1 workspaces"),
        };
        status_bar(left, center, right)
    }

    fn handle_navigate_workspaces(
        &mut self,
        _: &NavigateToWorkspaces,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            state.navigate(ViewState::WorkspaceList);
            cx.notify();
        });
        self.workspace_list_focus.focus(window, cx);
    }

    fn handle_navigate_config(
        &mut self,
        _: &NavigateToConfig,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            state.navigate(ViewState::Config);
            cx.notify();
        });
    }

    fn handle_refresh(
        &mut self,
        _: &RefreshWorkspaces,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            let current_workspace = state.current_detail_workspace.clone();
            state.refresh_workspaces();
            if let Some(workspace_name) = current_workspace {
                state.open_workspace_detail(workspace_name, true);
                state.set_flash(
                    FlashLevel::Info,
                    "Workspace list and detail state refreshed.",
                );
            } else {
                state.set_flash(FlashLevel::Info, "Workspace list refreshed.");
            }
            cx.notify();
        });
    }

    fn handle_workspace_list_up(
        &mut self,
        _: &WorkspaceListUp,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            if matches!(state.current_view, ViewState::WorkspaceList) {
                state.move_workspace_cursor(-1);
                cx.notify();
            }
        });
    }

    fn handle_workspace_list_down(
        &mut self,
        _: &WorkspaceListDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            if matches!(state.current_view, ViewState::WorkspaceList) {
                state.move_workspace_cursor(1);
                cx.notify();
            }
        });
    }

    fn handle_confirm_workspace_selection(
        &mut self,
        _: &ConfirmWorkspaceSelection,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            if matches!(state.current_view, ViewState::WorkspaceList)
                && let Some(workspace_name) = state.selected_workspace_name()
            {
                state.open_workspace_detail(workspace_name, false);
                cx.notify();
            }
        });
    }
}

pub fn open_main_window(cx: &mut App, state: Entity<AppState>) {
    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-1", NavigateToWorkspaces, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-1", NavigateToWorkspaces, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-2", NavigateToConfig, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-2", NavigateToConfig, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-r", RefreshWorkspaces, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-r", RefreshWorkspaces, None),
        KeyBinding::new("k", WorkspaceListUp, Some("WorkspaceListNav")),
        KeyBinding::new("j", WorkspaceListDown, Some("WorkspaceListNav")),
        KeyBinding::new("enter", ConfirmWorkspaceSelection, Some("WorkspaceListNav")),
        KeyBinding::new("up", WorkspaceListUp, Some("WorkspaceListNav")),
        KeyBinding::new("down", WorkspaceListDown, Some("WorkspaceListNav")),
    ]);

    let state_for_window = state.clone();
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let window_decorations = Some(WindowDecorations::Client);
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    let window_decorations = None;
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1120.0), px(760.0)), cx)),
            focus: true,
            show: true,
            window_min_size: Some(size(px(960.0), px(640.0))),
            titlebar: Some(TitlebarOptions {
                title: Some("wtp".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(10.0))),
            }),
            window_decorations,
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| MainWindow::new(state_for_window.clone(), window, cx)),
    );

    match result {
        Ok(window) => {
            window
                .update(cx, |view, window, cx| {
                    cx.activate(true);
                    window.activate_window();
                    view.workspace_list_focus.focus(window, cx);
                    window.refresh();
                })
                .ok();
        }
        Err(error) => {
            eprintln!("Failed to open wtp window: {error}");
            state.update(cx, |state, cx| {
                state.set_flash(
                    FlashLevel::Error,
                    format!("Failed to open the dashboard window: {error}"),
                );
                cx.notify();
            });
        }
    }
}
