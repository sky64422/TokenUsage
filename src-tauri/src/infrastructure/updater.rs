//! Tauri updater: startup auto-check (release only) + manual install path.

use crate::state::AppHandleState;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(30);

pub fn spawn_update_check(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(UPDATE_CHECK_DELAY).await;
        if let Err(err) = check_and_install_update(&app).await {
            note(&app, format!("updater check failed: {err}"));
        }
    });
}

pub async fn check_and_install_update(app: &AppHandle) -> Result<bool, String> {
    note(app, "updater check started");

    let updater = app.updater().map_err(|e| map_updater_err(e.to_string()))?;
    match updater
        .check()
        .await
        .map_err(|e| map_updater_err(e.to_string()))?
    {
        Some(update) => {
            note(
                app,
                format!(
                    "update available: {} -> {}",
                    update.current_version, update.version
                ),
            );
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| map_updater_err(e.to_string()))?;
            note(app, "update installed — restarting");
            // Does not return; process is replaced by the new binary when possible.
            app.restart();
        }
        None => {
            note(app, "no update available");
            Ok(false)
        }
    }
}

/// Map raw updater/network errors into actionable messages.
fn map_updater_err(raw: String) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("404")
        || lower.contains("not found")
        || lower.contains("failed to fetch")
        || lower.contains("error sending request")
        || lower.contains("connection")
        || lower.contains("timed out")
    {
        return format!(
            "cannot reach update endpoint (private GitHub repo or network). \
             latest.json must be public. Original: {raw}"
        );
    }
    raw
}

fn note(app: &AppHandle, message: impl Into<String>) {
    let message = message.into();
    if let Some(state) = app.try_state::<AppHandleState>() {
        state.core.note_diag(message);
    } else {
        eprintln!("{message}");
    }
}
