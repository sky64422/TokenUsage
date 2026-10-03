//! Visible-window refresh loop. Interval is fixed (see RefreshPolicy).

use crate::domain::constants::RefreshPolicy;
use crate::state::AppHandleState;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

static REFRESH_NOTIFIER: OnceLock<Arc<Notify>> = OnceLock::new();

fn notifier() -> Arc<Notify> {
    REFRESH_NOTIFIER.get_or_init(|| Arc::new(Notify::new())).clone()
}

pub fn notify_refresh() {
    if let Some(notify) = REFRESH_NOTIFIER.get() {
        notify.notify_one();
    }
}

pub fn spawn_refresh_loop(app_handle: AppHandle) {
    let notify = notifier();
    tauri::async_runtime::spawn(async move {
        let mut tick: u64 = 0;
        loop {
            let every = RefreshPolicy::DEFAULT_REFRESH_SECS.max(1);
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(RefreshPolicy::TICK_SECS)) => {
                    tick = tick.wrapping_add(1);
                    if !tick.is_multiple_of(every) {
                        continue;
                    }
                }
                _ = notify.notified() => {
                    // Woken up immediately by background provider fetch completion
                }
            }
            let Some(state) = app_handle.try_state::<AppHandleState>() else {
                continue;
            };
            if !state.core.is_visible() {
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
