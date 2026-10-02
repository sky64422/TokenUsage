//! Official CLI quotas only. Never uses cloudcode-pa, browser scraping or estimates.
use crate::domain::types::{ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow, WindowKind};
use chrono::Utc;
use std::{sync::{Mutex, OnceLock}, time::{Duration, Instant}};
use super::{agy_cli, snapshot::finish_snapshot};

const CLI_CACHE_TTL: Duration = Duration::from_secs(60);
const CLI_TIMEOUT: Duration = Duration::from_secs(40);
const WAITING: &str = "Reading Antigravity CLI quota";

#[derive(Default)]
struct Cache {
    pending: bool,
    completed: Option<Instant>,
    snapshot: Option<ProviderSnapshot>,
    error: Option<String>,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub fn fetch() -> Result<ProviderSnapshot, String> {
    if std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA").as_deref() == Ok("1") {
        return Err("Direct quota disabled for preview".into());
    }
    let cache = CACHE.get_or_init(Default::default);
    let mut state = cache.lock().map_err(|_| "Antigravity cache lock poisoned")?;
    if !state.pending && state.completed.is_none_or(|at| at.elapsed() >= CLI_CACHE_TTL) {
        state.pending = true;
        // CLI startup must not delay other providers or the fixed 5s UI refresh.
        std::thread::spawn(move || {
            let result = read_snapshot();
            let mut state = cache.lock().expect("Antigravity cache lock poisoned");
            state.pending = false;
            state.completed = Some(Instant::now());
            match result {
                Ok(snapshot) => { state.snapshot = Some(snapshot); state.error = None; }
                Err(error) => state.error = Some(error),
            }
        });
    }
    if let Some(mut snapshot) = state.snapshot.clone() {
        if let Some(error) = &state.error {
            snapshot.status = SnapshotStatus::Degraded;
            snapshot.message = Some(format!("Last known quota: {error}"));
        }
        return Ok(snapshot);
    }
    Err(state.error.clone().unwrap_or_else(|| WAITING.into()))
}

/// One bounded read, also used by the opt-in live verification example.
pub fn read_snapshot() -> Result<ProviderSnapshot, String> {
    let program = agy_cli::find_agy().ok_or("Antigravity CLI not installed; install and sign in with agy")?;
    let cwd = dirs::cache_dir().ok_or("Cache directory unavailable")?.join("TokenUsage").join("quota-work");
    std::fs::create_dir_all(&cwd).map_err(|_| "Cannot create Antigravity quota working directory")?;
    let text = agy_cli::run_cmd_conpty(&program,
        &["--sandbox", "--print-timeout", "30s", "--print", "/usage"], Some(&cwd), CLI_TIMEOUT)?;
    Ok(finish_snapshot(ProviderId::Agy, parse_quota(&text)?, None, Utc::now()))
}

fn parse_quota(text: &str) -> Result<Vec<UsageWindow>, String> {
    let clean = agy_cli::sanitize_terminal_output(text);
    if !clean.lines().any(|line| line.trim() == "Quota:") {
        return Err("No Antigravity quota report; check agy sign-in".into());
    }
    let mut windows = Vec::new();
    for line in clean.lines().filter(|line| line.contains("Limit Remaining")) {
        let (left, reset) = line.rsplit_once('%').ok_or("Invalid Antigravity quota row")?;
        let (label, value) = left.trim().rsplit_once(char::is_whitespace).ok_or("Missing Antigravity percentage")?;
        let remaining: f64 = value.parse().map_err(|_| "Invalid Antigravity percentage")?;
        if !remaining.is_finite() || !(0. ..=100.).contains(&remaining) {
            return Err("Antigravity percentage out of range".into());
        }
        let reset = chrono::DateTime::parse_from_rfc3339(reset.trim()).map_err(|_| "Invalid Antigravity reset time")?;
        if reset.timestamp() <= 0 { return Err("Invalid Antigravity reset time".into()); }
        let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
        let (family, kind, period) = if let Some(family) = label.strip_suffix(" Five Hour Limit Remaining") {
            (family, WindowKind::Rolling5h, "5h")
        } else if let Some(family) = label.strip_suffix(" Weekly Limit Remaining") {
            (family, WindowKind::Weekly, "Week")
        } else { return Err("Unrecognized Antigravity quota period".into()); };
        let group = family.replace(" Models", "").replace(" models", "").replace(" and ", "/");
        let used = 100. - remaining;
        let window = UsageWindow {
            group: Some(group), kind, used, limit: Some(100.), unit: UsageUnit::Percent,
            resets_at: Some(reset.with_timezone(&Utc).to_rfc3339()), used_percent: Some(used), label: Some(period.into()),
        };
        // Terminal redraws may repeat a row; the last complete observation wins.
        if let Some(previous) = windows.iter_mut().find(|w: &&mut UsageWindow| w.group == window.group && w.kind == window.kind) {
            *previous = window;
        } else { windows.push(window); }
    }
    if windows.is_empty() { return Err("Antigravity returned no recognized quota windows".into()); }
    windows.sort_by_key(|w| (if w.group.as_deref() == Some("Gemini") { 0 } else { 1 }, w.group.clone(), if w.kind == WindowKind::Rolling5h { 0 } else { 1 }));
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    const REPORT: &str = "Quota:\nGemini Models Five Hour Limit Remaining 25% 2026-10-03T01:00:00Z\nGemini Models Weekly Limit Remaining 56% 2026-10-07T00:00:00Z\nClaude and GPT Models Five Hour Limit Remaining 4% 2026-10-03T01:20:00Z\nClaude and GPT Models Weekly Limit Remaining 35% 2026-10-07T01:00:00Z\n";

    #[test]
    #[ignore = "Explicit opt-in: reads signed-in official Antigravity CLI quota"]
    fn live_official_cli_quota() {
        let snapshot = read_snapshot().expect("official Antigravity quota");
        assert!(!snapshot.windows.is_empty());
        println!("Antigravity: {} official quota windows; primary used {}%", snapshot.windows.len(), snapshot.primary_used_percent.unwrap());
    }

    #[test]
    fn maps_remaining_to_used_and_keeps_model_families_separate() {
        let windows = parse_quota(REPORT).unwrap();
        assert_eq!(windows.len(), 4);
        assert_eq!(windows[0].used_percent, Some(75.));
        assert_eq!(windows[0].group.as_deref(), Some("Gemini"));
        assert_eq!(windows[2].group.as_deref(), Some("Claude/GPT"));
        assert_eq!(windows[2].used_percent, Some(96.));
        assert_eq!(windows[1].kind, WindowKind::Weekly);
        assert_eq!(windows[0].resets_at.as_deref(), Some("2026-10-03T01:00:00+00:00"));
    }
    #[test]
    fn terminal_escapes_and_crlf_do_not_change_quota() {
        assert_eq!(parse_quota(REPORT).unwrap(), parse_quota(&format!("\x1b[32m{}\x1b[0m", REPORT.replace('\n', "\r\n"))).unwrap());
    }
    #[test]
    fn redraws_replace_rows_and_week_first_reports_sort_by_family_then_period() {
        let mut lines: Vec<_> = REPORT.lines().skip(1).collect();
        lines.reverse();
        let text = format!("Quota:\n{}\nGemini Models Five Hour Limit Remaining 20% 2026-10-03T01:00:00Z", lines.join("\n"));
        let windows = parse_quota(&text).unwrap();
        assert_eq!(windows.len(), 4);
        assert_eq!(windows[0].kind, WindowKind::Rolling5h);
        assert_eq!(windows[0].used_percent, Some(80.));
    }
    #[test]
    fn missing_malformed_or_out_of_range_is_not_zero() {
        for text in ["", "Please sign in", "Quota:\n", "Quota:\nFive Hour Limit Remaining 110% 2026-10-03T01:00:00Z", "Quota:\nFive Hour Limit Remaining NaN% 2026-10-03T01:00:00Z", "Quota:\nFive Hour Limit Remaining 50% tomorrow"] {
            assert!(parse_quota(text).is_err());
        }
    }
}
