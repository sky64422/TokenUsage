//! Visible-window refresh loop. Interval is fixed (see RefreshPolicy).

use crate::application::service::AppCore;
use crate::domain::constants::RefreshPolicy;
use crate::domain::types::ProviderSnapshot;
use crate::state::AppHandleState;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

static REFRESH_NOTIFIER: OnceLock<Arc<Notify>> = OnceLock::new();

fn notifier() -> Arc<Notify> {
    REFRESH_NOTIFIER
        .get_or_init(|| Arc::new(Notify::new()))
        .clone()
}

pub fn notify_refresh() {
    if let Some(notify) = REFRESH_NOTIFIER.get() {
        notify.notify_one();
    }
}

/// Shared blocking refresh and event delivery for boot, periodic wakeups and showing.
pub async fn refresh_and_emit(app: &AppHandle, core: Arc<AppCore>) {
    let worker = Arc::clone(&core);
    if let Err(error) = refresh_and_deliver(
        move || worker.refresh_all(),
        |snapshots| {
            app.emit("snapshots-updated", snapshots)
                .map_err(|e| e.to_string())
        },
    )
    .await
    {
        core.note_diag(error);
    }
}

async fn refresh_and_deliver(
    refresh: impl FnOnce() -> Vec<ProviderSnapshot> + Send + 'static,
    emit: impl FnOnce(&[ProviderSnapshot]) -> Result<(), String>,
) -> Result<(), String> {
    let snapshots = tokio::task::spawn_blocking(refresh)
        .await
        .map_err(|_| "Snapshot refresh worker failed".to_string())?;
    emit(&snapshots).map_err(|error| format!("Snapshot event delivery failed: {error}"))
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
            refresh_and_emit(&app_handle, core).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{DataSource, ProviderId, SnapshotStatus};

    #[tokio::test]
    async fn refresh_delivers_the_worker_snapshots() {
        let snapshot = ProviderSnapshot {
            provider_id: ProviderId::Grok,
            display_name: "Grok".into(),
            windows: vec![],
            status: SnapshotStatus::Unavailable,
            source: DataSource::Unavailable,
            as_of: "2026-10-04T00:00:00Z".into(),
            message: Some("Fixture unavailable".into()),
            primary_resets_at: None,
            primary_used_percent: None,
        };
        let expected = snapshot.clone();
        let mut delivered = None;
        refresh_and_deliver(
            move || vec![snapshot],
            |snapshots| {
                delivered = Some(snapshots.to_vec());
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(delivered, Some(vec![expected]));
    }

    #[tokio::test]
    async fn refresh_worker_failure_is_reported_without_emitting_empty_success() {
        let mut emitted = false;
        let result = refresh_and_deliver(
            || panic!("fixture worker failure"),
            |_| {
                emitted = true;
                Ok(())
            },
        )
        .await;
        assert!(result.is_err());
        assert!(!emitted);
    }

    #[tokio::test]
    async fn refresh_delivery_failure_is_reported() {
        let result =
            refresh_and_deliver(Vec::new, |_| Err("fixture delivery failure".into())).await;
        assert_eq!(
            result.unwrap_err(),
            "Snapshot event delivery failed: fixture delivery failure"
        );
    }
}
