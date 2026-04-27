//! Small GPUI layout helpers used across the app.
//!
//! GPUI's `flex_col()` only changes direction. It does not opt the element into
//! flex layout, so these helpers keep `display:flex` and axis setup together.

use gpui::{Div, Styled, div};

/// Extension methods for common flex layouts.
pub trait LayoutExt: Styled + Sized {
    /// Horizontal flex row with centered cross-axis alignment.
    fn h_flex(self) -> Self {
        self.flex().flex_row().items_center()
    }

    /// Vertical flex column.
    fn v_flex(self) -> Self {
        self.flex().flex_col()
    }
}

impl<T: Styled + Sized> LayoutExt for T {}

/// Create a horizontal flex row with centered cross-axis alignment.
#[track_caller]
pub fn h_flex() -> Div {
    div().h_flex()
}

/// Create a vertical flex column.
#[track_caller]
pub fn v_flex() -> Div {
    div().v_flex()
}
