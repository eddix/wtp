//! Workspace detail view.

use crate::components::layout::{LayoutExt, h_flex, v_flex};
use crate::components::primitives::{
    BadgeTone, Button, ButtonVariant, IconName, badge, empty_state, key_value_row, loading_hint,
    middle_truncate, page_header, page_stack, section_stack,
};
use crate::components::status_badge;
use crate::components::theme;
use crate::state::{AppState, FlashLevel, ViewState, WorktreeInfo};
use gpui::prelude::*;
use gpui::*;
use wtp_core::{GitClient, WorktreeManager};

pub fn render(
    workspace_name: &str,
    state: &AppState,
    state_entity: &Entity<AppState>,
) -> impl IntoElement {
    let workspace_name_owned = workspace_name.to_string();
    let workspace_path = state
        .workspaces
        .iter()
        .find(|workspace| workspace.name == workspace_name)
        .map(|workspace| workspace.path.display().to_string())
        .unwrap_or_else(|| "(workspace not found)".to_string());

    page_stack()
        .child(page_header(
            workspace_name_owned.clone(),
            middle_truncate(&workspace_path, 72),
            h_flex()
                .gap(theme::inline_gap())
                .child(
                    Button::new("detail-back", "Back")
                        .variant(ButtonVariant::Secondary)
                        .icon(IconName::ArrowLeft)
                        .on_click({
                            let state = state_entity.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.navigate(ViewState::WorkspaceList);
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new("detail-refresh", "Refresh")
                        .variant(ButtonVariant::Secondary)
                        .icon(IconName::Refresh)
                        .on_click({
                            let state = state_entity.clone();
                            let workspace_name = workspace_name_owned.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.open_workspace_detail(workspace_name.clone(), true);
                                    state.set_flash(
                                        FlashLevel::Info,
                                        "Refreshing worktree status...",
                                    );
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new("detail-import", "Import Repo")
                        .variant(ButtonVariant::Primary)
                        .icon(IconName::Plus)
                        .on_click({
                            let state = state_entity.clone();
                            let workspace_name = workspace_name_owned.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.navigate(ViewState::ImportRepo(workspace_name.clone()));
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
                            format!("{} worktrees", state.current_worktrees.len()),
                            BadgeTone::Accent,
                        ))
                        .child(if state.loading_worktrees {
                            badge("loading", BadgeTone::Info).into_any_element()
                        } else {
                            badge("ready", BadgeTone::Neutral).into_any_element()
                        }),
                )
                .child(
                    div()
                        .font(theme::mono_font())
                        .text_size(theme::ui_text_size())
                        .text_color(theme::text_tertiary())
                        .child(".wtp/worktree.toml"),
                ),
        )
        .when(state.loading_worktrees, |el: Div| {
            el.child(loading_hint("Loading worktree status and git metadata"))
        })
        .when(
            !state.loading_worktrees && state.current_worktrees.is_empty(),
            |el: Div| {
                el.child(empty_state(
                    "No worktrees imported",
                    "This workspace exists, but no repository worktrees have been imported yet.",
                ))
            },
        )
        .when(!state.current_worktrees.is_empty(), |el: Div| {
            el.child(
                v_flex()
                    .gap(theme::section_gap())
                    .children(state.current_worktrees.iter().map(render_worktree_card)),
            )
        })
}

fn render_worktree_card(worktree: &WorktreeInfo) -> impl IntoElement {
    let repo_display = worktree.entry.repo.display();
    let branch = worktree.entry.branch.clone();
    let path = worktree.abs_path.display().to_string();
    let stash_count = worktree
        .status
        .as_ref()
        .map(|status| status.stash_count)
        .unwrap_or(0);

    section_stack(1)
        .child(
            h_flex()
                .justify_between()
                .gap(theme::inline_gap())
                .child(
                    h_flex()
                        .gap(theme::inline_gap())
                        .child(div().text_color(theme::text_secondary()).child(
                            crate::components::primitives::Icon::new(IconName::GitBranch),
                        ))
                        .child(
                            div()
                                .text_size(theme::title_text_size())
                                .font(theme::ui_font_medium())
                                .child(repo_display),
                        ),
                )
                .child(badge(&branch, BadgeTone::Accent)),
        )
        .child(
            div()
                .font(theme::mono_font())
                .text_size(theme::ui_text_size())
                .text_color(theme::text_tertiary())
                .child(middle_truncate(&path, 78)),
        )
        .child(match &worktree.status {
            Some(full_status) => h_flex()
                .gap(px(4.0))
                .child(status_badge::render(&full_status.status))
                .when(stash_count > 0, |el: Div| {
                    el.child(badge(format!("stash {}", stash_count), BadgeTone::Warning))
                })
                .into_any_element(),
            None => badge("status unavailable", BadgeTone::Warning).into_any_element(),
        })
        .child(match &worktree.head_info {
            Some((hash, subject, relative_time)) => key_value_row(
                "Latest Commit",
                format!(
                    "{}  {}  {}",
                    middle_truncate(hash, 10),
                    subject,
                    relative_time
                ),
                84,
            )
            .into_any_element(),
            None => key_value_row("Latest Commit", "Unavailable", 84).into_any_element(),
        })
        .child(match worktree.base_ahead_behind {
            Some((ahead, behind)) if ahead == 0 && behind == 0 => {
                key_value_row("Base", "Up to date", 84).into_any_element()
            }
            Some((ahead, behind)) => {
                key_value_row("Base", format!("ahead {} / behind {}", ahead, behind), 84)
                    .into_any_element()
            }
            None => key_value_row("Base", "No comparison", 84).into_any_element(),
        })
}

pub(crate) async fn load_worktree_details(
    workspace_path: &std::path::Path,
    hosts: &indexmap::IndexMap<String, wtp_core::config::HostConfig>,
) -> Result<Vec<WorktreeInfo>, String> {
    let workspace_path = workspace_path.to_path_buf();
    let hosts = hosts.clone();

    smol::unblock(move || {
        let manager = WorktreeManager::load(&workspace_path).map_err(|error| error.to_string())?;
        let git = GitClient::new();
        let mut worktrees = Vec::new();

        for entry in manager.list_worktrees() {
            let absolute_path = workspace_path.join(&entry.worktree_path);
            let _ = entry.repo.to_absolute_path(&hosts);

            let status = git.get_full_status(&absolute_path).ok();
            let head_info = git.get_head_info(&absolute_path).ok();
            let base_ahead_behind = match &entry.base {
                Some(base) => git.get_ahead_behind(&absolute_path, base).ok().flatten(),
                None => None,
            };

            worktrees.push(WorktreeInfo {
                entry: entry.clone(),
                abs_path: absolute_path,
                status,
                head_info,
                base_ahead_behind,
            });
        }

        Ok(worktrees)
    })
    .await
}
