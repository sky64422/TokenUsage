use crate::domain::types::{
    DiagnosticsSnapshot, PersistedState, PlanLimits, ProviderId, ProviderSnapshot,
    WindowGeometry,
};
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_state(state: State<'_, AppHandleState>) -> PersistedState {
    state.core.get_state()
}

#[tauri::command]
pub fn get_snapshots(state: State<'_, AppHandleState>) -> Vec<ProviderSnapshot> {
    state.core.get_snapshots()
}

#[tauri::command]
pub fn refresh_now(
    app: AppHandle,
    state: State<'_, AppHandleState>,
) -> Result<Vec<ProviderSnapshot>, String> {
    let snaps = state.core.refresh_all();
    let _ = app.emit("snapshots-updated", &snaps);
    Ok(snaps)
}

#[tauri::command]
pub fn set_opacity(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    opacity: f64,
) -> Result<f64, String> {
    let o = state.core.set_opacity(opacity)?;
    window_ctl::apply_opacity(&app, o)?;
    Ok(o)
}

#[tauri::command]
pub fn set_autostart(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    enabled: bool,
) -> Result<(), String> {
    state.core.set_autostart(enabled)?;
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|e| e.to_string())?;
    } else {
        autostart.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_refresh_secs(state: State<'_, AppHandleState>, secs: u64) -> Result<u64, String> {
    state.core.set_refresh_secs(secs)
}

#[tauri::command]
pub fn set_window_geometry(
    state: State<'_, AppHandleState>,
    geometry: WindowGeometry,
) -> Result<(), String> {
    state.core.set_window_geometry(geometry)
}

#[tauri::command]
pub fn set_provider_enabled(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    provider: ProviderId,
    enabled: bool,
) -> Result<Vec<crate::domain::types::ProviderSnapshot>, String> {
    // Persist + refresh (disabled providers omitted from list)
    let snaps = state.core.set_provider_enabled(provider, enabled)?;
    let _ = app.emit("snapshots-updated", &snaps);
    Ok(snaps)
}

#[tauri::command]
pub fn set_provider_limits(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    provider: ProviderId,
    limits: PlanLimits,
) -> Result<(), String> {
    state.core.set_provider_limits(provider, limits)?;
    let snaps = state.core.get_snapshots();
    let _ = app.emit("snapshots-updated", &snaps);
    Ok(())
}

#[tauri::command]
pub fn hide_widget(app: AppHandle, state: State<'_, AppHandleState>) -> Result<(), String> {
    let window = window_ctl::main_window(&app)?;
    window_ctl::hide_window(&window)?;
    state.core.set_visible(false);
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_diagnostics(state: State<'_, AppHandleState>) -> DiagnosticsSnapshot {
    state.core.diagnostics()
}

/// Update OS min-size from measured content height (logical px).
/// `grow_if_needed`: snap height to content (grow or shrink). False = min only (live ticks).
#[tauri::command(rename_all = "snake_case")]
pub fn set_content_min_size(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    width: f64,
    height: f64,
    grow_if_needed: bool,
) -> Result<(), String> {
    let w = width.max(1.0).ceil() as u32;
    let h = height.max(1.0).ceil() as u32;
    state.set_content_min(w, h);
    let window = window_ctl::main_window(&app)?;
    window_ctl::apply_content_min_size(&window, width, height)?;
    if grow_if_needed {
        // Full content-hug: avoid leftover empty glass under cards.
        window_ctl::snap_height_to_content(&window, width, height)?;
        let _ = window_ctl::apply_clean_glass_edge(&window);
    }
    Ok(())
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<bool, String> {
    crate::infrastructure::updater::check_and_install_update(&app).await
}

pub fn toggle_visibility_from_handle(app: &AppHandle) {
    let Some(state) = app.try_state::<AppHandleState>() else {
        return;
    };
    let Ok(window) = window_ctl::main_window(app) else {
        return;
    };
    if state.core.is_visible() {
        let _ = window_ctl::hide_window(&window);
        state.core.set_visible(false);
    } else {
        let _ = window_ctl::show_window(&window);
        state.core.set_visible(true);
        let snaps = state.core.refresh_all();
        let _ = app.emit("snapshots-updated", &snaps);
    }
}
