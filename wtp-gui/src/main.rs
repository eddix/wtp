//! wtp-gui entry point
//!
//! GUI application for managing git workspaces.
//! Uses GPUI framework for rendering and tray-icon for system tray integration.

mod app;
mod components;
mod state;
mod tray;
mod views;

fn main() {
    // TODO: Initialize GPUI App once the gpui dependency is pinned
    // let app = gpui::App::new();
    // app.run(|cx| {
    //     let state = cx.new(|_| state::AppState::load());
    //     tray::setup_tray(cx, state.clone());
    // });
    eprintln!("wtp-gui: GPUI dependency not yet configured. See Cargo.toml for details.");
    std::process::exit(1);
}
