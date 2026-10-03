//! Official CLI quotas only. Never uses cloudcode-pa, browser scraping or estimates.
use crate::domain::types::{DataSource, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow, WindowKind};
use chrono::Utc;
use std::{sync::{Mutex, OnceLock}, time::{Duration, Instant}};
use super::{agy_cli, snapshot::finish_snapshot};

const CLI_CACHE_TTL: Duration = Duration::from_secs(60);
const CLI_ERROR_TTL: Duration = Duration::from_secs(15);
const CLI_TIMEOUT: Duration = Duration::from_secs(40);
const WAITING: &str = "Reading Antigravity CLI quota";

fn cache_file_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| d.join("TokenUsage").join("quota-cache").join("agy_snapshot.json"))
}

fn save_disk_cache(snapshot: &ProviderSnapshot) {
    if let Some(path) = cache_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(snapshot) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn load_disk_cache() -> Option<ProviderSnapshot> {
    let path = cache_file_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<ProviderSnapshot>(&text).ok()
}

struct Cache {
    pending: bool,
    completed: Option<Instant>,
    snapshot: Option<ProviderSnapshot>,
    error: Option<String>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            pending: false,
            completed: None,
            snapshot: load_disk_cache(),
            error: None,
        }
    }
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub fn fetch() -> Result<ProviderSnapshot, String> {
    if std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA").as_deref() == Ok("1") {
        return Err("Direct quota disabled for preview".into());
    }
    let cache = CACHE.get_or_init(Default::default);
    let mut state = cache.lock().map_err(|_| "Antigravity cache lock poisoned")?;
    let ttl = if state.error.is_some() { CLI_ERROR_TTL } else { CLI_CACHE_TTL };
    if !state.pending && state.completed.is_none_or(|at| at.elapsed() >= ttl) {
        state.pending = true;
        // CLI startup must not delay other providers or the fixed 5s UI refresh.
        std::thread::spawn(move || {
            let result = read_snapshot();
            let mut state = cache.lock().expect("Antigravity cache lock poisoned");
            state.pending = false;
            state.completed = Some(Instant::now());
            match result {
                Ok(snapshot) => {
                    save_disk_cache(&snapshot);
                    state.snapshot = Some(snapshot);
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
            drop(state);
            crate::infrastructure::poll::notify_refresh();
        });
    }
    if let Some(mut snapshot) = state.snapshot.clone() {
        if let Some(error) = &state.error {
            snapshot.status = SnapshotStatus::Degraded;
            snapshot.message = Some(format!("Last known quota: {error}"));
        }
        return Ok(snapshot);
    }
    if state.pending && state.error.is_none() {
        return Ok(ProviderSnapshot {
            provider_id: ProviderId::Agy,
            display_name: ProviderId::Agy.display_name().into(),
            windows: vec![],
            status: SnapshotStatus::Ok,
            source: DataSource::Vendor,
            as_of: Utc::now().to_rfc3339(),
            message: Some(WAITING.into()),
            primary_resets_at: None,
            primary_used_percent: None,
        });
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

fn parse_reset_time(raw: &str) -> Result<chrono::DateTime<Utc>, String> {
    use chrono::{DateTime, Local, LocalResult, NaiveDateTime, TimeZone};
    let trimmed = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        if dt.timestamp() > 0 {
            return Ok(dt.with_timezone(&Utc));
        }
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S %z",
        "%Y-%m-%d %H:%M %z",
        "%Y/%m/%d %H:%M:%S %z",
        "%Y/%m/%d %H:%M %z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(trimmed, fmt) {
            if dt.timestamp() > 0 {
                return Ok(dt.with_timezone(&Utc));
            }
        }
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 2 {
        let dt_str = format!("{} {}", parts[0], parts[1]);
        for fmt in [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d %H:%M",
        ] {
            if let Ok(naive) = NaiveDateTime::parse_from_str(&dt_str, fmt) {
                let dt = match Local.from_local_datetime(&naive) {
                    LocalResult::Single(dt) => dt.with_timezone(&Utc),
                    LocalResult::Ambiguous(dt, _) => dt.with_timezone(&Utc),
                    LocalResult::None => Utc.from_utc_datetime(&naive),
                };
                if dt.timestamp() > 0 {
                    return Ok(dt);
                }
            }
        }
    }
    Err("Invalid Antigravity reset time".into())
}

fn parse_quota(text: &str) -> Result<Vec<UsageWindow>, String> {
    let clean = agy_cli::sanitize_terminal_output(text);
    if !clean.contains("Limit Remaining") && !clean.lines().any(|line| line.trim() == "Quota:") {
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
        let reset = parse_reset_time(reset)?;
        let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
        let (family, kind, period) = if let Some(family) = label.strip_suffix(" Five Hour Limit Remaining") {
            (family, WindowKind::Rolling5h, "5h")
        } else if let Some(family) = label.strip_suffix(" Weekly Limit Remaining") {
            (family, WindowKind::Weekly, "Week")
        } else { return Err("Unrecognized Antigravity quota period".into()); };
        let group = family.replace(" Models", "").replace(" models", "").replace(" and ", "/");
        let group = if group == "Claude/GPT" || group == "Claude and GPT" {
            "Claude".to_string()
        } else {
            group
        };
        let used = 100. - remaining;
        let window = UsageWindow {
            group: Some(group), kind, used, limit: Some(100.), unit: UsageUnit::Percent,
            resets_at: Some(reset.to_rfc3339()), used_percent: Some(used), label: Some(period.into()),
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
        assert_eq!(windows[2].group.as_deref(), Some("Claude"));
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

    #[test]
    fn parses_conpty_terminal_output_with_localized_timestamps() {
        const CONPTY_REPORT: &str = "\
Quota:\n\
Gemini Models          Weekly Limit Remaining     33%   2026-10-04 00:55 KST\n\
Gemini Models          Five Hour Limit Remaining  70%   2026-10-03 13:30 KST\n\
Claude and GPT models  Weekly Limit Remaining     2%    2026-10-04 17:33 KST\n\
Claude and GPT models  Five Hour Limit Remaining  100%  2026-10-03 14:00 KST\n";

        let windows = parse_quota(CONPTY_REPORT).expect("parse conpty report");
        assert_eq!(windows.len(), 4);
        // Gemini 5h first, then Weekly
        assert_eq!(windows[0].group.as_deref(), Some("Gemini"));
        assert_eq!(windows[0].kind, WindowKind::Rolling5h);
        assert_eq!(windows[0].used_percent, Some(30.));
        assert!(windows[0].resets_at.is_some());

        assert_eq!(windows[1].group.as_deref(), Some("Gemini"));
        assert_eq!(windows[1].kind, WindowKind::Weekly);
        assert_eq!(windows[1].used_percent, Some(67.));

        // Claude 5h then Weekly
        assert_eq!(windows[2].group.as_deref(), Some("Claude"));
        assert_eq!(windows[2].kind, WindowKind::Rolling5h);
        assert_eq!(windows[2].used_percent, Some(0.));

        assert_eq!(windows[3].group.as_deref(), Some("Claude"));
        assert_eq!(windows[3].kind, WindowKind::Weekly);
        assert_eq!(windows[3].used_percent, Some(98.));
    }

    #[test]
    fn parses_pipe_output_without_quota_header() {
        const PIPE_REPORT: &str = "\
Gemini Models\tWeekly Limit Remaining\t24%\t2026-10-03T15:55:06Z\n\
Gemini Models\tFive Hour Limit Remaining\t20%\t2026-10-03T04:30:10Z\n\
Claude and GPT models\tWeekly Limit Remaining\t2%\t2026-10-04T08:33:40Z\n\
Claude and GPT models\tFive Hour Limit Remaining\t100%\t2026-10-03T06:25:35Z\n";

        let windows = parse_quota(PIPE_REPORT).expect("parse pipe report");
        assert_eq!(windows.len(), 4);
        assert_eq!(windows[0].group.as_deref(), Some("Gemini"));
        assert_eq!(windows[0].kind, WindowKind::Rolling5h);
        assert_eq!(windows[0].used_percent, Some(80.));
    }
}
