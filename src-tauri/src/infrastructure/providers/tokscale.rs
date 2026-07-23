//! Primary data path: `tokscale usage --json` (vendor-reported quotas).
//!
//! Falls back is handled by the caller when this returns Err or partial map.

use crate::domain::types::{
    DataSource, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow, WindowKind,
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

struct CacheEntry {
    at: Instant,
    raw: String,
}

static CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(45);

/// Run tokscale (or return cached stdout) and map to our providers.
pub fn fetch_all() -> Result<HashMap<ProviderId, ProviderSnapshot>, String> {
    let raw = run_tokscale_usage_json()?;
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
        // monthly / 30d vendor windows
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
        // Already RFC3339 or similar
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(dt.with_timezone(&Utc).to_rfc3339());
        }
        return Some(s.to_string());
    }
    if let Some(n) = v.as_i64() {
        // unix seconds
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

fn run_tokscale_usage_json() -> Result<String, String> {
    // short-lived process cache
    {
        let guard = CACHE.lock().unwrap();
        if let Some(entry) = guard.as_ref() {
            if entry.at.elapsed() < CACHE_TTL {
                return Ok(entry.raw.clone());
            }
        }
    }

    let output = spawn_tokscale()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "tokscale exited {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("tokscale produced empty stdout".into());
    }
    // Must look like JSON array/object
    if !(stdout.starts_with('[') || stdout.starts_with('{')) {
        return Err(format!(
            "tokscale stdout is not JSON: {}",
            stdout.chars().take(120).collect::<String>()
        ));
    }

    {
        let mut guard = CACHE.lock().unwrap();
        *guard = Some(CacheEntry {
            at: Instant::now(),
            raw: stdout.clone(),
        });
    }
    Ok(stdout)
}

fn spawn_tokscale() -> Result<std::process::Output, String> {
    // Prefer PATH binary, then npx (first install may be slow)
    match run_cmd("tokscale", &["usage", "--json"]) {
        Ok(out) if out.status.success() => return Ok(out),
        Ok(out) => {
            // Binary exists but failed — still try npx only if stdout empty
            if !out.stdout.is_empty() {
                return Ok(out);
            }
        }
        Err(_) => {}
    }

    match run_cmd("npx", &["--yes", "tokscale", "usage", "--json"]) {
        Ok(out) => Ok(out),
        Err(e) => Err(format!(
            "tokscale not available ({e}). Install: npm i -g tokscale"
        )),
    }
}

fn run_cmd(program: &str, args: &[&str]) -> Result<std::process::Output, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    // Hide console window on Windows when spawned from GUI app
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
        .map_err(|e| format!("failed to spawn {program}: {e}"))
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
}
