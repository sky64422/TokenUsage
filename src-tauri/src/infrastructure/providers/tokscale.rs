//! Primary data path: `tokscale usage --json` (vendor-reported quotas).
//!
//! Process spawn lives in `tokscale_exec` (excluded from coverage gate).
//! Fallback is handled by the caller when this returns Err or partial map.

use crate::domain::types::{
    DataSource, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow, WindowKind,
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct TokscaleProvider {
    provider: String,
    #[serde(default)]
    plan: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    metrics: Vec<TokscaleMetric>,
}

#[derive(Debug, Deserialize)]
struct TokscaleMetric {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    remaining_percent: Option<f64>,
    #[serde(default)]
    remaining_label: Option<String>,
    #[serde(default)]
    resets_at: Option<Value>,
}

/// Run tokscale (or return cached stdout) and map to our providers.
pub fn fetch_all() -> Result<HashMap<ProviderId, ProviderSnapshot>, String> {
    if std::env::var("TOKENUSAGE_SKIP_TOKSCALE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Err("skipped by TOKENUSAGE_SKIP_TOKSCALE".into());
    }
    let raw = super::tokscale_exec::run_tokscale_usage_json()?;
    parse_usage_json(&raw)
}

pub fn parse_usage_json(raw: &str) -> Result<HashMap<ProviderId, ProviderSnapshot>, String> {
    let items: Vec<TokscaleProvider> =
        serde_json::from_str(raw).map_err(|e| format!("tokscale JSON parse: {e}"))?;

    let now = Utc::now();
    let mut out = HashMap::new();

    for item in items {
        let Some(pid) = map_provider_name(&item.provider) else {
            continue;
        };
        if out.contains_key(&pid) {
            continue;
        }
        let snap = snapshot_from_item(pid, &item, now);
        out.insert(pid, snap);
    }

    if out.is_empty() {
        return Err("tokscale returned no matching providers (claude/codex/grok)".into());
    }
    Ok(out)
}

fn snapshot_from_item(
    pid: ProviderId,
    item: &TokscaleProvider,
    now: chrono::DateTime<Utc>,
) -> ProviderSnapshot {
    let mut windows = Vec::new();
    for m in &item.metrics {
        let kind = classify_label(m.label.as_deref().unwrap_or(""));
        let used_percent = m
            .used_percent
            .or_else(|| m.remaining_percent.map(|r| (100.0 - r).clamp(0.0, 100.0)));
        let resets_at = parse_resets_at(&m.resets_at);
        let label = m
            .label
            .clone()
            .unwrap_or_else(|| kind_default_label(kind).into());

        windows.push(UsageWindow {
            kind,
            used: used_percent.unwrap_or(0.0),
            limit: Some(100.0),
            unit: UsageUnit::Percent,
            resets_at,
            used_percent,
            label: Some(label),
        });
    }

    let primary_used_percent = windows
        .iter()
        .filter_map(|w| w.used_percent)
        .fold(None, |acc: Option<f64>, p| {
            Some(acc.map(|a| a.max(p)).unwrap_or(p))
        });

    let primary_resets_at = windows
        .iter()
        .filter_map(|w| w.resets_at.as_ref())
        .filter_map(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .filter(|d| d.with_timezone(&Utc) > now)
        .min()
        .map(|d| d.with_timezone(&Utc).to_rfc3339())
        .or_else(|| windows.iter().filter_map(|w| w.resets_at.clone()).min());

    let plan = item.plan.clone().unwrap_or_else(|| "—".into());
    let mut msg_parts = vec![format!("tokscale · plan {plan}")];
    if let Some(email) = &item.email {
        msg_parts.push(email.clone());
    }
    if let Some(rem) = item.metrics.iter().find_map(|m| m.remaining_label.as_ref()) {
        msg_parts.push(rem.clone());
    }

    let status = if windows.is_empty() {
        SnapshotStatus::Degraded
    } else {
        SnapshotStatus::Ok
    };

    ProviderSnapshot {
        provider_id: pid,
        display_name: pid.display_name().into(),
        windows,
        status,
        source: DataSource::Tokscale,
        as_of: now.to_rfc3339(),
        message: Some(msg_parts.join(" · ")),
        primary_resets_at,
        primary_used_percent,
    }
}

fn map_provider_name(name: &str) -> Option<ProviderId> {
    let n = name.trim().to_ascii_lowercase();
    if n.contains("claude") {
        Some(ProviderId::Claude)
    } else if n.contains("codex") || n.contains("openai") {
        Some(ProviderId::Codex)
    } else if n.contains("grok") {
        Some(ProviderId::Grok)
    } else {
        None
    }
}

fn classify_label(label: &str) -> WindowKind {
    let l = label.trim().to_ascii_lowercase();
    if l.contains("5") && (l.contains("h") || l.contains("hour") || l.contains("session"))
        || l == "session"
        || l == "5h"
        || l == "5hr"
        || l == "5-hour"
    {
        WindowKind::Rolling5h
    } else if l.contains("week") || l == "7d" || l.contains("7-day") || l.contains("7 day") {
        WindowKind::Weekly
    } else if l.contains("30") || l.contains("month") {
        WindowKind::Unknown
    } else if l.contains("day") || (l.ends_with('d') && l.chars().any(|c| c.is_ascii_digit())) {
        WindowKind::Daily
    } else {
        WindowKind::Unknown
    }
}

fn kind_default_label(kind: WindowKind) -> &'static str {
    match kind {
        WindowKind::Rolling5h => "5-hour",
        WindowKind::Weekly => "Weekly",
        WindowKind::Daily => "Daily",
        WindowKind::Session => "Session",
        WindowKind::Unknown => "Quota",
    }
}

fn parse_resets_at(v: &Option<Value>) -> Option<String> {
    let v = v.as_ref()?;
    if let Some(s) = v.as_str() {
        if s.is_empty() {
            return None;
        }
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(dt.with_timezone(&Utc).to_rfc3339());
        }
        return Some(s.to_string());
    }
    if let Some(n) = v.as_i64() {
        return Utc.timestamp_opt(n, 0).single().map(|d| d.to_rfc3339());
    }
    if let Some(n) = v.as_f64() {
        return Utc
            .timestamp_opt(n as i64, 0)
            .single()
            .map(|d| d.to_rfc3339());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
  {
    "provider": "Codex",
    "plan": "Free",
    "email": "a@b.com",
    "metrics": [
      {
        "label": "30d",
        "used_percent": 100.0,
        "remaining_percent": 0.0,
        "remaining_label": null,
        "resets_at": "2026-08-21T13:55:22+00:00"
      }
    ]
  },
  {
    "provider": "Grok Build",
    "plan": null,
    "email": "a@b.com",
    "metrics": [
      {
        "label": "Weekly",
        "used_percent": 12.0,
        "remaining_percent": 88.0,
        "remaining_label": null,
        "resets_at": "2026-07-30T12:36:54+00:00"
      }
    ]
  },
  {
    "provider": "Claude",
    "plan": "Max",
    "metrics": [
      {
        "label": "5h",
        "used_percent": 40.0,
        "resets_at": 1738425600
      },
      {
        "label": "Weekly",
        "used_percent": 22.5,
        "resets_at": "2026-07-30T00:00:00Z"
      }
    ]
  }
]"#;

    #[test]
    fn parses_real_shape() {
        let map = parse_usage_json(SAMPLE).expect("parse");
        assert!(map.contains_key(&ProviderId::Codex));
        assert!(map.contains_key(&ProviderId::Grok));
        assert!(map.contains_key(&ProviderId::Claude));

        let claude = map.get(&ProviderId::Claude).unwrap();
        assert_eq!(claude.source, DataSource::Tokscale);
        assert_eq!(claude.windows.len(), 2);
        assert!((claude.primary_used_percent.unwrap() - 40.0).abs() < 0.01);

        let grok = map.get(&ProviderId::Grok).unwrap();
        assert!((grok.windows[0].used_percent.unwrap() - 12.0).abs() < 0.01);
        assert!(grok.primary_resets_at.is_some());
    }

    #[test]
    fn map_names() {
        assert_eq!(map_provider_name("Grok Build"), Some(ProviderId::Grok));
        assert_eq!(map_provider_name("Claude Code"), Some(ProviderId::Claude));
    }

    #[test]
    fn classify_labels() {
        assert_eq!(classify_label("5h"), WindowKind::Rolling5h);
        assert_eq!(classify_label("Weekly"), WindowKind::Weekly);
        assert_eq!(classify_label("30d"), WindowKind::Unknown);
        assert_eq!(classify_label("1d"), WindowKind::Daily);
        assert_eq!(classify_label("day"), WindowKind::Daily);
    }

    #[test]
    fn remaining_percent_fallback() {
        let raw = r#"[{"provider":"Codex","metrics":[{"label":"5h","remaining_percent":25.0}]}]"#;
        let map = parse_usage_json(raw).unwrap();
        let w = &map.get(&ProviderId::Codex).unwrap().windows[0];
        assert!((w.used_percent.unwrap() - 75.0).abs() < 0.01);
    }
}
