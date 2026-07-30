//! Claude Code subscription rate limits via `GET /api/oauth/usage`.

use crate::domain::types::{
    DataSource, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow, WindowKind,
};
use crate::infrastructure::providers::credentials::claude as claude_creds;
use chrono::{Duration, TimeZone, Utc};
use serde_json::Value;

use super::claude_fetch;

pub fn fetch() -> Result<ProviderSnapshot, String> {
    if std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Err("skipped by TOKENUSAGE_SKIP_DIRECT_QUOTA".into());
    }

    let mut creds = claude_creds::load()?;
    if claude_creds::needs_refresh(&creds, Duration::minutes(2)) {
        let Some(rt) = creds.refresh_token.clone() else {
            return Err("claude token expired; run `claude` login".into());
        };
        let client_id = std::env::var("CLAUDE_CODE_OAUTH_CLIENT_ID")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| claude_creds::DEFAULT_CLIENT_ID.into());
        match claude_fetch::refresh_access_token(&client_id, &rt) {
            Ok(tok) => {
                let exp_ms = Utc::now().timestamp_millis() + tok.expires_in.max(60) * 1000;
                let new_rt = tok.refresh_token.as_deref().or(Some(rt.as_str()));
                let _ = claude_creds::write_refreshed(
                    &creds.path,
                    &tok.access_token,
                    new_rt,
                    exp_ms,
                );
                creds.access_token = tok.access_token;
                if let Some(r) = tok.refresh_token {
                    creds.refresh_token = Some(r);
                }
                creds.expires_at_ms = Some(exp_ms);
            }
            Err(e) => {
                if creds.access_token.is_empty()
                    || claude_creds::expires_at_utc(&creds)
                        .map(|t| t <= Utc::now())
                        .unwrap_or(true)
                {
                    return Err(format!("claude token expired ({e}); run `claude` login"));
                }
            }
        }
    }

    if creds.access_token.is_empty() {
        return Err("claude access token missing; run `claude` login".into());
    }

    let raw = claude_fetch::get_usage_json(&creds.access_token)?;
    let mut snap = parse_oauth_usage(&raw)?;
    if snap.message.is_none() {
        snap.message = creds.subscription_type.clone();
    }
    Ok(snap)
}

/// Pure parse of Anthropic OAuth usage JSON (unit-tested).
pub fn parse_oauth_usage(raw: &str) -> Result<ProviderSnapshot, String> {
    let v: Value =
        serde_json::from_str(raw).map_err(|e| format!("claude usage parse: {e}"))?;
    let now = Utc::now();
    let mut windows = Vec::new();

    // Shape A: top-level five_hour / seven_day (API utilization 0–1)
    // Shape B: nested under rate_limits with used_percentage
    let rl = v.get("rate_limits").unwrap_or(&v);

    push_window(
        &mut windows,
        WindowKind::Rolling5h,
        "5-hour",
        rl.get("five_hour")
            .or_else(|| rl.get("fiveHour"))
            .or_else(|| v.get("five_hour")),
    );
    push_window(
        &mut windows,
        WindowKind::Weekly,
        "Weekly",
        rl.get("seven_day")
            .or_else(|| rl.get("sevenDay"))
            .or_else(|| v.get("seven_day")),
    );
    // Optional model-specific weekly bars
    push_window(
        &mut windows,
        WindowKind::Weekly,
        "Opus week",
        rl.get("seven_day_opus")
            .or_else(|| rl.get("sevenDayOpus")),
    );
    push_window(
        &mut windows,
        WindowKind::Weekly,
        "Sonnet week",
        rl.get("seven_day_sonnet")
            .or_else(|| rl.get("sevenDaySonnet")),
    );

    let plan = v
        .get("subscription_type")
        .or_else(|| v.get("subscriptionType"))
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

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
        provider_id: ProviderId::Claude,
        display_name: ProviderId::Claude.display_name().into(),
        windows,
        status,
        source: DataSource::Vendor,
        as_of: now.to_rfc3339(),
        message: plan,
        primary_resets_at,
        primary_used_percent,
    })
}

fn push_window(out: &mut Vec<UsageWindow>, kind: WindowKind, label: &str, obj: Option<&Value>) {
    let Some(obj) = obj else { return };
    if obj.is_null() {
        return;
    }
    let Some(pct) = extract_percent(obj) else {
        return;
    };
    let over = pct > 100.0;
    let used_percent = Some(pct.clamp(0.0, 100.0));
    let resets_at = extract_resets_at(obj);

    out.push(UsageWindow {
        kind,
        used: used_percent.unwrap_or(0.0),
        limit: Some(100.0),
        unit: UsageUnit::Percent,
        resets_at,
        used_percent,
        label: Some(if over {
            format!("{label} · over")
        } else {
            label.into()
        }),
    });
}

fn extract_percent(obj: &Value) -> Option<f64> {
    if let Some(p) = obj
        .get("used_percentage")
        .or_else(|| obj.get("usedPercentage"))
        .and_then(|x| x.as_f64())
    {
        return Some(p);
    }
    // API utilization is typically 0.0–1.0
    if let Some(u) = obj.get("utilization").and_then(|x| x.as_f64()) {
        if (0.0..=1.5).contains(&u) {
            return Some(u * 100.0);
        }
        return Some(u);
    }
    None
}

fn extract_resets_at(obj: &Value) -> Option<String> {
    let v = obj
        .get("resets_at")
        .or_else(|| obj.get("resetsAt"))
        .or_else(|| obj.get("reset_at"))?;
    if let Some(n) = v.as_i64() {
        let secs = if n > 10_000_000_000 { n / 1000 } else { n };
        return Utc
            .timestamp_opt(secs, 0)
            .single()
            .map(|d| d.to_rfc3339());
    }
    if let Some(n) = v.as_f64() {
        let n = n as i64;
        let secs = if n > 10_000_000_000 { n / 1000 } else { n };
        return Utc
            .timestamp_opt(secs, 0)
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
        return Some(s.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_utilization_fraction() {
        let raw = r#"{
          "five_hour": { "utilization": 0.42, "resets_at": 1780000000 },
          "seven_day": { "utilization": 0.15, "resets_at": 1781000000 },
          "subscription_type": "max"
        }"#;
        let snap = parse_oauth_usage(raw).unwrap();
        assert_eq!(snap.source, DataSource::Vendor);
        assert_eq!(snap.windows.len(), 2);
        assert!((snap.windows[0].used_percent.unwrap() - 42.0).abs() < 0.01);
        assert_eq!(snap.windows[0].kind, WindowKind::Rolling5h);
        assert_eq!(snap.windows[1].kind, WindowKind::Weekly);
        assert_eq!(snap.message.as_deref(), Some("max"));
    }

    #[test]
    fn parse_rate_limits_used_percentage() {
        let raw = r#"{
          "rate_limits": {
            "five_hour": { "used_percentage": 40.0, "resets_at": "2026-08-01T12:00:00Z" },
            "seven_day": { "used_percentage": 22.5, "resets_at": "2026-08-05T00:00:00Z" }
          }
        }"#;
        let snap = parse_oauth_usage(raw).unwrap();
        assert_eq!(snap.windows.len(), 2);
        assert!((snap.primary_used_percent.unwrap() - 40.0).abs() < 0.01);
    }

    #[test]
    fn empty_degraded() {
        let snap = parse_oauth_usage(r#"{}"#).unwrap();
        assert_eq!(snap.status, SnapshotStatus::Degraded);
    }
}
