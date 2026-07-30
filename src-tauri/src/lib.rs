//! Token Usage — floating agent usage widget.

pub mod application;
mod commands;
pub mod domain;
pub mod infrastructure;
mod state;

use domain::constants::RefreshPolicy;
use infrastructure::store::{load_state, save_state};
use infrastructure::updater;
use infrastructure::window_ctl;
use state::AppHandleState;
use std::sync::Arc;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _sc, event| {
                    if event.state() == ShortcutState::Pressed {
                        commands::toggle_visibility_from_handle(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
            let persisted = load_state(&app_data_dir);
            let _ = save_state(&app_data_dir, &persisted);

            let core = application::service::AppCore::new(persisted.clone(), app_data_dir);
            let handle_state = AppHandleState::new(Arc::clone(&core));

            if let Some(window) = app.get_webview_window("main") {
                let _ = window_ctl::apply_always_on_top(&window, true);
                // Floating widget: desktop + tray only, not the taskbar.
                let _ = window.set_skip_taskbar(true);
                let _ = window_ctl::apply_geometry(&window, &persisted.settings.window);
                let _ = window_ctl::apply_opacity(app.handle(), persisted.settings.opacity);
                let _ = window_ctl::apply_clean_glass_edge(&window);
                let _ = window_ctl::show_window(&window);
            }

            if let Err(e) = setup_system_tray(app) {
                eprintln!("system tray setup failed: {e}");
            }

            {
                use tauri_plugin_autostart::ManagerExt;
                let autostart = app.autolaunch();
                if persisted.settings.autostart {
                    let _ = autostart.enable();
                } else {
                    let _ = autostart.disable();
                }
            }

            // Register hotkey
            let hotkey = persisted.settings.hotkey.clone();
            if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }

            // Initial refresh
            let snaps = core.refresh_all();
            let _ = app.emit("snapshots-updated", &snaps);

            app.manage(handle_state);

            // In-app updates (release builds only; skipped under debug_assertions)
            updater::spawn_update_check(app.handle().clone());

            // Background refresh loop
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut tick: u64 = 0;
                loop {
                    tokio::time::sleep(Duration::from_secs(RefreshPolicy::TICK_SECS)).await;
                    tick = tick.wrapping_add(1);
                    let Some(state) = app_handle.try_state::<AppHandleState>() else {
                        continue;
                    };
                    if !state.core.is_visible() {
                        continue;
                    }
                    let every = state.core.refresh_secs().max(1);
                    if tick.is_multiple_of(every) {
                        let snaps = state.core.refresh_all();
                        let _ = app_handle.emit("snapshots-updated", &snaps);
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(size) = event {
                if let Some(state) = window.app_handle().try_state::<AppHandleState>() {
                    let (min_w, min_h) = state.content_min_logical();
                    let _ = window_ctl::clamp_physical_size_to_content_min(
                        window, *size, min_w, min_h,
                    );
                }
                // Re-clip rounded HWND after size changes (content-hug, user drag).
                if let Some(w) = window.app_handle().get_webview_window(window.label()) {
                    let _ = window_ctl::apply_clean_glass_edge(&w);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_snapshots,
            commands::refresh_now,
            commands::set_theme,
            commands::set_opacity,
            commands::set_autostart,
            commands::set_refresh_secs,
            commands::set_use_tokscale,
            commands::set_use_direct_quota,
            commands::set_window_geometry,
            commands::set_provider_enabled,
            commands::set_provider_limits,
            commands::hide_widget,
            commands::quit_app,
            commands::get_diagnostics,
            commands::set_content_min_size,
            commands::check_for_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TokenUsage");
}

/// Tray-only presence for a desktop widget (no taskbar button).
fn setup_system_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
