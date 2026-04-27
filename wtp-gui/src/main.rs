//! wtp-gui entry point
//!
//! Initializes the GPUI application, sets up the system tray icon,
//! and enters the main event loop.

mod app;
mod assets;
mod components;
mod state;
mod tray;
mod views;

use assets::Assets;
use gpui::AppContext;
use state::FlashLevel;

fn main() {
    gpui_platform::application().with_assets(Assets).run(|cx| {
        let font_warning = Assets
            .load_fonts(cx)
            .err()
            .map(|error| format!("Failed to load bundled fonts, using fallbacks: {error}"));

        components::text_input::init(cx);

        // Create shared application state as a GPUI Entity
        let state = cx.new(|_cx| state::AppState::load());
        if let Some(warning) = font_warning {
            state.update(cx, |state, cx| {
                state.set_flash(FlashLevel::Warning, warning);
                cx.notify();
            });
        }

        // Set up system tray icon with workspace quick-access menu
        if let Err(error) = tray::setup_tray(cx, state.clone()) {
            state.update(cx, |state, cx| {
                state.set_flash(FlashLevel::Warning, error);
                cx.notify();
            });
        }

        // Open the main window immediately on launch
        app::open_main_window(cx, state);
    });
}
