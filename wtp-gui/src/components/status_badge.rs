//! Git status badge component.

use gpui::prelude::FluentBuilder;
use gpui::*;
use wtp_core::git::GitStatus;

use crate::components::layout::h_flex;
use crate::components::primitives::{BadgeTone, badge};
use crate::components::theme;

/// Render a git status summary as a row of compact badges.
pub fn render(status: &GitStatus) -> impl IntoElement {
    let is_clean = !status.dirty && status.ahead == 0 && status.behind == 0;
    if is_clean {
        return badge("clean", BadgeTone::Success).into_any_element();
    }

    h_flex()
        .gap(theme::space_1())
        .when(status.staged > 0, |el: Div| {
            el.child(badge(
                &format!("staged {}", status.staged),
                BadgeTone::Warning,
            ))
        })
        .when(status.unstaged > 0, |el: Div| {
            el.child(badge(
                &format!("unstaged {}", status.unstaged),
                BadgeTone::Warning,
            ))
        })
        .when(status.untracked > 0, |el: Div| {
            el.child(badge(
                &format!("untracked {}", status.untracked),
                BadgeTone::Danger,
            ))
        })
        .when(status.ahead > 0, |el: Div| {
            el.child(badge(
                &format!("ahead {}", status.ahead),
                BadgeTone::Success,
            ))
        })
        .when(status.behind > 0, |el: Div| {
            el.child(badge(
                &format!("behind {}", status.behind),
                BadgeTone::Danger,
            ))
        })
        .into_any_element()
}
