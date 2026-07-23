//! Claude Code local usage from `~/.claude/projects/**/*.jsonl`.
//!
//! Optional official-ish capture:
//! - `~/.claude-monitor/state/latest.json` (claude-monitor companion)
//! - `~/.token-usage/claude-rate-limits.json` (manual statusline dump)

use crate::domain::types::{
    DataSource, PlanLimits, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow,
    WindowKind,
};
use crate::domain::usage_math::{build_snapshot_from_events, unavailable_snapshot, UsageEvent};
use crate::infrastructure::providers::paths::{claude_home, home_dir};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn fetch(limits: &PlanLimits) -> ProviderSnapshot {
    if let Some(snap) = try_official_companion(limits) {
        return snap;
    }

    let Some(root) = claude_home() else {
        return unavailable_snapshot(ProviderId::Claude, "Home directory not found");
    };
    if !root.exists() {
        return unavailable_snapshot(
            ProviderId::Claude,
            format!("Claude data not found at {}", root.display()),
        );
    }

    let projects = root.join("projects");
    let events = if projects.is_dir() {
        collect_jsonl_events(&projects)
    } else {
        Vec::new()
    };

    build_snapshot_from_events(
        ProviderId::Claude,
        events,
        limits,
        Utc::now(),
        DataSource::Estimate,
        Some(format!("Local estimate from {}", projects.display())),
    )
}

fn try_official_companion(limits: &PlanLimits) -> Option<ProviderSnapshot> {
    let candidates: Vec<PathBuf> = [
        home_dir().map(|h| h.join(".claude-monitor").join("state").join("latest.json")),
        home_dir().map(|h| h.join(".token-usage").join("claude-rate-limits.json")),
        claude_home().map(|h| h.join("rate_limits.json")),
    ]
    .into_iter()
    .flatten()
    .collect();

    for path in candidates {
        if let Some(snap) = parse_rate_limits_file(&path, limits) {
            return Some(snap);
        }
    }
    None
}

fn parse_rate_limits_file(path: &Path, _limits: &PlanLimits) -> Option<ProviderSnapshot> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;

    // Support a few shapes:
    // 1) { "rate_limits": { "five_hour": { used_percentage, resets_at }, "seven_day": ... } }
    // 2) { "five_hour_percent": n, "five_hour_resets_at": "...", ... }
    // 3) claude-monitor snapshot with nested fields — best-effort
    let mut windows = Vec::new();

    if let Some(rl) = v.get("rate_limits") {
        push_window_from_obj(
            &mut windows,
            WindowKind::Rolling5h,
            "5-hour",
            rl.get("five_hour")
                .or_else(|| rl.get("fiveHour"))
                .or_else(|| rl.get("primary")),
        );
        push_window_from_obj(
            &mut windows,
            WindowKind::Weekly,
            "Weekly",
            rl.get("seven_day")
                .or_else(|| rl.get("sevenDay"))
                .or_else(|| rl.get("weekly"))
                .or_else(|| rl.get("secondary")),
        );
    }

    if windows.is_empty() {
        if let Some(p) = v
            .get("five_hour_percent")
            .and_then(|x| x.as_f64())
            .or_else(|| {
                v.pointer("/limits/five_hour/used_percentage")
                    .and_then(|x| x.as_f64())
            })
        {
            let resets = v
                .get("five_hour_resets_at")
                .and_then(|x| x.as_str())
                .or_else(|| {
                    v.pointer("/limits/five_hour/resets_at")
                        .and_then(|x| x.as_str())
                })
                .map(|s| s.to_string());
            windows.push(UsageWindow {
                kind: WindowKind::Rolling5h,
                used: p,
                limit: Some(100.0),
                unit: UsageUnit::Percent,
                resets_at: resets,
                used_percent: Some(p),
                label: Some("5-hour".into()),
            });
        }
        if let Some(p) = v
            .get("weekly_percent")
            .and_then(|x| x.as_f64())
            .or_else(|| {
                v.pointer("/limits/seven_day/used_percentage")
                    .and_then(|x| x.as_f64())
            })
        {
            let resets = v
                .get("weekly_resets_at")
                .and_then(|x| x.as_str())
                .or_else(|| {
                    v.pointer("/limits/seven_day/resets_at")
                        .and_then(|x| x.as_str())
                })
                .map(|s| s.to_string());
            windows.push(UsageWindow {
                kind: WindowKind::Weekly,
                used: p,
                limit: Some(100.0),
                unit: UsageUnit::Percent,
                resets_at: resets,
                used_percent: Some(p),
                label: Some("Weekly".into()),
            });
        }
    }

    if windows.is_empty() {
        return None;
    }

    let primary_used_percent = windows
        .iter()
        .filter_map(|w| w.used_percent)
        .fold(None, |acc: Option<f64>, p| {
            Some(acc.map(|a| a.max(p)).unwrap_or(p))
        });
    let primary_resets_at = windows.iter().filter_map(|w| w.resets_at.clone()).min();

    Some(ProviderSnapshot {
        provider_id: ProviderId::Claude,
        display_name: ProviderId::Claude.display_name().into(),
        windows,
        status: SnapshotStatus::Ok,
        source: DataSource::LocalFile,
        as_of: Utc::now().to_rfc3339(),
        message: Some(format!("From {}", path.display())),
        primary_resets_at,
        primary_used_percent,
    })
}

fn push_window_from_obj(
    out: &mut Vec<UsageWindow>,
    kind: WindowKind,
    label: &str,
    obj: Option<&Value>,
) {
    let Some(obj) = obj else { return };
    let pct = obj
        .get("used_percentage")
        .or_else(|| obj.get("usedPercentage"))
        .or_else(|| obj.get("utilization"))
        .and_then(|x| x.as_f64());
    let resets = obj
        .get("resets_at")
        .or_else(|| obj.get("resetsAt"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    if pct.is_none() && resets.is_none() {
        return;
    }
    let used = pct.unwrap_or(0.0);
    out.push(UsageWindow {
        kind,
        used,
        limit: Some(100.0),
        unit: UsageUnit::Percent,
        resets_at: resets,
        used_percent: pct,
        label: Some(label.into()),
    });
}

fn collect_jsonl_events(projects_dir: &Path) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let cutoff = Utc::now() - chrono::Duration::days(8);

    for entry in WalkDir::new(projects_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
    {
        // Skip huge inactive files by mtime
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                let modified: DateTime<Utc> = modified.into();
                if modified < cutoff {
                    continue;
                }
            }
        }
        if let Ok(file) = File::open(entry.path()) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if line.len() < 20 || !line.contains("usage") {
                    continue;
                }
                if let Some(ev) = parse_claude_line(&line) {
                    if ev.at >= cutoff {
                        events.push(ev);
                    }
                }
            }
        }
    }
    events
}

fn parse_claude_line(line: &str) -> Option<UsageEvent> {
    let v: Value = serde_json::from_str(line).ok()?;
    // assistant messages carry message.usage
    let usage = v.pointer("/message/usage").or_else(|| v.get("usage"))?;
    let input = usage
        .get("input_tokens")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);
    let output = usage
        .get("output_tokens")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);
    let cache_create = usage
        .get("cache_creation_input_tokens")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);
    // Count billable-ish tokens: input + output + cache write (not cache reads)
    let tokens = input + output + cache_create;
    if tokens <= 0.0 {
        return None;
    }
    let ts = v
        .get("timestamp")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/message/timestamp").and_then(|x| x.as_str()))?;
    let at = DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|d| d.with_timezone(&Utc))?;
    Some(UsageEvent { at, tokens })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_assistant_usage_line() {
        let line = r#"{"type":"assistant","timestamp":"2026-07-22T10:00:00.000Z","message":{"usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":10,"cache_read_input_tokens":999}}}"#;
        let ev = parse_claude_line(line).expect("parse");
        assert!((ev.tokens - 160.0).abs() < 0.01);
    }
}
