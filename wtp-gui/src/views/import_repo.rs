//! Import repository view.

use crate::components::layout::{h_flex, v_flex};
use crate::components::primitives::{
    BadgeTone, Button, ButtonVariant, IconName, ListItem, ListItemSize, badge, empty_hint,
    empty_state_with_action, field_block, info_block, loading_hint, middle_truncate, page_header,
    page_stack, section_stack, section_title,
};
use crate::components::text_input::TextInput;
use crate::components::theme;
use crate::state::{AppState, FlashLevel, RepoSelection, ViewState};
use gpui::prelude::*;
use gpui::*;
use wtp_core::git::scan_git_repos;
use wtp_core::{GitClient, RepoRef, WorktreeEntry, WorktreeManager};

pub fn render(
    workspace_name: &str,
    state: &AppState,
    state_entity: &Entity<AppState>,
    search_input: &Entity<TextInput>,
    branch_input: &Entity<TextInput>,
    base_input: &Entity<TextInput>,
    search_query: String,
    branch_value: String,
    base_value: String,
) -> impl IntoElement {
    let workspace_name_owned = workspace_name.to_string();
    let selected_host = state.selected_import_host.clone();
    let repos_for_selected_host = selected_host
        .as_ref()
        .and_then(|host| state.scanned_repos.get(host))
        .cloned()
        .unwrap_or_default();
    let filtered_repos: Vec<String> = repos_for_selected_host
        .into_iter()
        .filter(|repo| search_query.is_empty() || repo.to_lowercase().contains(&search_query))
        .collect();
    let selected_repo = state.selected_import_repo.clone();

    page_stack()
        .child(page_header(
            format!("Import into {}", workspace_name_owned),
            "Choose a host, select a repo, then confirm branch and base.",
            Button::new("import-back", "Back")
                .variant(ButtonVariant::Secondary)
                .icon(IconName::ArrowLeft)
                .on_click({
                    let state = state_entity.clone();
                    let search_input = search_input.clone();
                    let branch_input = branch_input.clone();
                    let base_input = base_input.clone();
                    let workspace_name = workspace_name_owned.clone();
                    move |_event, _window, cx| {
                        clear_import_inputs(&search_input, &branch_input, &base_input, cx);
                        state.update(cx, |state, cx| {
                            state.open_workspace_detail(workspace_name.clone(), false);
                            cx.notify();
                        });
                    }
                }),
        ))
        .when(state.loaded_config.config.hosts.is_empty(), |el: Div| {
            el.child(empty_state_with_action(
                "No hosts configured",
                "Add at least one host root in your wtp config first.",
                Button::new("import-open-config", "Open Config")
                    .variant(ButtonVariant::Secondary)
                    .icon(IconName::Settings)
                    .on_click({
                        let state = state_entity.clone();
                        move |_event, _window, cx| {
                            state.update(cx, |state, cx| {
                                state.navigate(ViewState::Config);
                                cx.notify();
                            });
                        }
                    }),
            ))
        })
        .when(!state.loaded_config.config.hosts.is_empty(), |el: Div| {
            el.child(
                page_stack()
                    .child(render_host_picker(
                        state,
                        state_entity,
                        &workspace_name_owned,
                        search_input,
                    ))
                    .child(render_repo_picker(
                        state,
                        state_entity,
                        search_input,
                        &filtered_repos,
                    ))
                    .child(render_import_form(
                        state,
                        state_entity,
                        &workspace_name_owned,
                        selected_repo,
                        branch_value,
                        base_value,
                        branch_input,
                        base_input,
                        search_input,
                    )),
            )
        })
}

fn render_host_picker(
    state: &AppState,
    state_entity: &Entity<AppState>,
    workspace_name: &str,
    search_input: &Entity<TextInput>,
) -> impl IntoElement {
    section_stack(1).child(section_title("Hosts")).children(
        state
            .loaded_config
            .config
            .hosts
            .iter()
            .map(|(alias, host)| {
                let alias_owned = alias.clone();
                let host_root = host.root.clone();
                let is_selected = state.selected_import_host.as_deref() == Some(alias.as_str());
                let is_scanning = state.scanning_host.as_deref() == Some(alias.as_str());
                let repo_count = state
                    .scanned_repos
                    .get(alias)
                    .map(|repos| repos.len())
                    .unwrap_or(0);
                let state = state_entity.clone();
                let search_input = search_input.clone();
                let workspace_name = workspace_name.to_string();

                ListItem::new(
                    format!("import-host:{workspace_name}:{alias}"),
                    alias.clone(),
                )
                .icon(IconName::Folder)
                .supporting(middle_truncate(&host.root.display().to_string(), 68))
                .monospace_supporting(true)
                .selected(is_selected)
                .actions(badge(
                    if is_scanning {
                        "scanning".to_string()
                    } else if repo_count > 0 {
                        format!("{} repos", repo_count)
                    } else {
                        "scan".to_string()
                    },
                    if is_scanning {
                        BadgeTone::Info
                    } else {
                        BadgeTone::Neutral
                    },
                ))
                .size(ListItemSize::Standard)
                .on_click(move |_event, _window, cx| {
                    let should_scan = state.read(cx).scanned_repos.get(&alias_owned).is_none();

                    state.update(cx, |state, cx| {
                        state.select_import_host(alias_owned.clone());
                        if should_scan {
                            state.scanning_host = Some(alias_owned.clone());
                        }
                        cx.notify();
                    });

                    if !should_scan {
                        return;
                    }

                    search_input.update(cx, |input, cx| input.clear(cx));
                    let state_for_task = state.clone();
                    let alias_for_task = alias_owned.clone();
                    let host_root_for_task = host_root.clone();
                    cx.spawn(async move |cx| {
                        let repos =
                            smol::unblock(move || scan_git_repos(&host_root_for_task)).await;
                        cx.update(|cx| {
                            state_for_task.update(cx, |state, cx| {
                                state
                                    .scanned_repos
                                    .insert(alias_for_task.clone(), repos.clone());
                                if state.scanning_host.as_deref() == Some(alias_for_task.as_str()) {
                                    state.scanning_host = None;
                                }
                                state.select_import_host(alias_for_task.clone());
                                if repos.is_empty() {
                                    state.set_flash(
                                        FlashLevel::Warning,
                                        format!(
                                            "No repositories were found under host '{}'.",
                                            alias_for_task
                                        ),
                                    );
                                }
                                cx.notify();
                            });
                        });
                    })
                    .detach();
                })
            }),
    )
}

fn render_repo_picker(
    state: &AppState,
    state_entity: &Entity<AppState>,
    search_input: &Entity<TextInput>,
    filtered_repos: &[String],
) -> impl IntoElement {
    let selected_host = state.selected_import_host.clone();

    section_stack(1)
        .child(section_title("Repositories"))
        .child(search_input.clone())
        .child(
            v_flex()
                .id("import-repo-list")
                .max_h(theme::scroll_list_max_height())
                .overflow_y_scroll()
                .when(selected_host.is_none(), |el| {
                    el.child(empty_hint("Select a host first."))
                })
                .when(
                    selected_host.is_some() && state.scanning_host.is_some(),
                    |el| el.child(loading_hint("Scanning repositories")),
                )
                .when(
                    selected_host.is_some()
                        && state.scanning_host.is_none()
                        && filtered_repos.is_empty(),
                    |el| el.child(empty_hint("No repositories match the current filter.")),
                )
                .children(filtered_repos.iter().map(|repo_path| {
                    let host = selected_host.clone().unwrap_or_default();
                    let repo_path_owned = repo_path.clone();
                    let is_selected = state.selected_import_repo.as_ref().is_some_and(|selected| {
                        selected.host == host && selected.repo_path == repo_path_owned
                    });
                    let state = state_entity.clone();
                    let host_label = host.clone();
                    let repo_label = repo_path_owned.clone();

                    ListItem::new(
                        format!("repo:{host}:{repo_path_owned}"),
                        repo_path_owned.clone(),
                    )
                    .icon(IconName::GitBranch)
                    .supporting(middle_truncate(&repo_label, 70))
                    .monospace_supporting(true)
                    .selected(is_selected)
                    .actions(badge(&host_label, BadgeTone::Accent))
                    .size(ListItemSize::Standard)
                    .on_click(move |_event, _window, cx| {
                        let repo_selection = RepoSelection {
                            host: host.clone(),
                            repo_path: repo_path_owned.clone(),
                        };
                        let repo_root = state
                            .read(cx)
                            .loaded_config
                            .config
                            .hosts
                            .get(&repo_selection.host)
                            .map(|host| host.root.join(&repo_selection.repo_path));

                        state.update(cx, |state, cx| {
                            state.select_import_repo(
                                repo_selection.host.clone(),
                                repo_selection.repo_path.clone(),
                            );
                            cx.notify();
                        });

                        let Some(repo_root) = repo_root else {
                            state.update(cx, |state, cx| {
                                state.resolving_import_base = false;
                                state.set_flash(
                                    FlashLevel::Error,
                                    "Selected host no longer exists in the loaded config.",
                                );
                                cx.notify();
                            });
                            return;
                        };

                        let state_for_task = state.clone();
                        let selection_for_task = repo_selection.clone();
                        cx.spawn(async move |cx| {
                            let detected_base =
                                smol::unblock(move || detect_base_ref(&repo_root)).await;
                            cx.update(|cx| {
                                state_for_task.update(cx, |state, cx| {
                                    if state.selected_import_repo.as_ref()
                                        != Some(&selection_for_task)
                                    {
                                        return;
                                    }
                                    state.import_base_hint =
                                        Some(detected_base.unwrap_or_else(|| "HEAD".to_string()));
                                    state.resolving_import_base = false;
                                    cx.notify();
                                });
                            });
                        })
                        .detach();
                    })
                })),
        )
}

#[allow(clippy::too_many_arguments)]
fn render_import_form(
    state: &AppState,
    state_entity: &Entity<AppState>,
    workspace_name: &str,
    selected_repo: Option<RepoSelection>,
    branch_value: String,
    base_value: String,
    branch_input: &Entity<TextInput>,
    base_input: &Entity<TextInput>,
    search_input: &Entity<TextInput>,
) -> impl IntoElement {
    let detected_base = state.import_base_hint.clone();
    let effective_branch = if branch_value.is_empty() {
        workspace_name.to_string()
    } else {
        branch_value.clone()
    };
    let effective_base = if base_value.is_empty() {
        detected_base.clone().unwrap_or_else(|| "HEAD".to_string())
    } else {
        base_value.clone()
    };

    page_stack()
        .child(
            section_stack(1)
                .child(section_title("Selected Repo"))
                .child(match selected_repo.clone() {
                    Some(repo) => {
                        info_block("Selected Repo", repo.display().to_string(), 84).into_any_element()
                    }
                    None => empty_hint("Pick a repository before importing.").into_any_element(),
                }),
        )
        .child(
            section_stack(1)
                .child(section_title("Branch and Base"))
                .child(field_block(
                    "Branch",
                    "Defaults to workspace name",
                    branch_input.clone(),
                ))
                .child(field_block(
                    "Base Ref",
                    "Leave blank to use detected base",
                    base_input.clone(),
                ))
                .when(state.resolving_import_base, |el: Div| {
                    el.child(badge("detecting base", BadgeTone::Info))
                })
                .when(!state.resolving_import_base && detected_base.is_some(), |el: Div| {
                    el.child(badge(
                        format!(
                            "detected {}",
                            detected_base.unwrap_or_else(|| "HEAD".to_string())
                        ),
                        BadgeTone::Neutral,
                    ))
                }),
        )
        .child(
            section_stack(1)
                .child(section_title("Import Plan"))
                .child(info_block(
                    "Resolved Plan",
                    format!("{effective_branch} <- {effective_base}"),
                    84,
                )),
        )
        .child(
            section_stack(1)
                .child(section_title("Actions"))
                .child(
                    h_flex()
                        .gap(theme::inline_gap())
                        .child(
                            Button::new("import-confirm", "Import Repository")
                                .variant(ButtonVariant::Primary)
                                .icon(IconName::Plus)
                                .disabled(state.importing_repo)
                                .on_click({
                                    let state = state_entity.clone();
                                    let branch_input = branch_input.clone();
                                    let base_input = base_input.clone();
                                    let search_input = search_input.clone();
                                    let workspace_name = workspace_name.to_string();
                                    move |_event, _window, cx| {
                                        let Some(selection) = state.read(cx).selected_import_repo.clone() else {
                                            state.update(cx, |state, cx| {
                                                state.set_flash(
                                                    FlashLevel::Error,
                                                    "Choose a repository before starting import.",
                                                );
                                                cx.notify();
                                            });
                                            return;
                                        };

                                        let branch = branch_input.read(cx).text_trimmed().to_string();
                                        let base = base_input.read(cx).text_trimmed().to_string();
                                        let (loaded_config, detected_base) = {
                                            let state = state.read(cx);
                                            (state.loaded_config.clone(), state.import_base_hint.clone())
                                        };

                                        state.update(cx, |state, cx| {
                                            state.importing_repo = true;
                                            state.clear_flash();
                                            cx.notify();
                                        });

                                        let state_for_task = state.clone();
                                        let branch_input_for_task = branch_input.clone();
                                        let base_input_for_task = base_input.clone();
                                        let search_input_for_task = search_input.clone();
                                        let workspace_name_for_task = workspace_name.clone();

                                        cx.spawn(async move |cx| {
                                            let workspace_name_for_blocking =
                                                workspace_name_for_task.clone();
                                            let result = smol::unblock(move || {
                                                import_repository_blocking(
                                                    loaded_config,
                                                    &workspace_name_for_blocking,
                                                    &selection,
                                                    &branch,
                                                    &base,
                                                    detected_base.as_deref(),
                                                )
                                            })
                                            .await;

                                            cx.update(|cx| {
                                                state_for_task.update(cx, |state, cx| {
                                                    state.importing_repo = false;
                                                    match result {
                                                        Ok(entry) => {
                                                            branch_input_for_task.update(
                                                                cx,
                                                                |input, cx| input.clear(cx),
                                                            );
                                                            base_input_for_task.update(
                                                                cx,
                                                                |input, cx| input.clear(cx),
                                                            );
                                                            search_input_for_task.update(
                                                                cx,
                                                                |input, cx| input.clear(cx),
                                                            );
                                                            state.open_workspace_detail(
                                                                workspace_name_for_task.clone(),
                                                                true,
                                                            );
                                                            state.set_flash(
                                                                FlashLevel::Success,
                                                                format!(
                                                                    "Imported '{}' into '{}' on branch '{}'.",
                                                                    entry.repo.display(),
                                                                    workspace_name_for_task,
                                                                    entry.branch
                                                                ),
                                                            );
                                                        }
                                                        Err(error) => {
                                                            state.set_flash(FlashLevel::Error, error);
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
                            Button::new("import-cancel", "Cancel")
                                .variant(ButtonVariant::Secondary)
                                .on_click({
                                    let state = state_entity.clone();
                                    let branch_input = branch_input.clone();
                                    let base_input = base_input.clone();
                                    let search_input = search_input.clone();
                                    let workspace_name = workspace_name.to_string();
                                    move |_event, _window, cx| {
                                        clear_import_inputs(
                                            &search_input,
                                            &branch_input,
                                            &base_input,
                                            cx,
                                        );
                                        state.update(cx, |state, cx| {
                                            state.open_workspace_detail(workspace_name.clone(), false);
                                            cx.notify();
                                        });
                                    }
                                }),
                        ),
                ),
        )
}

fn import_repository_blocking(
    loaded_config: wtp_core::LoadedConfig,
    workspace_name: &str,
    selection: &RepoSelection,
    branch_input: &str,
    base_input: &str,
    detected_base: Option<&str>,
) -> Result<WorktreeEntry, String> {
    let git = GitClient::new();
    git.check_git().map_err(|error| error.to_string())?;

    let manager = wtp_core::WorkspaceManager::new(loaded_config.clone());
    let workspace_path = manager
        .global_config()
        .get_workspace_path(workspace_name)
        .ok_or_else(|| format!("Workspace '{}' was not found on disk.", workspace_name))?;

    let repo_ref = RepoRef::Hosted {
        host: selection.host.clone(),
        path: selection.repo_path.clone(),
    };
    let repo_path = repo_ref.to_absolute_path(&loaded_config.config.hosts);
    if !git.is_in_git_repo(&repo_path) {
        return Err(format!(
            "'{}' is not a git repository.",
            repo_path.display()
        ));
    }

    let repo_root = git
        .get_repo_root(Some(&repo_path))
        .map_err(|error| error.to_string())?;
    let branch = if branch_input.trim().is_empty() {
        workspace_name.to_string()
    } else {
        branch_input.trim().to_string()
    };
    let base = if base_input.trim().is_empty() {
        detected_base.unwrap_or("HEAD").to_string()
    } else {
        base_input.trim().to_string()
    };

    let mut worktree_manager =
        WorktreeManager::load(&workspace_path).map_err(|error| error.to_string())?;
    if let Some(existing) = worktree_manager.config().find_by_repo(&repo_ref) {
        return Err(format!(
            "Repository '{}' is already imported into this workspace as branch '{}'.",
            existing.repo.display(),
            existing.branch
        ));
    }

    let repo_slug = repo_ref.slug();
    let worktree_path = workspace_path.join(worktree_manager.generate_worktree_path(&repo_slug));
    if worktree_path.exists() {
        return Err(format!(
            "Target worktree path already exists: {}",
            worktree_path.display()
        ));
    }

    let branch_exists = git
        .branch_exists(&repo_root, &branch)
        .map_err(|error| error.to_string())?;
    if branch_exists {
        git.add_worktree_for_branch(&repo_root, &worktree_path, &branch)
            .map_err(|error| error.to_string())?;
    } else {
        git.create_worktree_with_branch(&repo_root, &worktree_path, &branch, &base)
            .map_err(|error| error.to_string())?;
    }

    let head_commit = git.get_head_commit_full(&worktree_path).ok();
    let entry = WorktreeEntry::new(
        repo_ref,
        branch,
        worktree_manager.generate_worktree_path(&repo_slug),
        Some(base),
        head_commit,
    );

    worktree_manager
        .add_worktree(entry.clone())
        .map_err(|error| error.to_string())?;

    Ok(entry)
}

fn detect_base_ref(repo_root: &std::path::Path) -> Option<String> {
    GitClient::new()
        .get_current_branch(repo_root)
        .ok()
        .filter(|branch| !branch.trim().is_empty() && branch != "HEAD")
}

fn clear_import_inputs(
    search_input: &Entity<TextInput>,
    branch_input: &Entity<TextInput>,
    base_input: &Entity<TextInput>,
    cx: &mut App,
) {
    search_input.update(cx, |input, cx| input.clear(cx));
    branch_input.update(cx, |input, cx| input.clear(cx));
    base_input.update(cx, |input, cx| input.clear(cx));
}
