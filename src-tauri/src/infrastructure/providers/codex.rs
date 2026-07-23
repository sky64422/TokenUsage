//! Codex local usage from `~/.codex/sessions/**/rollout-*.jsonl` token_count events.

use crate::domain::types::{DataSource, PlanLimits, ProviderId, ProviderSnapshot};
use crate::domain::usage_math::{build_snapshot_from_events, unavailable_snapshot, UsageEvent};
use crate::infrastructure::providers::paths::codex_home;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use walkdir::WalkDir;

pub fn fetch(limits: &PlanLimits) -> ProviderSnapshot {
    let Some(root) = codex_home() else {
        return unavailable_snapshot(ProviderId::Codex, "Home directory not found");
    };
    let sessions = root.join("sessions");
    if !sessions.is_dir() {
        return unavailable_snapshot(ProviderId::Codex, "Codex sessions not found");
    }

    let events = collect_events(&sessions);
    build_snapshot_from_events(
        ProviderId::Codex,
        events,
        limits,
        Utc::now(),
        DataSource::Estimate,
        None,
    )
}

fn collect_events(sessions_dir: &Path) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let cutoff = Utc::now() - chrono::Duration::days(8);

    for entry in WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
    {
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                let modified: DateTime<Utc> = modified.into();
                if modified < cutoff {
                    continue;
                }
            }
        }

        // Per-file: track last cumulative total so we can emit deltas
        let mut last_total: Option<f64> = None;
        if let Ok(file) = File::open(entry.path()) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if !line.contains("token_count") {
                    continue;
                }
                if let Some((at, total)) = parse_token_count_line(&line) {
                    if at < cutoff {
                        last_total = Some(total);
                        continue;
                    }
                    let delta = match last_total {
                        Some(prev) if total >= prev => total - prev,
                        Some(_) => total,         // reset / new thread
                        None => total.min(total), // first sighting: use last_token if available below
                    };
                    // Prefer last_token_usage for first event accuracy
                    if last_total.is_none() {
                        if let Some(last) = parse_last_token_usage(&line) {
                            events.push(UsageEvent { at, tokens: last });
                        } else if delta > 0.0 {
                            events.push(UsageEvent { at, tokens: delta });
                        }
                    } else if delta > 0.0 {
                        events.push(UsageEvent { at, tokens: delta });
                    }
                    last_total = Some(total);
                }
            }
        }
    }
    events
}

fn parse_token_count_line(line: &str) -> Option<(DateTime<Utc>, f64)> {
    let v: Value = serde_json::from_str(line).ok()?;
    let typ = v.get("type").and_then(|x| x.as_str())?;
    // event_msg with payload.type == token_count
    let is_token = typ == "token_count"
        || v.pointer("/payload/type").and_then(|x| x.as_str()) == Some("token_count");
    if !is_token {
        return None;
    }
    let total = v
        .pointer("/payload/info/total_token_usage/total_tokens")
        .or_else(|| v.pointer("/info/total_token_usage/total_tokens"))
        .and_then(|x| x.as_f64())?;
    let ts = v.get("timestamp").and_then(|x| x.as_str())?;
    let at = DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|d| d.with_timezone(&Utc))?;
    Some((at, total))
}

fn parse_last_token_usage(line: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(line).ok()?;
    // Prefer last turn total if present
    v.pointer("/payload/info/last_token_usage/total_tokens")
        .or_else(|| v.pointer("/info/last_token_usage/total_tokens"))
        .and_then(|x| x.as_f64())
        .or_else(|| {
            let input = v
                .pointer("/payload/info/last_token_usage/input_tokens")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            let output = v
                .pointer("/payload/info/last_token_usage/output_tokens")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            if input + output > 0.0 {
                Some(input + output)
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_token_count() {
        let line = r#"{"timestamp":"2026-07-22T13:56:50.854Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":14392},"last_token_usage":{"total_tokens":14392,"input_tokens":14045,"output_tokens":347}}}}"#;
        let (at, total) = parse_token_count_line(line).expect("parse");
        assert!((total - 14392.0).abs() < 0.1);
        assert!(at.timestamp() > 0);
    }
}
