//! Workspace list panel.

use crate::components::layout::{LayoutExt, h_flex};
use crate::components::primitives::{
    BadgeTone, Button, ButtonVariant, IconName, ListItem, ListItemSize, badge,
    empty_state_with_action, middle_truncate, page_header, page_stack, section_stack,
};
use crate::components::theme;
use crate::state::{AppState, FlashLevel, ViewState};
use gpui::prelude::*;
use gpui::{FocusHandle, *};

pub fn render(
    state: &AppState,
    state_entity: &Entity<AppState>,
    list_focus: &FocusHandle,
) -> impl IntoElement {
    let workspace_root = state
        .loaded_config
        .config
        .workspace_root
        .display()
        .to_string();

    page_stack()
        .child(page_header(
            "Workspaces",
            "j/k to move, Enter to open",
            h_flex()
                .gap(theme::inline_gap())
                .child(
                    Button::new("workspace-refresh", "Refresh")
                        .variant(ButtonVariant::Secondary)
                        .icon(IconName::Refresh)
                        .on_click({
                            let state = state_entity.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.refresh_workspaces();
                                    state.set_flash(
                                        FlashLevel::Info,
                                        "Workspace list refreshed from disk.",
                                    );
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new("workspace-create", "New Workspace")
                        .variant(ButtonVariant::Primary)
                        .icon(IconName::Plus)
                        .on_click({
                            let state = state_entity.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.navigate(ViewState::CreateWorkspace);
                                    cx.notify();
                                });
                            }
                        }),
                ),
        ))
        .child(
            section_stack(1)
                .h_flex()
                .justify_between()
                .gap(theme::inline_gap())
                .child(
                    h_flex()
                        .gap(theme::inline_gap())
                        .child(badge(
                            format!("{} workspaces", state.workspaces.len()),
                            BadgeTone::Accent,
                        ))
                        .child(badge(
                            format!("{} hosts", state.loaded_config.config.hosts.len()),
                            BadgeTone::Neutral,
                        )),
                )
                .child(
                    div()
                        .font(theme::mono_font())
                        .text_size(theme::ui_text_size())
                        .text_color(theme::text_tertiary())
                        .child(middle_truncate(&workspace_root, 48)),
                ),
        )
        .when(state.workspaces.is_empty(), |el: Div| {
            el.child(empty_state_with_action(
                "No workspaces",
                "Create one first, then import repositories into it.",
                Button::new("workspace-create-empty", "Create Workspace")
                    .variant(ButtonVariant::Primary)
                    .icon(IconName::Plus)
                    .on_click({
                        let state = state_entity.clone();
                        move |_event, _window, cx| {
                            state.update(cx, |state, cx| {
                                state.navigate(ViewState::CreateWorkspace);
                                cx.notify();
                            });
                        }
                    }),
            ))
        })
        .when(!state.workspaces.is_empty(), |el: Div| {
            el.child(
                section_stack(1).children(state.workspaces.iter().enumerate().map(
                    |(index, workspace)| {
                        let workspace_name = workspace.name.clone();
                        let is_selected = index == state.workspace_list_cursor;
                        let state = state_entity.clone();
                        let list_focus = list_focus.clone();

                        ListItem::new(
                            format!("workspace-row:{}", workspace.name),
                            workspace.name.clone(),
                        )
                        .icon(IconName::Folder)
                        .supporting(middle_truncate(&workspace.path.display().to_string(), 56))
                        .monospace_supporting(true)
                        .auxiliary(".wtp")
                        .selected(is_selected)
                        .size(ListItemSize::Standard)
                        .on_click(move |_event, window, cx| {
                            state.update(cx, |state, cx| {
                                state.open_workspace_detail(workspace_name.clone(), false);
                                cx.notify();
                            });
                            list_focus.focus(window, cx);
                        })
                    },
                )),
            )
        })
}
