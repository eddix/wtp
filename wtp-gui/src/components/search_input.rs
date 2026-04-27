//! Search/filter input component
//!
//! Text input with filtering. For v1, this is a simplified placeholder.
//! Proper implementation requires gpui-component's InputState entity.

use crate::components::layout::h_flex;
use crate::components::theme;
use gpui::*;

/// Render a search input field (static placeholder for v1)
#[allow(dead_code)]
pub fn render_search_placeholder(placeholder: &str) -> impl IntoElement {
    h_flex()
        .w_full()
        .px(theme::panel_padding())
        .py(theme::space_2())
        .rounded(theme::button_radius())
        .bg(theme::surface_2())
        .border_1()
        .border_color(theme::border())
        .child(
            div()
                .text_size(theme::ui_text_size())
                .text_color(theme::text_tertiary())
                .child(placeholder.to_string()),
        )
}
