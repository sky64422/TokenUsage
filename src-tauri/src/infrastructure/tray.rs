//! Tray-only presence for a desktop widget (no taskbar button).

use crate::commands;
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

pub fn setup_system_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let hide_i = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &hide_i, &quit_i])?;

    let icon = app
        .default_window_icon()
        .ok_or("default window icon missing")?
        .clone();

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Token Usage")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(state) = app.try_state::<AppHandleState>() {
                    if let Ok(window) = window_ctl::main_window(app) {
                        let _ = window_ctl::show_window(&window);
                        state.core.set_visible(true);
                    }
                }
            }
            "hide" => {
                if let Some(state) = app.try_state::<AppHandleState>() {
                    if let Ok(window) = window_ctl::main_window(app) {
                        let _ = window_ctl::hide_window(&window);
                        state.core.set_visible(false);
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                commands::toggle_visibility_from_handle(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
