use crate::components::layout::{LayoutExt, h_flex, v_flex};
use crate::components::theme;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, IntoElement, ParentElement, RenderOnce, Rgba, SharedString,
    StatefulInteractiveElement, Styled, Svg, TextAlign, Window, div, px, relative, svg,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

impl ButtonSize {
    fn height(self) -> gpui::Pixels {
        match self {
            Self::Small => px(24.0),
            Self::Medium => px(32.0),
            Self::Large => px(40.0),
        }
    }

    fn padding_x(self) -> gpui::Pixels {
        match self {
            Self::Small => px(8.0),
            Self::Medium => px(12.0),
            Self::Large => px(12.0),
        }
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    pub label: SharedString,
    pub variant: ButtonVariant,
    pub size: ButtonSize,
    pub disabled: bool,
    pub icon: Option<IconName>,
    pub on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl Button {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Medium,
            disabled: false,
            icon: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    #[allow(dead_code)]
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, border, text) = if self.disabled {
            (theme::surface_1(), theme::border(), theme::text_tertiary())
        } else {
            match self.variant {
                ButtonVariant::Primary => (theme::accent(), theme::accent(), theme::text_inverse()),
                ButtonVariant::Secondary => {
                    (theme::surface_2(), theme::border(), theme::text_primary())
                }
                // Ghost is purely textual at rest so it composes cleanly next
                // to Primary/Secondary buttons regardless of background. The
                // transparent border keeps its hit target 1px-aligned with
                // peer buttons without drawing a visible edge.
                ButtonVariant::Ghost => (
                    gpui::rgba(0x00000000),
                    gpui::rgba(0x00000000),
                    theme::text_secondary(),
                ),
            }
        };

        let hover_bg = match self.variant {
            ButtonVariant::Primary => theme::accent_hover(),
            ButtonVariant::Secondary => theme::surface_3(),
            // Ghost hover stays restrained — surface_1 is barely visible on
            // surface_0 (the page) but enough to read as "interactive".
            ButtonVariant::Ghost => theme::surface_1(),
        };

        // Lift the border on hover too — bg-only feedback is too subtle on
        // dark themes and reads as "did anything happen?". Primary stays on
        // its accent border (changing it would clash with the bg shift), but
        // Secondary and Ghost get a brighter border.
        let hover_border = match self.variant {
            ButtonVariant::Primary => theme::accent_hover(),
            ButtonVariant::Secondary => theme::accent(),
            ButtonVariant::Ghost => theme::border(),
        };

        let content = h_flex()
            .gap(theme::space_1())
            .when_some(self.icon, |this, icon| {
                this.child(Icon::new(icon).size(IconSize::Medium))
            })
            .child(
                div()
                    .text_size(theme::ui_text_size())
                    .font(theme::ui_font())
                    .line_height(relative(theme::compact_line_height()))
                    .child(self.label),
            );

        let base = div()
            .id(self.id)
            .h(self.size.height())
            .px(self.size.padding_x())
            .rounded(theme::button_radius())
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(text)
            .h_flex()
            .child(content);

        let base = if self.disabled {
            base
        } else {
            base.cursor_pointer().hover(move |style: gpui::StyleRefinement| {
                style.bg(hover_bg).border_color(hover_border)
            })
        };

        match (self.disabled, self.on_click) {
            (false, Some(on_click)) => base.on_click(on_click).into_any_element(),
            _ => base.into_any_element(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    GitBranch,
    Folder,
    File,
    X,
    Minus,
    Square,
    Check,
    Plus,
    Settings,
    Search,
    Refresh,
    ChevronLeft,
    ArrowLeft,
}

impl IconName {
    fn asset_path(self) -> SharedString {
        match self {
            Self::GitBranch => "icons/git-branch.svg".into(),
            Self::Folder => "icons/folder.svg".into(),
            Self::File => "icons/file.svg".into(),
            Self::X => "icons/x.svg".into(),
            Self::Minus => "icons/minus.svg".into(),
            Self::Square => "icons/square.svg".into(),
            Self::Check => "icons/check.svg".into(),
            Self::Plus => "icons/plus.svg".into(),
            Self::Settings => "icons/settings.svg".into(),
            Self::Search => "icons/search.svg".into(),
            Self::Refresh => "icons/refresh.svg".into(),
            Self::ChevronLeft => "icons/chevron-left.svg".into(),
            Self::ArrowLeft => "icons/arrow-left.svg".into(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
    Small,
    Medium,
    Large,
}

impl IconSize {
    fn pixels(self) -> gpui::Pixels {
        match self {
            Self::Small => px(12.0),
            Self::Medium => px(16.0),
            Self::Large => px(20.0),
        }
    }
}

#[derive(IntoElement)]
pub struct Icon {
    name: IconName,
    size: IconSize,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: IconSize::Medium,
        }
    }

    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let icon: Svg = svg().flex_none();
        icon.size(self.size.pixels())
            .text_color(window.text_style().color)
            .path(self.name.asset_path())
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListItemSize {
    Compact,
    Standard,
}

impl ListItemSize {
    fn min_height(self) -> gpui::Pixels {
        match self {
            Self::Compact => px(32.0),
            Self::Standard => px(40.0),
        }
    }
}

#[derive(IntoElement)]
pub struct ListItem {
    id: SharedString,
    leading_icon: Option<IconName>,
    title: SharedString,
    supporting: Option<SharedString>,
    auxiliary: Option<SharedString>,
    actions: Option<AnyElement>,
    selected: bool,
    monospace_supporting: bool,
    size: ListItemSize,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl ListItem {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            leading_icon: None,
            title: title.into(),
            supporting: None,
            auxiliary: None,
            actions: None,
            selected: false,
            monospace_supporting: false,
            size: ListItemSize::Standard,
            on_click: None,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    pub fn supporting(mut self, supporting: impl Into<SharedString>) -> Self {
        self.supporting = Some(supporting.into());
        self
    }

    pub fn auxiliary(mut self, auxiliary: impl Into<SharedString>) -> Self {
        self.auxiliary = Some(auxiliary.into());
        self
    }

    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn monospace_supporting(mut self, monospace_supporting: bool) -> Self {
        self.monospace_supporting = monospace_supporting;
        self
    }

    pub fn size(mut self, size: ListItemSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let selected = self.selected;
        let base = div()
            .id(self.id)
            .w_full()
            .min_h(self.size.min_height())
            .flex()
            .items_stretch()
            .rounded(theme::panel_radius())
            .overflow_hidden()
            .bg(if selected {
                theme::surface_2()
            } else {
                theme::surface_0()
            })
            .child(
                div()
                    .w(px(2.0))
                    .rounded(theme::panel_radius())
                    .bg(if selected {
                        theme::accent()
                    } else {
                        theme::surface_0()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(theme::space_3())
                    .py(theme::space_2())
                    .h_flex()
                    .gap(theme::space_2())
                    .child(
                        self.leading_icon
                            .map(|icon| {
                                div()
                                    .w(px(16.0))
                                    .text_color(if selected {
                                        theme::text_primary()
                                    } else {
                                        theme::text_secondary()
                                    })
                                    .child(Icon::new(icon))
                                    .into_any_element()
                            })
                            .unwrap_or_else(|| div().w(px(16.0)).into_any_element()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .v_flex()
                            .gap(px(2.0))
                            .justify_center()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(theme::ui_text_size())
                                    .font(theme::ui_font())
                                    .line_height(relative(theme::compact_line_height()))
                                    .text_color(theme::text_primary())
                                    .child(self.title),
                            )
                            .when_some(self.supporting, |this, supporting| {
                                this.child(
                                    div()
                                        .truncate()
                                        .text_size(theme::ui_text_size())
                                        .line_height(relative(theme::compact_line_height()))
                                        .font(if self.monospace_supporting {
                                            theme::mono_font()
                                        } else {
                                            theme::ui_font()
                                        })
                                        .text_color(theme::text_tertiary())
                                        .child(supporting),
                                )
                            }),
                    )
                    .when_some(self.auxiliary, |this, auxiliary| {
                        this.child(
                            div()
                                .max_w(px(220.0))
                                .truncate()
                                .text_size(theme::ui_text_size())
                                .line_height(relative(theme::compact_line_height()))
                                .font(theme::mono_font())
                                .text_color(theme::text_tertiary())
                                .child(auxiliary),
                        )
                    })
                    .when_some(self.actions, |this, actions| this.child(actions)),
            );

        let base = if self.on_click.is_some() {
            // Hover bumps to surface_2 — `surface_1` was barely distinguishable
            // from the page background, so users couldn't tell rows were
            // interactive without already knowing.
            base.cursor_pointer()
                .hover(|style: gpui::StyleRefinement| style.bg(theme::surface_2()))
        } else {
            base
        };

        match self.on_click {
            Some(on_click) => base.on_click(on_click).into_any_element(),
            None => base.into_any_element(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

pub fn badge(label: impl Into<SharedString>, tone: BadgeTone) -> impl IntoElement {
    let (foreground, background) = badge_colors(tone);
    let label = label.into();

    h_flex()
        .px(theme::space_2())
        .h(px(20.0))
        .rounded_full()
        .bg(background)
        .text_color(foreground)
        .justify_center()
        .child(
            div()
                .font(theme::ui_font_medium())
                .text_size(theme::status_text_size())
                .line_height(relative(theme::compact_line_height()))
                .child(label),
        )
}

pub fn status_bar(
    left: impl Into<SharedString>,
    center: impl Into<SharedString>,
    right: impl Into<SharedString>,
) -> impl IntoElement {
    div()
        .h(theme::status_bar_height())
        .bg(theme::surface_0())
        .border_t_1()
        .border_color(theme::border_soft())
        .font(theme::mono_font())
        .text_size(theme::status_text_size())
        .line_height(relative(theme::compact_line_height()))
        .text_color(theme::text_tertiary())
        .h_flex()
        .child(status_segment(left.into(), true))
        .child(status_segment(center.into(), true))
        .child(
            div()
                .flex_1()
                .px(theme::space_2())
                .truncate()
                .text_align(TextAlign::Right)
                .child(right.into()),
        )
}

pub fn page_stack() -> gpui::Div {
    v_flex().gap(theme::page_gap())
}

pub fn page_intro(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
) -> impl IntoElement {
    let title = title.into();
    let description = description.into();

    v_flex()
        .gap(theme::heading_gap())
        .child(
            div()
                .text_size(theme::title_text_size())
                .font(theme::ui_font_medium())
                .child(title),
        )
        .child(
            div()
                .text_size(theme::ui_text_size())
                .text_color(theme::text_secondary())
                .child(description),
        )
}

pub fn page_header(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    actions: impl IntoElement,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .gap(theme::inline_gap())
        .child(page_intro(title, description))
        .child(actions)
}

fn empty_state_base(
    title: SharedString,
    description: SharedString,
    action: Option<AnyElement>,
) -> gpui::Div {
    section_stack(1)
        .child(section_intro(title, description))
        .when_some(action, |this, action| this.child(action))
}

pub fn empty_state(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
) -> gpui::Div {
    empty_state_base(title.into(), description.into(), None)
}

pub fn empty_state_with_action(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    action: impl IntoElement,
) -> gpui::Div {
    empty_state_base(
        title.into(),
        description.into(),
        Some(action.into_any_element()),
    )
}

pub fn field_block(
    label: impl Into<SharedString>,
    note: impl Into<SharedString>,
    control: impl IntoElement,
) -> gpui::Div {
    v_flex()
        .gap(theme::section_gap())
        .child(field_header(label, note))
        .child(control)
}

pub fn titlebar_brand_segment(
    label: impl Into<SharedString>,
    left_padding: gpui::Pixels,
    active: bool,
) -> impl IntoElement {
    div()
        .w(theme::sidebar_width())
        .h_full()
        .bg(theme::surface_1())
        .border_r_1()
        .border_color(theme::border())
        .px(left_padding)
        .pr(theme::page_padding())
        .h_flex()
        .gap(theme::inline_gap())
        .child(
            div()
                .text_size(theme::title_text_size())
                .font(theme::ui_font_medium())
                .text_color(if active {
                    theme::text_primary()
                } else {
                    theme::text_secondary()
                })
                .child(label.into()),
        )
}

pub fn titlebar_context_segment(title: impl Into<SharedString>, active: bool) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .bg(theme::surface_0())
        .px(theme::page_padding())
        .h_flex()
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(theme::ui_text_size())
                .font(theme::ui_font_medium())
                .text_color(if active {
                    theme::text_primary()
                } else {
                    theme::text_secondary()
                })
                .child(title.into()),
        )
}

pub fn sidebar_section_label(label: impl Into<SharedString>) -> impl IntoElement {
    div()
        .font(theme::mono_font())
        .text_size(theme::status_text_size())
        .text_color(theme::text_tertiary())
        .child(label.into())
}

pub fn nav_item(
    id: impl Into<SharedString>,
    icon: IconName,
    label: impl Into<SharedString>,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .h(px(40.0))
        .flex()
        .items_stretch()
        .rounded(theme::panel_radius())
        .overflow_hidden()
        .cursor_pointer()
        .hover(|style| style.bg(theme::surface_1()))
        .on_click(on_click)
        .child(
            div()
                .w(px(2.0))
                .rounded(theme::panel_radius())
                .bg(if active {
                    theme::accent()
                } else {
                    theme::surface_1()
                }),
        )
        .child(
            div()
                .flex_1()
                .px(theme::panel_padding())
                .h_flex()
                .gap(theme::inline_gap())
                .bg(if active {
                    theme::surface_2()
                } else {
                    theme::surface_1()
                })
                .child(
                    div()
                        .text_color(if active {
                            theme::text_primary()
                        } else {
                            theme::text_secondary()
                        })
                        .child(Icon::new(icon)),
                )
                .child(
                    div()
                        .font(theme::ui_font())
                        .text_size(theme::ui_text_size())
                        .text_color(if active {
                            theme::text_primary()
                        } else {
                            theme::text_secondary()
                        })
                        .child(label.into()),
                ),
        )
}

pub fn info_card(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    footer: impl Into<SharedString>,
    max_chars: usize,
) -> impl IntoElement {
    let value = value.into();

    section_stack(2)
        .child(
            div()
                .text_size(theme::ui_text_size())
                .font(theme::ui_font_medium())
                .text_color(theme::text_secondary())
                .child(label.into()),
        )
        .child(
            div()
                .font(theme::mono_font())
                .text_size(theme::ui_text_size())
                .text_color(theme::text_primary())
                .child(middle_truncate(value.as_ref(), max_chars)),
        )
        .child(
            div()
                .text_size(theme::ui_text_size())
                .text_color(theme::text_tertiary())
                .child(footer.into()),
        )
}

pub fn section_stack(level: u8) -> gpui::Div {
    panel(level)
        .p(theme::panel_padding())
        .v_flex()
        .gap(theme::section_gap())
}

pub fn section_intro(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
) -> impl IntoElement {
    let title = title.into();
    let description = description.into();

    v_flex()
        .gap(theme::heading_gap())
        .child(
            div()
                .text_size(theme::title_text_size())
                .font(theme::ui_font_medium())
                .child(title),
        )
        .child(
            div()
                .text_size(theme::ui_text_size())
                .text_color(theme::text_secondary())
                .child(description),
        )
}

pub fn section_title(title: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::title_text_size())
        .font(theme::ui_font_medium())
        .child(title.into())
}

pub fn field_header(
    label: impl Into<SharedString>,
    note: impl Into<SharedString>,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .gap(theme::inline_gap())
        .child(
            div()
                .text_size(theme::ui_text_size())
                .font(theme::ui_font_medium())
                .text_color(theme::text_secondary())
                .child(label.into()),
        )
        .child(
            div()
                .font(theme::mono_font())
                .text_size(theme::ui_text_size())
                .text_color(theme::text_tertiary())
                .child(note.into()),
        )
}

pub fn toggle_row(
    id: impl Into<SharedString>,
    enabled: bool,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .rounded(theme::button_radius())
        .border_1()
        .border_color(if enabled {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(if enabled {
            theme::surface_2()
        } else {
            theme::surface_1()
        })
        .cursor_pointer()
        .hover(|style| style.bg(theme::surface_2()))
        .on_click(on_click)
        .child(
            div()
                .h(px(40.0))
                .h_flex()
                .gap(theme::inline_gap())
                .child(div().w(px(2.0)).h_full().bg(if enabled {
                    theme::accent()
                } else {
                    theme::surface_1()
                }))
                .child(
                    div()
                        .w(px(16.0))
                        .text_color(if enabled {
                            theme::accent()
                        } else {
                            theme::text_tertiary()
                        })
                        .child(if enabled {
                            Icon::new(IconName::Check).into_any_element()
                        } else {
                            Icon::new(IconName::X).into_any_element()
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(theme::ui_text_size())
                        .text_color(theme::text_primary())
                        .child(label.into()),
                )
                .child(badge(
                    if enabled { "enabled" } else { "disabled" },
                    if enabled {
                        BadgeTone::Accent
                    } else {
                        BadgeTone::Neutral
                    },
                )),
        )
}

pub fn info_block(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    max_chars: usize,
) -> impl IntoElement {
    let value = value.into();

    v_flex()
        .gap(theme::section_gap())
        .child(
            div()
                .text_size(theme::ui_text_size())
                .font(theme::ui_font_medium())
                .text_color(theme::text_secondary())
                .child(label.into()),
        )
        .child(
            section_stack(2).child(
                div()
                    .font(theme::mono_font())
                    .text_size(theme::ui_text_size())
                    .text_color(theme::text_primary())
                    .child(middle_truncate(value.as_ref(), max_chars)),
            ),
        )
}

pub fn empty_hint(message: impl Into<SharedString>) -> impl IntoElement {
    section_stack(2).child(
        div()
            .text_size(theme::ui_text_size())
            .text_color(theme::text_secondary())
            .child(message.into()),
    )
}

/// Pending/in-flight hint, distinct from `empty_hint` (passive) and
/// `empty_state` (terminal). Use when an async load is in progress so users
/// can tell "we're working on it" apart from "nothing here yet".
pub fn loading_hint(message: impl Into<SharedString>) -> impl IntoElement {
    let message: SharedString = message.into();
    section_stack(2).child(
        div()
            .text_size(theme::ui_text_size())
            .text_color(theme::text_primary())
            .child(format!("… {}", message)),
    )
}

pub fn message_banner(
    label: impl Into<SharedString>,
    tone: BadgeTone,
    message: impl Into<SharedString>,
) -> impl IntoElement {
    let label = label.into();
    let message = message.into();
    let (accent_color, background) = badge_colors(tone);

    div()
        .w_full()
        .rounded(theme::panel_radius())
        .border_1()
        .border_color(accent_color)
        .bg(background)
        .flex()
        .items_stretch()
        .child(div().w(px(2.0)).bg(accent_color))
        .child(
            div()
                .flex_1()
                .px(theme::panel_padding())
                .py(theme::space_2())
                .h_flex()
                .gap(theme::inline_gap())
                .child(badge(label, tone))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(theme::text_primary())
                        .child(message),
                ),
        )
}

pub fn stat_card(id: &'static str, label: &str, value: &str, max_chars: usize) -> impl IntoElement {
    section_stack(1)
        .id(id)
        .flex_1()
        .child(
            div()
                .text_size(theme::ui_text_size())
                .font(theme::ui_font_medium())
                .text_color(theme::text_secondary())
                .child(label.to_string()),
        )
        .child(
            div()
                .font(theme::mono_font())
                .text_size(theme::ui_text_size())
                .text_color(theme::text_primary())
                .child(middle_truncate(value, max_chars)),
        )
}

pub fn key_value_row(
    label: &str,
    value: impl Into<SharedString>,
    max_chars: usize,
) -> impl IntoElement {
    let value = value.into();

    div()
        .flex()
        .gap(theme::inline_gap())
        .child(
            div()
                .w(px(88.0))
                .font(theme::ui_font_medium())
                .text_size(theme::ui_text_size())
                .text_color(theme::text_secondary())
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font(theme::mono_font())
                .text_size(theme::ui_text_size())
                .text_color(theme::text_primary())
                .child(middle_truncate(value.as_ref(), max_chars)),
        )
}

pub fn panel(level: u8) -> gpui::Div {
    let background = match level {
        0 => theme::surface_0(),
        1 => theme::surface_1(),
        2 => theme::surface_2(),
        _ => theme::surface_3(),
    };

    div()
        .rounded(theme::panel_radius())
        .border_1()
        .border_color(theme::border())
        .bg(background)
}

pub fn middle_truncate(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars || max_chars <= 5 {
        return value.to_string();
    }

    let tail_len = max_chars / 2;
    let head_len = max_chars - tail_len - 3;
    let head: String = chars[..head_len].iter().collect();
    let tail: String = chars[chars.len() - tail_len..].iter().collect();
    format!("{head}...{tail}")
}

fn badge_colors(tone: BadgeTone) -> (Rgba, Rgba) {
    match tone {
        BadgeTone::Neutral => (theme::text_secondary(), theme::surface_3()),
        BadgeTone::Accent => (theme::accent(), theme::accent_subtle()),
        BadgeTone::Success => (theme::success(), theme::success_subtle()),
        BadgeTone::Warning => (theme::warning(), theme::warning_subtle()),
        BadgeTone::Danger => (theme::danger(), theme::danger_subtle()),
        BadgeTone::Info => (theme::info(), theme::info_subtle()),
    }
}

fn status_segment(label: SharedString, with_border: bool) -> impl IntoElement {
    div()
        .px(theme::space_2())
        .h_full()
        .h_flex()
        .border_r_1()
        .border_color(if with_border {
            theme::border_soft()
        } else {
            theme::surface_0()
        })
        .child(label)
}
