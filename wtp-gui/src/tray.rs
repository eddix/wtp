//! System tray integration via tray-icon crate.
//!
//! Creates a menu bar icon with quick access to key workspace actions.

use crate::state::{AppState, ViewState};
use gpui::{App, AsyncApp, Entity};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

const MENU_OPEN_DASHBOARD: &str = "open_dashboard";
const MENU_CREATE_WORKSPACE: &str = "create_workspace";
const MENU_QUIT: &str = "quit";
const MENU_EMPTY_WORKSPACES: &str = "no_workspaces";

std::thread_local! {
    static TRAY_HANDLE: std::cell::RefCell<Option<TrayIcon>> = const { std::cell::RefCell::new(None) };
}

/// Set up the system tray icon and keep its menu in sync with workspace state.
pub fn setup_tray(cx: &mut App, state: Entity<AppState>) -> Result<(), String> {
    let icon = load_tray_icon()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(build_tray_menu(&state, cx)))
        .with_icon(icon)
        .with_tooltip("wtp — WorkTree for Polyrepo")
        .build()
        .map_err(|error| format!("Tray icon unavailable: {error}"))?;

    TRAY_HANDLE.with(|handle| {
        *handle.borrow_mut() = Some(tray);
    });

    cx.observe(&state, {
        let state = state.clone();
        move |_, cx| refresh_tray_menu(&state, cx)
    })
    .detach();

    let (menu_tx, menu_rx) = smol::channel::unbounded::<MenuEvent>();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_tx.try_send(event);
    }));

    let state_clone = state.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(event) = menu_rx.recv().await {
            let state = state_clone.clone();
            cx.update(|cx| {
                handle_menu_event(&event.id, cx, &state);
            });
        }
    })
    .detach();

    Ok(())
}

fn refresh_tray_menu(state: &Entity<AppState>, cx: &App) {
    let menu = build_tray_menu(state, cx);
    TRAY_HANDLE.with(|handle| {
        if let Some(tray) = handle.borrow().as_ref() {
            tray.set_menu(Some(Box::new(menu)));
        }
    });
}

fn build_tray_menu(state: &Entity<AppState>, cx: &App) -> Menu {
    let menu = Menu::new();
    let app_state = state.read(cx);

    if app_state.workspaces.is_empty() {
        let _ = menu.append(&MenuItem::with_id(
            MenuId::new(MENU_EMPTY_WORKSPACES),
            "No workspaces yet",
            false,
            None,
        ));
    } else {
        for workspace in &app_state.workspaces {
            let _ = menu.append(&MenuItem::with_id(
                MenuId::new(&format!("ws:{}", workspace.name)),
                &workspace.name,
                true,
                None,
            ));
        }
    }

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(
        MenuId::new(MENU_OPEN_DASHBOARD),
        "Open Dashboard…",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(
        MenuId::new(MENU_CREATE_WORKSPACE),
        "Create Workspace…",
        true,
        None,
    ));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(
        MenuId::new(MENU_QUIT),
        "Quit wtp",
        true,
        None,
    ));

    menu
}

fn handle_menu_event(id: &MenuId, cx: &mut App, state: &Entity<AppState>) {
    let id_str = id.as_ref();
    match id_str {
        MENU_QUIT => cx.quit(),
        MENU_OPEN_DASHBOARD => crate::app::open_main_window(cx, state.clone()),
        MENU_CREATE_WORKSPACE => {
            state.update(cx, |state, cx| {
                state.navigate(ViewState::CreateWorkspace);
                cx.notify();
            });
            crate::app::open_main_window(cx, state.clone());
        }
        other if other.starts_with("ws:") => {
            let workspace_name = other.trim_start_matches("ws:");
            state.update(cx, |state, cx| {
                state.open_workspace_detail(workspace_name.to_string(), false);
                cx.notify();
            });
            crate::app::open_main_window(cx, state.clone());
        }
        _ => {}
    }
}

fn load_tray_icon() -> Result<tray_icon::Icon, String> {
    let icon_bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(icon_bytes)
        .map_err(|error| format!("Failed to decode tray icon: {error}"))?;
    let rgba = image.into_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
        .map_err(|error| format!("Failed to create tray icon: {error}"))
}
