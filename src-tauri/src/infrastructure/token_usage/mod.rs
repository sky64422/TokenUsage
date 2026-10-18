mod reader;

use crate::domain::token_usage::{TokenLedger, TokenUsageSnapshot, TokenUsageStatus};
use chrono::Local;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_secs(5);
struct State { reader: Option<reader::TokenReader>, cached: TokenUsageSnapshot, running: bool, completed: Option<Instant> }
static STATE: OnceLock<Arc<Mutex<State>>> = OnceLock::new();

/// No credentials or transcript discovery in isolated fixture mode.
pub fn snapshot() -> TokenUsageSnapshot {
    if std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA").as_deref() == Ok("1") {
        return TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Ready);
    }
    let state = STATE.get_or_init(|| Arc::new(Mutex::new(State {
        reader: crate::infrastructure::providers::paths::codex_home().map(|p| reader::TokenReader::new(p.join("sessions"))),
        cached: TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Loading), running: false, completed: None,
    })));
    let Ok(mut guard) = state.lock() else { return TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Unavailable); };
    if !guard.running && guard.completed.is_none_or(|t| t.elapsed() >= REFRESH) {
        guard.running = true;
        let mut reader = guard.reader.take();
        let state = Arc::clone(state);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let now = Local::now();
                if let Some(reader) = reader.as_mut() { reader.scan(now); reader.summary(now) }
                else { TokenLedger::default().summarize(now, TokenUsageStatus::Unavailable) }
            }));
            if let Ok(mut guard) = state.lock() {
                guard.cached = result.unwrap_or_else(|_| TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Unavailable));
                guard.reader = reader; guard.running = false; guard.completed = Some(Instant::now());
            }
            crate::infrastructure::poll::notify_refresh();
        });
    }
    guard.cached.clone()
}
