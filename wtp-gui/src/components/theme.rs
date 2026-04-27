use gpui::{Font, FontFallbacks, FontWeight, Pixels, Rgba, font, px, rgb, rgba};

pub fn base_unit() -> Pixels {
    px(4.0)
}

pub fn surface_0() -> Rgba {
    rgb(0x0f1218)
}

pub fn surface_1() -> Rgba {
    rgb(0x151a22)
}

pub fn surface_2() -> Rgba {
    rgb(0x1c2330)
}

pub fn surface_3() -> Rgba {
    rgb(0x252f3f)
}

pub fn border() -> Rgba {
    rgb(0x313c4f)
}

pub fn border_soft() -> Rgba {
    rgb(0x242d3c)
}

pub fn text_primary() -> Rgba {
    rgb(0xd8dfeb)
}

pub fn text_secondary() -> Rgba {
    rgb(0xa0acbf)
}

pub fn text_tertiary() -> Rgba {
    rgb(0x6f7b8e)
}

pub fn text_inverse() -> Rgba {
    rgb(0x0f1218)
}

pub fn accent() -> Rgba {
    rgb(0x4d9eff)
}

pub fn accent_hover() -> Rgba {
    rgb(0x69adff)
}

pub fn accent_subtle() -> Rgba {
    rgba(0x224364ff)
}

pub fn success() -> Rgba {
    rgb(0x53c28b)
}

pub fn success_subtle() -> Rgba {
    rgba(0x1f3d30ff)
}

pub fn warning() -> Rgba {
    // Muted amber, distinct from accent_hover. Tuned for legibility on
    // surface_0/1 backgrounds without the harsh saturation of pure yellow.
    rgb(0xe5b454)
}

pub fn warning_subtle() -> Rgba {
    // ~10% L amber wash matching the lightness of other `_subtle` tints.
    rgba(0x4a3a1cff)
}

pub fn danger() -> Rgba {
    rgb(0xe07d86)
}

pub fn danger_subtle() -> Rgba {
    rgba(0x4a2730ff)
}

pub fn info() -> Rgba {
    rgb(0x78b6ff)
}

pub fn info_subtle() -> Rgba {
    rgba(0x253953ff)
}

pub fn space_1() -> Pixels {
    base_unit()
}

pub fn space_2() -> Pixels {
    px(8.0)
}

pub fn space_3() -> Pixels {
    px(12.0)
}

pub fn compact_line_height() -> f32 {
    1.2
}

pub fn ui_text_size() -> Pixels {
    px(12.0)
}

pub fn title_text_size() -> Pixels {
    px(14.0)
}

pub fn status_text_size() -> Pixels {
    px(11.0)
}

pub fn button_radius() -> Pixels {
    px(4.0)
}

pub fn panel_radius() -> Pixels {
    px(4.0)
}

pub fn input_radius() -> Pixels {
    px(2.0)
}

pub fn status_bar_height() -> Pixels {
    px(24.0)
}

pub fn titlebar_height() -> Pixels {
    px(34.0)
}

pub fn sidebar_width() -> Pixels {
    px(240.0)
}

pub fn content_min_width() -> Pixels {
    px(400.0)
}

pub fn content_max_width() -> Pixels {
    px(1120.0)
}

/// Max height for scroll-clipped lists embedded in a page (e.g. repo picker).
/// Tuned to fit ~10 rows at the default `ui_text_size` density.
pub fn scroll_list_max_height() -> Pixels {
    px(320.0)
}

pub fn inline_gap() -> Pixels {
    space_2()
}

pub fn section_gap() -> Pixels {
    space_2()
}

pub fn page_gap() -> Pixels {
    space_3()
}

pub fn page_padding() -> Pixels {
    space_3()
}

pub fn panel_padding() -> Pixels {
    space_3()
}

pub fn heading_gap() -> Pixels {
    space_1()
}

pub fn ui_font() -> Font {
    let mut font = font("Inter");
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![
        "SF Pro Text".to_string(),
        ".SystemUIFont".to_string(),
        "Helvetica".to_string(),
        "Arial".to_string(),
    ]));
    font.weight = FontWeight::NORMAL;
    font
}

pub fn ui_font_medium() -> Font {
    let mut font = ui_font();
    font.weight = FontWeight::MEDIUM;
    font
}

pub fn mono_font() -> Font {
    let mut font = font("JetBrains Mono");
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![
        "SF Mono".to_string(),
        "Menlo".to_string(),
        "Monaco".to_string(),
        ".ZedMono".to_string(),
    ]));
    font
}
