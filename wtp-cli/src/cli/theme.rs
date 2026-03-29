use clap::builder::styling::{AnsiColor, Style, Styles};

pub const fn wtp_styles() -> Styles {
    Styles::styled()
        .header(Style::new().bold())
        .usage(Style::new().bold())
        .literal(AnsiColor::Blue.on_default())
        .placeholder(AnsiColor::Magenta.on_default())
        .error(AnsiColor::Red.on_default().bold())
        .valid(AnsiColor::Cyan.on_default().bold())
        .invalid(AnsiColor::Red.on_default())
}
