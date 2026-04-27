//! Local design system facade for `wtp-gui`.
//!
//! This module does two jobs:
//! 1. Give the app a single, explicit entry point for the design system.
//! 2. Document which layer should own which decision, so views stop rebuilding
//!    spacing, typography, and section structure ad hoc.
//!
//! The current stack is intentionally small:
//! - `theme.rs`: raw design tokens
//! - `layout.rs`: GPUI layout helpers (`h_flex` / `v_flex`)
//! - `primitives.rs`: reusable controls and page-level building blocks
//!
//! Recommended usage:
//! - App shell and views should prefer the semantic page primitives below.
//! - Reach for raw token functions only when no semantic primitive exists yet.
//! - Add new primitives before copy-pasting a pattern into multiple views.
//!
//! Core composition rules:
//! - Page root: `page_stack()`
//! - Page header: `page_header(...)`
//! - Section container: `section_stack(level)`
//! - Section heading: `section_intro(...)` or `section_title(...)`
//! - Prominent empty state: `empty_state(...)` or `empty_state_with_action(...)`
//! - Label + hint row: `field_header(...)`
//! - Form field group: `field_block(...)`
//! - Read-only code/path block: `info_block(...)`
//! - Empty content hint: `empty_hint(...)` (passive: "nothing here yet")
//! - In-flight load hint: `loading_hint(...)` (active: "we're working on it")
//! - Boolean form row: `toggle_row(...)`
//! - Summary stat tile: `stat_card(...)`
//!
//! This module is a facade only. The actual implementations still live in
//! `theme.rs`, `layout.rs`, and `primitives.rs`.

#![allow(unused_imports)]

pub use crate::components::layout::{LayoutExt, h_flex, v_flex};
pub use crate::components::primitives::{
    BadgeTone, Button, ButtonSize, ButtonVariant, Icon, IconName, IconSize, ListItem, ListItemSize,
    badge, empty_hint, empty_state, empty_state_with_action, field_block, field_header, info_block,
    info_card, key_value_row, loading_hint, message_banner, middle_truncate, nav_item, page_header,
    page_intro, page_stack, panel, section_intro, section_stack, section_title,
    sidebar_section_label, stat_card, status_bar, titlebar_brand_segment, titlebar_context_segment,
    toggle_row,
};
pub use crate::components::theme::{
    accent, accent_hover, accent_subtle, base_unit, border, border_soft, button_radius,
    compact_line_height, content_max_width, content_min_width, danger, danger_subtle, heading_gap,
    info, info_subtle, inline_gap, input_radius, mono_font, page_gap, page_padding, panel_padding,
    panel_radius, section_gap, sidebar_width, space_1, space_2, space_3, status_bar_height,
    status_text_size, success, success_subtle, surface_0, surface_1, surface_2, surface_3,
    text_inverse, text_primary, text_secondary, text_tertiary, title_text_size, titlebar_height,
    ui_font, ui_font_medium, ui_text_size, warning, warning_subtle,
};
