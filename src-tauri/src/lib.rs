//! Token Usage — floating agent usage widget.

pub mod application;
mod commands;
pub mod domain;
pub mod infrastructure;
mod state;

use infrastructure::store::{default_state, load_state, save_state};
use infrastructure::updater;
use infrastructure::window_ctl;
use state::AppHandleState;
use std::sync::Arc;
use tauri::{Emitter, Manager};
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
            let persisted = match load_state(&app_data_dir) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("load_state: {e}");
                    default_state()
                }
            };
            let _ = save_state(&app_data_dir, &persisted);

            let core = application::service::AppCore::new(persisted.clone(), app_data_dir);
            let handle_state = AppHandleState::new(Arc::clone(&core));

            if let Some(window) = app.get_webview_window("main") {
                let _ = window_ctl::apply_always_on_top(&window, true);
                // Floating widget: desktop + tray only, not the taskbar.
                let _ = window.set_skip_taskbar(true);
                let _ = window_ctl::apply_geometry(&window, &persisted.settings.window);
                let _ = window_ctl::apply_clean_glass_edge(&window);
                let _ = window_ctl::show_window(&window);
            }

            if let Err(e) = infrastructure::tray::setup_system_tray(app) {
                eprintln!("system tray setup failed: {e}");
            }

            // Prefer release binary for OS login items (see commands::sync_os_autostart).
            let _ = commands::sync_os_autostart(app.handle(), persisted.settings.autostart);

            let hotkey = persisted.settings.hotkey.clone();
            if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }

            app.manage(handle_state);
            app.manage(updater::PendingUpdateState::default());

            updater::spawn_update_check(app.handle().clone());
            infrastructure::poll::spawn_refresh_loop(app.handle().clone());

            let boot_core = Arc::clone(&core);
            let boot_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let snaps =
                    match tokio::task::spawn_blocking(move || boot_core.refresh_all()).await {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                let _ = boot_app.emit("snapshots-updated", &snaps);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Resized(size) = event {
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
            commands::set_opacity,
            commands::set_autostart,
            commands::set_window_geometry,
            commands::set_provider_enabled,
            commands::set_provider_tint,
            commands::hide_widget,
            commands::quit_app,
            commands::get_diagnostics,
            commands::set_content_min_size,
            commands::check_for_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TokenUsage");
}
