use crate::domain::types::{
    CardTint, DiagnosticsSnapshot, PersistedState, ProviderId, ProviderSnapshot,
};
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_provider_activity(app: AppHandle) -> Vec<crate::domain::activity::ProviderActivity> {
    crate::infrastructure::activity::get(&app)
}

#[tauri::command]
pub fn get_notch_reveal(app: AppHandle) -> Result<bool, String> {
    crate::infrastructure::notch_window::get_reveal(&app)
}

#[tauri::command]
pub fn set_notch_focus(app: AppHandle, focused: bool) -> Result<bool, String> {
    crate::infrastructure::notch_window::set_focus(&app, focused)
}

#[tauri::command]
pub fn get_notch_monitors(
    app: AppHandle,
) -> Result<Vec<crate::domain::notch::MonitorArea>, String> {
    crate::infrastructure::notch_window::monitors(&app)
}

#[tauri::command]
pub fn set_notch_placement(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    placement: crate::domain::notch::NotchPlacement,
) -> Result<crate::domain::notch::NotchLayout, String> {
    state.core.set_notch_placement(placement)?;
    crate::infrastructure::notch_window::apply(&app, None)
}

#[tauri::command]
pub fn begin_notch_drag(app: AppHandle, id: u64) -> Result<(), String> {
    crate::infrastructure::notch_window::begin_drag(&app, id)
}
#[tauri::command]
pub fn finish_notch_drag(app: AppHandle, id: u64, cancel: bool) -> Result<(), String> {
    crate::infrastructure::notch_window::finish_drag(&app, id, cancel)
}

#[tauri::command]
pub fn set_notch_surface(
    app: AppHandle,
    revision: u64,
    expanded: bool,
    height: f64,
    target: Option<f64>,
) -> Result<crate::domain::notch::NotchLayout, String> {
    crate::infrastructure::notch_window::apply(&app, Some((revision, expanded, height, target)))
}

#[tauri::command]
pub fn get_state(state: State<'_, AppHandleState>) -> PersistedState {
    state.core.get_state()
}

#[tauri::command]
pub fn get_snapshots(state: State<'_, AppHandleState>) -> Vec<ProviderSnapshot> {
    state.core.get_snapshots()
}

#[tauri::command]
pub fn set_opacity(state: State<'_, AppHandleState>, opacity: f64) -> Result<f64, String> {
    state.core.set_opacity(opacity)
}

/// Sync login-item registration with the OS.
///
/// **Release only for enable:** `tauri dev` runs `target/debug/token-usage.exe`,
/// which loads the Vite `devUrl`. If that path is registered for boot, login
/// starts a broken widget (no Vite) and overwrites a good install-path entry.
/// Preference is still persisted; OS registration is applied on the next
/// release/install run. Disable is always applied so a bad entry can be cleared
/// from a debug session.
pub(crate) fn sync_os_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if cfg!(debug_assertions) {
        if !enabled {
            let _ = autostart.disable();
        }
        return Ok(());
    }
    if enabled {
        autostart.enable().map_err(|e| e.to_string())?;
    } else {
        autostart.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_autostart(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    enabled: bool,
) -> Result<(), String> {
    state.core.set_autostart(enabled)?;
    sync_os_autostart(&app, enabled)
}

#[tauri::command]
pub fn set_hover_detail(state: State<'_, AppHandleState>, enabled: bool) -> Result<(), String> {
    state.core.set_hover_detail(enabled)
}

#[tauri::command]
pub async fn set_provider_enabled(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    provider: ProviderId,
    enabled: bool,
) -> Result<Vec<ProviderSnapshot>, String> {
    let core = Arc::clone(&state.core);
    let snaps = tokio::task::spawn_blocking(move || core.set_provider_enabled(provider, enabled))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.emit("snapshots-updated", &snaps);
    Ok(snaps)
}

#[tauri::command]
pub fn set_provider_tint(
    state: State<'_, AppHandleState>,
    provider: ProviderId,
    tint: CardTint,
) -> Result<(), String> {
    state.core.set_provider_tint(provider, tint)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_diagnostics(state: State<'_, AppHandleState>) -> DiagnosticsSnapshot {
    state.core.diagnostics()
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
        return;
    }
    let _ = window_ctl::show_window(&window);
    state.core.set_visible(true);
    let app = app.clone();
    let core = Arc::clone(&state.core);
    tauri::async_runtime::spawn(async move {
        let snaps = match tokio::task::spawn_blocking(move || core.refresh_all()).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let _ = app.emit("snapshots-updated", &snaps);
    });
}
