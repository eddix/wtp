//! Create workspace view.

use std::sync::Arc;

use crate::components::layout::h_flex;
use crate::components::primitives::{
    Button, ButtonVariant, IconName, field_block, info_block, middle_truncate, page_header,
    page_stack, section_intro, section_stack, toggle_row,
};
use crate::components::text_input::TextInput;
use crate::components::theme;
use crate::state::{AppState, FlashLevel, ViewState};
use gpui::prelude::*;
use gpui::*;
use tokio::runtime::{Builder, Runtime};
use wtp_core::{CreateResult, WorkspaceManager, sanitize_workspace_name};

pub fn render(
    state: &AppState,
    state_entity: &Entity<AppState>,
    name_input: &Entity<TextInput>,
    draft_name: String,
) -> impl IntoElement {
    let sanitized_name = sanitize_workspace_name(&draft_name);
    let preview_path = if draft_name.is_empty() {
        state
            .loaded_config
            .config
            .workspace_root
            .join("<workspace-name>")
    } else if sanitized_name.is_empty() {
        // User typed something that sanitizes to nothing (`/`, `...`, etc.)
        state
            .loaded_config
            .config
            .workspace_root
            .join("<invalid-name>")
    } else {
        state
            .loaded_config
            .config
            .workspace_root
            .join(&sanitized_name)
    };
    let hook_path = state
        .loaded_config
        .config
        .hooks
        .on_create
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "(not configured)".to_string());

    page_stack()
        .max_w(px(760.0))
        .child(header_row(state_entity, name_input))
        .child(
            section_stack(1)
                .child(section_intro(
                    "Workspace Details",
                    "Name and target path live together here.",
                ))
                .child(field_block(
                    "Workspace Name",
                    "12px mono input, validated before create",
                    name_input.clone(),
                ))
                .child(info_block(
                    "Target Path",
                    preview_path.display().to_string(),
                    64,
                )),
        )
        .child(
            section_stack(1)
                .child(section_intro(
                    "Hook",
                    "Post-create automation stays isolated from the main form.",
                ))
                .child(field_block(
                    "Post-create Hook",
                    "Create should still succeed if hook fails",
                    toggle_row(
                        "create-hook-toggle",
                        state.create_run_hook,
                        if state.loaded_config.config.hooks.on_create.is_some() {
                            format!("Run on_create: {}", middle_truncate(&hook_path, 56))
                        } else {
                            "No hook configured".to_string()
                        },
                        {
                            let state = state_entity.clone();
                            move |_event, _window, cx| {
                                state.update(cx, |state, cx| {
                                    state.create_run_hook = !state.create_run_hook;
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )),
        )
        .child(
            section_stack(1)
                .child(section_intro(
                    "Actions",
                    "Confirm or cancel without disturbing the form state.",
                ))
                .child(
                    h_flex()
                        .gap(theme::inline_gap())
                        .child(
                            Button::new("create-confirm", "Create Workspace")
                                .variant(ButtonVariant::Primary)
                                .icon(IconName::Plus)
                                .disabled(state.creating_workspace)
                                .on_click({
                                    let state = state_entity.clone();
                                    let name_input = name_input.clone();
                                    move |_event, _window, cx| {
                                        let workspace_name =
                                            name_input.read(cx).text_trimmed().to_string();
                                        if let Err(error) = validate_workspace_name(&workspace_name)
                                        {
                                            state.update(cx, |state, cx| {
                                                state.set_flash(FlashLevel::Error, error);
                                                cx.notify();
                                            });
                                            return;
                                        }

                                        let (run_hook, loaded_config, runtime) = {
                                            let state_snapshot = state.read(cx);
                                            (
                                                state_snapshot.create_run_hook,
                                                state_snapshot.loaded_config.clone(),
                                                state_snapshot.tokio_runtime(),
                                            )
                                        };

                                        state.update(cx, |state, cx| {
                                            state.creating_workspace = true;
                                            state.clear_flash();
                                            cx.notify();
                                        });

                                        let workspace_name_for_task = workspace_name.clone();
                                        let state_for_task = state.clone();
                                        let input_for_task = name_input.clone();

                                        cx.spawn(async move |cx| {
                                            let result = smol::unblock(move || {
                                                create_workspace_blocking(
                                                    loaded_config,
                                                    runtime,
                                                    &workspace_name_for_task,
                                                    run_hook,
                                                )
                                            })
                                            .await;

                                            let workspace_name_for_state = workspace_name.clone();
                                            cx.update(|cx| {
                                                state_for_task.update(cx, |state, cx| {
                                                    state.creating_workspace = false;
                                                    match result {
                                                        Ok(result) => {
                                                            state.refresh_workspaces();
                                                            state.open_workspace_detail(
                                                                workspace_name_for_state.clone(),
                                                                true,
                                                            );
                                                            state.set_flash(
                                                                if result.hook_warning.is_some() {
                                                                    FlashLevel::Warning
                                                                } else {
                                                                    FlashLevel::Success
                                                                },
                                                                format_success_message(
                                                                    &workspace_name_for_state,
                                                                    &result,
                                                                ),
                                                            );
                                                            input_for_task
                                                                .update(cx, |input, cx| {
                                                                    input.clear(cx)
                                                                });
                                                        }
                                                        Err(error) => {
                                                            state.set_flash(
                                                                FlashLevel::Error,
                                                                error,
                                                            );
                                                        }
                                                    }
                                                    cx.notify();
                                                });
                                            });
                                        })
                                        .detach();
                                    }
                                }),
                        )
                        .child(
                            Button::new("create-cancel", "Cancel")
                                .variant(ButtonVariant::Secondary)
                                .on_click({
                                    let state = state_entity.clone();
                                    let name_input = name_input.clone();
                                    move |_event, _window, cx| {
                                        name_input.update(cx, |input, cx| input.clear(cx));
                                        state.update(cx, |state, cx| {
                                            state.navigate(ViewState::WorkspaceList);
                                            cx.notify();
                                        });
                                    }
                                }),
                        ),
                ),
        )
}

fn header_row(state_entity: &Entity<AppState>, name_input: &Entity<TextInput>) -> impl IntoElement {
    page_header(
        "Create Workspace",
        "Set the workspace name. The directory is created under your configured root.",
        Button::new("create-back", "Back")
            .variant(ButtonVariant::Secondary)
            .icon(IconName::ArrowLeft)
            .on_click({
                let state = state_entity.clone();
                let name_input = name_input.clone();
                move |_event, _window, cx| {
                    name_input.update(cx, |input, cx| input.clear(cx));
                    state.update(cx, |state, cx| {
                        state.navigate(ViewState::WorkspaceList);
                        cx.notify();
                    });
                }
            }),
    )
}

fn create_workspace_blocking(
    loaded_config: wtp_core::LoadedConfig,
    runtime: Option<Arc<Runtime>>,
    workspace_name: &str,
    run_hook: bool,
) -> Result<CreateResult, String> {
    let mut manager = WorkspaceManager::new(loaded_config);
    match runtime {
        Some(runtime) => runtime
            .block_on(manager.create_workspace(workspace_name, run_hook))
            .map_err(|error| error.to_string()),
        None => Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(manager.create_workspace(workspace_name, run_hook))
            .map_err(|error| error.to_string()),
    }
}

fn validate_workspace_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Workspace name cannot be empty.".to_string());
    }
    // Path separators and other unsafe characters are silently rewritten to
    // `_` by `wtp_core::sanitize_workspace_name`; only reject names that have
    // no usable characters at all (e.g. "/", "...", "   ").
    let sanitized = sanitize_workspace_name(name);
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err(
            "Workspace name has no usable characters after sanitization.".to_string(),
        );
    }
    Ok(())
}

fn format_success_message(workspace_name: &str, result: &CreateResult) -> String {
    let displayed = if workspace_name == result.effective_name {
        format!("'{}'", result.effective_name)
    } else {
        format!(
            "'{}' (sanitized from '{}')",
            result.effective_name, workspace_name
        )
    };
    if let Some(warning) = &result.hook_warning {
        return format!(
            "Workspace {displayed} created at {}. Hook warning: {warning}",
            result.path.display()
        );
    }
    if result.hook_output.is_some() {
        return format!(
            "Workspace {displayed} created at {}. Hook completed.",
            result.path.display()
        );
    }
    format!(
        "Workspace {displayed} created at {}.",
        result.path.display()
    )
}
