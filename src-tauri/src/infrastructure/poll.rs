//! Visible-window refresh loop. Interval is fixed (see RefreshPolicy).

use crate::domain::constants::RefreshPolicy;
use crate::state::AppHandleState;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub fn spawn_refresh_loop(app_handle: AppHandle) {
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
            let every = RefreshPolicy::DEFAULT_REFRESH_SECS.max(1);
            if !tick.is_multiple_of(every) {
                continue;
            }
            let core = Arc::clone(&state.core);
            let snaps = match tokio::task::spawn_blocking(move || core.refresh_all()).await {
                Ok(s) => s,
                Err(_) => continue,
            };
            let _ = app_handle.emit("snapshots-updated", &snaps);
        }
    });
}
