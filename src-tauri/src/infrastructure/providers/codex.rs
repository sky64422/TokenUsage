//! Codex local usage from `~/.codex/sessions/**/rollout-*.jsonl` token_count events.
//!
//! Accounting notes:
//! - `total_token_usage.total_tokens` is a **session cumulative** that grows with
//!   every turn (context re-reads included). It is **not** a subscription quota.
//! - We only emit a usage event when the cumulative **increases**, using the
//!   increase as the turn cost (equals `last_token_usage` when the series is clean).
//! - Duplicate token_count lines with the same total are ignored.
//! - Local % vs PlanLimits is a rough estimate; prefer `tokscale` for real quotas.

use crate::domain::types::{DataSource, PlanLimits, ProviderId, ProviderSnapshot};
use crate::domain::usage_math::{
    build_snapshot_from_events, unavailable_snapshot, UsageEvent,
};
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

        let mut last_total: Option<f64> = None;
        if let Ok(file) = File::open(entry.path()) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if !line.contains("token_count") {
                    continue;
                }
                if let Some((at, total, last_turn)) = parse_token_count_line(&line) {
                    if at < cutoff {
                        // Keep baseline so first in-window increase is correct
                        last_total = Some(total);
                        continue;
                    }
                    match last_total {
                        None => {
                            // First observation in window: charge the last turn only
                            // (not the full cumulative, which would invent pre-window usage).
                            let tokens = last_turn.filter(|t| *t > 0.0).unwrap_or(0.0);
                            if tokens > 0.0 {
                                events.push(UsageEvent { at, tokens });
                            }
                            last_total = Some(total);
                        }
                        Some(prev) if total > prev + 0.5 => {
                            // Strict increase → turn cost
                            let delta = total - prev;
                            // Prefer last_turn when it matches the delta (sanity)
                            let tokens = match last_turn {
                                Some(lt) if (lt - delta).abs() < 1.0 || lt <= delta + 1.0 => {
                                    // Use delta of cumulative as ground truth for session accounting
                                    delta
                                }
                                _ => delta,
                            };
                            if tokens > 0.0 {
                                events.push(UsageEvent { at, tokens });
                            }
                            last_total = Some(total);
                        }
                        Some(prev) if total < prev - 0.5 => {
                            // Counter reset / new accounting epoch — do not add full total
                            last_total = Some(total);
                            if let Some(lt) = last_turn.filter(|t| *t > 0.0) {
                                events.push(UsageEvent { at, tokens: lt });
                            }
                        }
                        Some(_) => {
                            // same total (duplicate event) — ignore
                        }
                    }
                }
            }
        }
    }
    events
}

/// Returns (timestamp, cumulative total_tokens, optional last turn tokens).
fn parse_token_count_line(line: &str) -> Option<(DateTime<Utc>, f64, Option<f64>)> {
    let v: Value = serde_json::from_str(line).ok()?;
    let typ = v.get("type").and_then(|x| x.as_str())?;
    let is_token = typ == "token_count"
        || v.pointer("/payload/type").and_then(|x| x.as_str()) == Some("token_count");
    if !is_token {
        return None;
    }
    let total = v
        .pointer("/payload/info/total_token_usage/total_tokens")
        .or_else(|| v.pointer("/info/total_token_usage/total_tokens"))
        .and_then(|x| x.as_f64())?;
    let last_turn = v
        .pointer("/payload/info/last_token_usage/total_tokens")
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
        });
    let ts = v.get("timestamp").and_then(|x| x.as_str())?;
    let at = DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|d| d.with_timezone(&Utc))?;
    Some((at, total, last_turn))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_token_count() {
        let line = r#"{"timestamp":"2026-07-22T13:56:50.854Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":14392},"last_token_usage":{"total_tokens":14392,"input_tokens":14045,"output_tokens":347}}}}"#;
        let (at, total, last) = parse_token_count_line(line).expect("parse");
        assert!((total - 14392.0).abs() < 0.1);
        assert!((last.unwrap() - 14392.0).abs() < 0.1);
        assert!(at.timestamp() > 0);
    }

    #[test]
    fn delta_logic_skips_duplicates_and_uses_increase() {
        // Simulate collect loop
        let lines = [
            r#"{"timestamp":"2026-07-22T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1000},"last_token_usage":{"total_tokens":1000}}}}"#,
            r#"{"timestamp":"2026-07-22T10:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1000},"last_token_usage":{"total_tokens":1000}}}}"#,
            r#"{"timestamp":"2026-07-22T10:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":2500},"last_token_usage":{"total_tokens":1500}}}}"#,
            r#"{"timestamp":"2026-07-22T10:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":4000},"last_token_usage":{"total_tokens":1500}}}}"#,
        ];
        let mut last_total: Option<f64> = None;
        let mut events: Vec<f64> = Vec::new();
        for line in lines {
            let (_at, total, last_turn) = parse_token_count_line(line).unwrap();
            match last_total {
                None => {
                    events.push(last_turn.unwrap_or(0.0));
                    last_total = Some(total);
                }
                Some(prev) if total > prev + 0.5 => {
                    events.push(total - prev);
                    last_total = Some(total);
                }
                Some(prev) if total < prev - 0.5 => {
                    last_total = Some(total);
                    if let Some(lt) = last_turn {
                        events.push(lt);
                    }
                }
                _ => {}
            }
        }
        // first turn 1000 + delta 1500 + delta 1500 = 4000 (= peak)
        assert!((events.iter().sum::<f64>() - 4000.0).abs() < 0.1);
        assert_eq!(events.len(), 3);
    }
}
