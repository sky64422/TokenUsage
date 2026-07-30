//! Codex / ChatGPT subscription quota via `chatgpt.com/backend-api/wham/usage`.

use crate::domain::types::{
    DataSource, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow, WindowKind,
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::codex_fetch;
use crate::infrastructure::providers::credentials::codex as codex_creds;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

pub fn fetch() -> Result<ProviderSnapshot, String> {
    if std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Err("skipped by TOKENUSAGE_SKIP_DIRECT_QUOTA".into());
    }
    let creds = codex_creds::load()?;
    let raw = codex_fetch::get_usage_json(USAGE_URL, &creds.access_token, creds.account_id.as_deref())?;
    parse_wham_usage(&raw)
}

/// Pure parse of wham/usage JSON (unit-tested).
pub fn parse_wham_usage(raw: &str) -> Result<ProviderSnapshot, String> {
    let v: WhamUsage = serde_json::from_str(raw).map_err(|e| format!("codex usage parse: {e}"))?;
    let now = Utc::now();
    let mut windows = Vec::new();

    if let Some(rl) = v.rate_limit.as_ref() {
        if let Some(w) = window_from_rate_window(rl.primary_window.as_ref(), "primary") {
            windows.push(w);
        }
        if let Some(w) = window_from_rate_window(rl.secondary_window.as_ref(), "secondary") {
            windows.push(w);
        }
    }

    // additional_rate_limits: array or object — best-effort
    if let Some(extra) = v.additional_rate_limits.as_ref() {
        push_additional(&mut windows, extra);
    }

    let plan = v
        .plan_type
        .as_ref()
        .filter(|p| !p.is_empty())
        .cloned();

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

    let status = if windows.is_empty() {
        SnapshotStatus::Degraded
    } else {
        SnapshotStatus::Ok
    };

    Ok(ProviderSnapshot {
        provider_id: ProviderId::Codex,
        display_name: ProviderId::Codex.display_name().into(),
        windows,
        status,
        source: DataSource::Vendor,
        as_of: now.to_rfc3339(),
        message: plan,
        primary_resets_at,
        primary_used_percent,
    })
}

#[derive(Debug, Deserialize)]
struct WhamUsage {
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<RateLimitBlock>,
    #[serde(default)]
    additional_rate_limits: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RateLimitBlock {
    #[serde(default)]
    primary_window: Option<RateWindow>,
    #[serde(default)]
    secondary_window: Option<RateWindow>,
}

#[derive(Debug, Deserialize)]
struct RateWindow {
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    limit_window_seconds: Option<i64>,
    #[serde(default)]
    reset_at: Option<Value>,
    #[serde(default)]
    reset_after_seconds: Option<i64>,
}

fn window_from_rate_window(w: Option<&RateWindow>, fallback_label: &str) -> Option<UsageWindow> {
    let w = w?;
    let raw_pct = w.used_percent?;
    let over = raw_pct > 100.0;
    let used_percent = Some(raw_pct.clamp(0.0, 100.0));
    let kind = classify_window_seconds(w.limit_window_seconds);
    let label = kind_label(kind, w.limit_window_seconds, fallback_label);
    let resets_at = parse_reset_at(w.reset_at.as_ref(), w.reset_after_seconds);

    Some(UsageWindow {
        kind,
        used: used_percent.unwrap_or(0.0),
        limit: Some(100.0),
        unit: UsageUnit::Percent,
        resets_at,
        used_percent,
        label: Some(if over {
            format!("{label} · over")
        } else {
            label
        }),
    })
}

fn push_additional(windows: &mut Vec<UsageWindow>, extra: &Value) {
    match extra {
        Value::Array(arr) => {
            for item in arr {
                if let Some(rw) = serde_json::from_value::<RateWindow>(item.clone()).ok() {
                    if let Some(w) = window_from_rate_window(Some(&rw), "extra") {
                        windows.push(w);
                    }
                } else if let Some(inner) = item.get("primary_window") {
                    if let Ok(rw) = serde_json::from_value::<RateWindow>(inner.clone()) {
                        if let Some(w) = window_from_rate_window(Some(&rw), "extra") {
                            windows.push(w);
                        }
                    }
                }
            }
        }
        Value::Object(_) => {
            if let Ok(rw) = serde_json::from_value::<RateWindow>(extra.clone()) {
                if let Some(w) = window_from_rate_window(Some(&rw), "extra") {
                    windows.push(w);
                }
            }
        }
        _ => {}
    }
}

fn classify_window_seconds(secs: Option<i64>) -> WindowKind {
    let Some(s) = secs else {
        return WindowKind::Unknown;
    };
    // Tolerate drift: 5h ≈ 18000, week ≈ 604800, 30d ≈ 2592000
    if (14_000..=22_000).contains(&s) {
        WindowKind::Rolling5h
    } else if (500_000..=700_000).contains(&s) {
        WindowKind::Weekly
    } else if (2_000_000..=3_200_000).contains(&s) {
        WindowKind::Unknown // 30-day
    } else if (80_000..=100_000).contains(&s) {
        WindowKind::Daily
    } else {
        WindowKind::Unknown
    }
}

fn kind_label(kind: WindowKind, secs: Option<i64>, fallback: &str) -> String {
    match kind {
        WindowKind::Rolling5h => "5h".into(),
        WindowKind::Weekly => "Weekly".into(),
        WindowKind::Daily => "Daily".into(),
        WindowKind::Session => "Session".into(),
        WindowKind::Unknown => {
            if let Some(s) = secs {
                if s >= 2_000_000 {
                    return "30d".into();
                }
                if s >= 80_000 {
                    let days = (s as f64 / 86_400.0).round() as i64;
                    if days > 0 {
                        return format!("{days}d");
                    }
                }
                if s >= 3600 {
                    let hours = (s as f64 / 3600.0).round() as i64;
                    return format!("{hours}h");
                }
            }
            fallback.into()
        }
    }
}

fn parse_reset_at(reset_at: Option<&Value>, reset_after_seconds: Option<i64>) -> Option<String> {
    if let Some(v) = reset_at {
        if let Some(n) = v.as_i64() {
            return Utc
                .timestamp_opt(n, 0)
                .single()
                .map(|d| d.to_rfc3339());
        }
        if let Some(n) = v.as_f64() {
            return Utc
                .timestamp_opt(n as i64, 0)
                .single()
                .map(|d| d.to_rfc3339());
        }
        if let Some(s) = v.as_str() {
            if s.is_empty() {
                return None;
            }
            if let Ok(n) = s.parse::<i64>() {
                return Utc
                    .timestamp_opt(n, 0)
                    .single()
                    .map(|d| d.to_rfc3339());
            }
            if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
                return Some(s.to_string());
            }
        }
    }
    if let Some(after) = reset_after_seconds {
        if after >= 0 {
            return Some((Utc::now() + chrono::Duration::seconds(after)).to_rfc3339());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "plan_type": "free",
      "rate_limit": {
        "allowed": false,
        "limit_reached": true,
        "primary_window": {
          "used_percent": 100,
          "limit_window_seconds": 2592000,
          "reset_after_seconds": 1902682,
          "reset_at": 1787320522
        },
        "secondary_window": null
      },
      "additional_rate_limits": null
    }"#;

    #[test]
    fn parse_free_30d_window() {
        let snap = parse_wham_usage(FIXTURE).unwrap();
        assert_eq!(snap.provider_id, ProviderId::Codex);
        assert_eq!(snap.source, DataSource::Vendor);
        assert_eq!(snap.message.as_deref(), Some("free"));
        assert_eq!(snap.windows.len(), 1);
        let w = &snap.windows[0];
        assert!((w.used_percent.unwrap() - 100.0).abs() < 0.01);
        assert_eq!(w.label.as_deref(), Some("30d"));
        assert!(w.resets_at.is_some());
        assert!((snap.primary_used_percent.unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn parse_dual_5h_week() {
        let raw = r#"{
          "plan_type": "plus",
          "rate_limit": {
            "primary_window": {
              "used_percent": 40.5,
              "limit_window_seconds": 18000,
              "reset_at": 1780000000
            },
            "secondary_window": {
              "used_percent": 12.0,
              "limit_window_seconds": 604800,
              "reset_at": 1781000000
            }
          }
        }"#;
        let snap = parse_wham_usage(raw).unwrap();
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].kind, WindowKind::Rolling5h);
        assert_eq!(snap.windows[1].kind, WindowKind::Weekly);
        assert!((snap.primary_used_percent.unwrap() - 40.5).abs() < 0.01);
    }

    #[test]
    fn classify_seconds() {
        assert_eq!(classify_window_seconds(Some(18000)), WindowKind::Rolling5h);
        assert_eq!(classify_window_seconds(Some(604800)), WindowKind::Weekly);
        assert_eq!(classify_window_seconds(Some(2592000)), WindowKind::Unknown);
    }
}
