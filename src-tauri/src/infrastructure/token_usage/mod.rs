mod reader;

use crate::domain::token_usage::{TokenLedger, TokenUsageSnapshot, TokenUsageStatus};
use crate::domain::types::UsageWindow;
use chrono::Local;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_secs(5);
struct State {
    reader: Option<reader::TokenReader>,
    cached: TokenUsageSnapshot,
    running: bool,
    completed: Option<Instant>,
}
static STATE: OnceLock<Arc<Mutex<State>>> = OnceLock::new();

/// No credentials or transcript discovery in isolated fixture mode.
pub fn snapshot(windows: &[UsageWindow]) -> TokenUsageSnapshot {
    if std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA").as_deref() == Ok("1") {
        return TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Ready, &[]);
    }
    let state = STATE.get_or_init(|| {
        Arc::new(Mutex::new(State {
            reader: crate::infrastructure::providers::paths::codex_home()
                .map(|p| reader::TokenReader::new(p.join("sessions"))),
            cached: TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Loading, &[]),
            running: false,
            completed: None,
        }))
    });
    let Ok(mut guard) = state.lock() else {
        return TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Unavailable, &[]);
    };
    if !guard.running && guard.completed.is_none_or(|t| t.elapsed() >= REFRESH) {
        guard.running = true;
        let mut reader = guard.reader.take();
        let state = Arc::clone(state);
        let windows = windows.to_vec();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let now = Local::now();
                if let Some(reader) = reader.as_mut() {
                    reader.scan(now);
                    reader.summary(now, &windows)
                } else {
                    TokenLedger::default().summarize(now, TokenUsageStatus::Unavailable, &[])
                }
            }));
            let mut changed = false;
            if let Ok(mut guard) = state.lock() {
                let next = result.unwrap_or_else(|_| {
                    TokenLedger::default().summarize(
                        Local::now(),
                        TokenUsageStatus::Unavailable,
                        &[],
                    )
                });
                changed = guard.cached.source != next.source
                    || guard.cached.status != next.status
                    || guard.cached.today != next.today
                    || guard.cached.week != next.week
                    || guard.cached.month != next.month
                    || guard.cached.windows != next.windows;
                guard.cached = next;
                guard.reader = reader;
                guard.running = false;
                guard.completed = Some(Instant::now());
            }
            if changed {
                crate::infrastructure::poll::notify_refresh();
            }
        });
    }
    guard.cached.clone()
}
