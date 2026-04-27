//! Configuration summary panel.

use crate::components::layout::h_flex;
use crate::components::primitives::{
    BadgeTone, Button, ButtonVariant, IconName, ListItem, ListItemSize, badge, middle_truncate,
    page_header, page_stack, section_stack, section_title, stat_card,
};
use crate::components::theme;
use crate::state::{AppState, FlashLevel};
use gpui::prelude::*;
use gpui::*;

pub fn render(state: &AppState, state_entity: &Entity<AppState>) -> impl IntoElement {
    let config = &state.loaded_config.config;
    let config_source = state
        .loaded_config
        .source_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "defaults".to_string());

    page_stack()
        .child(page_header(
            "Configuration",
            "Read-only runtime snapshot of config, hosts, and hooks.",
            Button::new("config-reload", "Reload")
                .variant(ButtonVariant::Secondary)
                .icon(IconName::Refresh)
                .on_click({
                    let state = state_entity.clone();
                    move |_event, _window, cx| {
                        state.update(cx, |state, cx| {
                            match state.reload_config() {
                                Ok(Some(warning)) => {
                                    state.set_flash(FlashLevel::Warning, warning);
                                }
                                Ok(None) => {
                                    state.set_flash(
                                        FlashLevel::Success,
                                        "Configuration reloaded from disk.",
                                    );
                                }
                                Err(error) => {
                                    state.set_flash(
                                        FlashLevel::Error,
                                        format!("Failed to reload config: {error}"),
                                    );
                                }
                            }
                            cx.notify();
                        });
                    }
                }),
        ))
        .child(
            h_flex().gap(theme::inline_gap()).children([
                stat_card(
                    "workspace_root",
                    "Workspace Root",
                    &config.workspace_root.display().to_string(),
                    42,
                )
                .into_any_element(),
                stat_card(
                    "default_host",
                    "Default Host",
                    config.default_host.as_deref().unwrap_or("(none)"),
                    42,
                )
                .into_any_element(),
                stat_card("loaded_from", "Loaded From", &config_source, 42).into_any_element(),
            ]),
        )
        .child(
            section_stack(1)
                .child(section_title("Hosts"))
                .when(config.hosts.is_empty(), |el: Div| {
                    el.child(
                        div()
                            .text_size(theme::ui_text_size())
                            .text_color(theme::text_secondary())
                            .child("No hosts configured."),
                    )
                })
                .children(config.hosts.iter().map(|(alias, host)| {
                    ListItem::new(format!("host:{alias}"), alias.clone())
                        .icon(IconName::Folder)
                        .supporting(middle_truncate(&host.root.display().to_string(), 60))
                        .monospace_supporting(true)
                        .actions(if config.default_host.as_deref() == Some(alias.as_str()) {
                            badge("default", BadgeTone::Accent).into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .size(ListItemSize::Standard)
                })),
        )
        .child(
            section_stack(1).child(section_title("Hooks")).child(
                ListItem::new("hook:on_create", "on_create")
                    .icon(IconName::File)
                    .supporting(middle_truncate(
                        &config
                            .hooks
                            .on_create
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "(not configured)".to_string()),
                        60,
                    ))
                    .monospace_supporting(true)
                    .size(ListItemSize::Standard),
            ),
        )
}
