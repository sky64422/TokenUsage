//! Read-only CLI lifecycle hints. Transcript contents are never retained or emitted.
mod evidence;
mod monitor;
pub use monitor::ActivityMonitor;

use crate::domain::{
    activity::{ActivityState, ProviderActivity},
    types::ProviderId,
};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const SAMPLE_SECONDS: u64 = 2;
const ACTIVITY_EVENT: &str = "provider-activity";

struct ActivityCache(Arc<Mutex<Vec<ProviderActivity>>>);

pub fn get(app: &AppHandle) -> Vec<ProviderActivity> {
    app.state::<ActivityCache>()
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub fn start(app: &AppHandle) {
    let unknown = ProviderId::all()
        .into_iter()
        .map(|provider_id| ProviderActivity {
            provider_id,
            state: ActivityState::Unknown,
            observed_at: None,
        })
        .collect();
    let cache = Arc::new(Mutex::new(unknown));
    if !app.manage(ActivityCache(Arc::clone(&cache))) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut monitor = ActivityMonitor::new();
        loop {
            let next = monitor.sample(Utc::now());
            let changed = {
                let mut previous = cache
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if *previous == next {
                    false
                } else {
                    *previous = next.clone();
                    true
                }
            };
            if changed && app.emit(ACTIVITY_EVENT, &next).is_err() {
                eprintln!("Activity update could not be delivered");
            }
            std::thread::sleep(std::time::Duration::from_secs(SAMPLE_SECONDS));
        }
    });
}
