//! Grok Build subscription credits via CLI chat proxy billing API.

use crate::domain::types::{ProviderId, ProviderSnapshot, UsageUnit, UsageWindow, WindowKind};
use crate::infrastructure::providers::credentials::grok as grok_creds;
use chrono::{Duration, Utc};
use serde::Deserialize;

use super::grok_fetch;

pub fn fetch() -> Result<ProviderSnapshot, String> {
    if super::http::skip_direct_quota() {
        return Err(super::http::skip_err());
    }

    let mut creds = grok_creds::load()?;
    // Refresh near/after expiry so cold start after reboot still works.
    if grok_creds::needs_refresh(&creds, Duration::minutes(2)) {
        if let (Some(rt), Some(client_id)) =
            (creds.refresh_token.clone(), creds.oidc_client_id.clone())
        {
            match grok_fetch::refresh_access_token(&creds.oidc_issuer, &client_id, &rt) {
                Ok(tok) => {
                    let exp = Utc::now() + Duration::seconds(tok.expires_in.max(60));
                    let new_rt = tok.refresh_token.as_deref().or(Some(rt.as_str()));
                    // Best-effort persist so next boot has a fresh access token
                    let _ = grok_creds::write_refreshed(
                        &creds.path,
                        &creds.storage_key,
                        &tok.access_token,
                        new_rt,
                        exp,
                    );
                    creds.access_token = tok.access_token;
                    if let Some(r) = tok.refresh_token {
                        creds.refresh_token = Some(r);
                    }
                    creds.expires_at = Some(exp);
                }
                Err(e) => {
                    // If already expired hard, surface auth; otherwise try old token
                    if creds
                        .expires_at
                        .map(|e| e <= Utc::now())
                        .unwrap_or(false)
                    {
                        return Err(format!("grok token expired ({e}); run `grok login`"));
                    }
                }
            }
        } else if creds
            .expires_at
            .map(|e| e <= Utc::now())
            .unwrap_or(false)
        {
            return Err("grok token expired; run `grok login`".into());
        }
    }

    let raw = grok_fetch::get_billing_json(
        &creds.access_token,
        creds.user_id.as_deref(),
    )?;
    parse_billing_json(&raw)
}

/// Pure parse of CLI proxy billing JSON (unit-tested).
pub fn parse_billing_json(raw: &str) -> Result<ProviderSnapshot, String> {
    let resp: BillingConfigResponse =
        serde_json::from_str(raw).map_err(|e| format!("grok billing parse: {e}"))?;
    let now = Utc::now();
    let mut windows = Vec::new();

    if let Some(cfg) = resp.config.as_ref() {
        let pct = cfg.credit_usage_percent.or_else(|| {
            // legacy: used/monthly_limit cents
            match (
                cfg.used.as_ref().map(|c| c.val),
                cfg.monthly_limit.as_ref().map(|c| c.val),
            ) {
                (Some(u), Some(lim)) if lim > 0 => Some((u as f64 / lim as f64) * 100.0),
                _ => None,
            }
        });

        let has_period = cfg.current_period.is_some() || cfg.billing_period_end.is_some();
        if pct.is_some() || has_period {
            // Omitted percent at period start (vendor drops default 0) → 0%.
            let raw_pct = pct.unwrap_or(0.0);
            let over = raw_pct > 100.0;
            let used_percent = raw_pct.clamp(0.0, 100.0);
            let (kind, label) = classify_period(cfg.current_period.as_ref());
            let resets_at = cfg
                .current_period
                .as_ref()
                .and_then(|p| p.end.clone())
                .or_else(|| cfg.billing_period_end.clone());

            windows.push(UsageWindow {
                kind,
                used: used_percent,
                limit: Some(100.0),
                unit: UsageUnit::Percent,
                resets_at,
                used_percent: Some(used_percent),
                label: Some(if over {
                    format!("{label} · over")
                } else {
                    label
                }),
            });
        }

        // productUsage (GrokBuild / GrokChat / …) is ignored — same credit pool.
    }

    let plan = resp
        .subscription_tier
        .filter(|s| !s.is_empty());
    let unified = resp
        .config
        .as_ref()
        .and_then(|c| c.is_unified_billing_user)
        .unwrap_or(false);

    let message = if windows.is_empty() && unified {
        Some("unified billing — vendor omitted weekly %".into())
    } else {
        plan
    };

    Ok(super::snapshot::finish_snapshot(
        ProviderId::Grok,
        windows,
        message,
        now,
    ))
}

fn classify_period(period: Option<&UsagePeriod>) -> (WindowKind, String) {
    let Some(p) = period else {
        return (WindowKind::Weekly, "Week".into());
    };
    let t = p.period_type.as_deref().unwrap_or("").to_ascii_uppercase();
    if t.contains("WEEK") {
        (WindowKind::Weekly, "Week".into())
    } else if t.contains("MONTH") {
        (WindowKind::Monthly, "Month".into())
    } else if t.contains("DAY") {
        (WindowKind::Daily, "Daily".into())
    } else {
        (WindowKind::Weekly, "Week".into())
    }
}

#[derive(Debug, Deserialize)]
struct BillingConfigResponse {
    #[serde(default)]
    config: Option<BillingConfig>,
    #[serde(default, rename = "subscriptionTier")]
    subscription_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BillingConfig {
    #[serde(default)]
    credit_usage_percent: Option<f64>,
    #[serde(default)]
    current_period: Option<UsagePeriod>,
    #[serde(default)]
    monthly_limit: Option<Cent>,
    #[serde(default)]
    used: Option<Cent>,
    #[serde(default)]
    billing_period_end: Option<String>,
    #[serde(default)]
    is_unified_billing_user: Option<bool>,
    // productUsage is intentionally ignored (same credit pool, too noisy for UI)
}

#[derive(Debug, Deserialize)]
struct Cent {
    #[serde(default)]
    val: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsagePeriod {
    #[serde(rename = "type", default)]
    period_type: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    start: Option<String>,
    #[serde(default)]
    end: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{DataSource, SnapshotStatus};

    const LIVE_SHAPE: &str = r#"{
      "config": {
        "currentPeriod": {
          "type": "USAGE_PERIOD_TYPE_WEEKLY",
          "start": "2026-07-30T12:36:54.070008+00:00",
          "end": "2026-08-06T12:36:54.070008+00:00"
        },
        "creditUsagePercent": 4.0,
        "onDemandCap": {"val": 0},
        "onDemandUsed": {"val": 0},
        "productUsage": [{"product": "GrokBuild", "usagePercent": 4.0}],
        "isUnifiedBillingUser": true,
        "prepaidBalance": {"val": 0},
        "billingPeriodStart": "2026-07-30T12:36:54.070008+00:00",
        "billingPeriodEnd": "2026-08-06T12:36:54.070008+00:00"
      }
    }"#;

    #[test]
    fn parse_weekly_credits() {
        let snap = parse_billing_json(LIVE_SHAPE).unwrap();
        assert_eq!(snap.provider_id, ProviderId::Grok);
        assert_eq!(snap.source, DataSource::Vendor);
        assert_eq!(snap.windows.len(), 1);
        assert_eq!(snap.windows[0].kind, WindowKind::Weekly);
        assert!((snap.windows[0].used_percent.unwrap() - 4.0).abs() < 0.01);
        assert!(snap.primary_resets_at.as_ref().unwrap().starts_with("2026-08-06"));
    }

    #[test]
    fn ignores_product_usage_breakdown() {
        let raw = r#"{
          "config": {
            "currentPeriod": {
              "type": "USAGE_PERIOD_TYPE_WEEKLY",
              "end": "2026-08-06T12:00:00Z"
            },
            "creditUsagePercent": 12.0,
            "productUsage": [
              {"product": "GrokBuild", "usagePercent": 40.0},
              {"product": "GrokChat", "usagePercent": 8.0}
            ]
          }
        }"#;
        let snap = parse_billing_json(raw).unwrap();
        assert_eq!(snap.windows.len(), 1, "product rows must not become windows");
        assert_eq!(snap.windows[0].kind, WindowKind::Weekly);
        assert!((snap.windows[0].used_percent.unwrap() - 12.0).abs() < 0.01);
        assert!((snap.primary_used_percent.unwrap() - 12.0).abs() < 0.01);
    }

    #[test]
    fn parse_legacy_cents() {
        let raw = r#"{
          "config": {
            "monthlyLimit": {"val": 2000},
            "used": {"val": 500},
            "billingPeriodEnd": "2026-05-01T00:00:00Z"
          },
          "subscriptionTier": "SuperGrok"
        }"#;
        let snap = parse_billing_json(raw).unwrap();
        assert!((snap.primary_used_percent.unwrap() - 25.0).abs() < 0.01);
        assert_eq!(snap.message.as_deref(), Some("SuperGrok"));
    }

    #[test]
    fn empty_config_degraded() {
        let snap = parse_billing_json(r#"{"config":null}"#).unwrap();
        assert_eq!(snap.status, SnapshotStatus::Degraded);
        assert!(snap.windows.is_empty());
    }

    #[test]
    fn unified_billing_omitted_percent_is_zero_when_period_present() {
        // Period rollover: vendor omits creditUsagePercent when usage is 0.
        let raw = r#"{
          "config": {
            "currentPeriod": {
              "type": "USAGE_PERIOD_TYPE_WEEKLY",
              "start": "2026-08-27T12:36:54.070008+00:00",
              "end": "2026-09-03T12:36:54.070008+00:00"
            },
            "isUnifiedBillingUser": true,
            "prepaidBalance": {"val": 0},
            "onDemandCap": {"val": 0},
            "onDemandUsed": {"val": 0},
            "billingPeriodEnd": "2026-09-03T12:36:54.070008+00:00"
          }
        }"#;
        let snap = parse_billing_json(raw).unwrap();
        assert_eq!(snap.status, SnapshotStatus::Ok);
        assert_eq!(snap.source, DataSource::Vendor);
        assert_eq!(snap.windows.len(), 1);
        assert_eq!(snap.windows[0].kind, WindowKind::Weekly);
        assert_eq!(snap.windows[0].label.as_deref(), Some("Week"));
        assert!((snap.windows[0].used_percent.unwrap() - 0.0).abs() < 0.01);
        assert!((snap.primary_used_percent.unwrap() - 0.0).abs() < 0.01);
        assert!(snap
            .primary_resets_at
            .as_ref()
            .unwrap()
            .starts_with("2026-09-03"));
        assert!(snap.message.is_none());
    }

    #[test]
    fn unified_billing_without_percent_or_period_is_degraded_with_hint() {
        let raw = r#"{
          "config": {
            "isUnifiedBillingUser": true,
            "prepaidBalance": {"val": 0}
          }
        }"#;
        let snap = parse_billing_json(raw).unwrap();
        assert_eq!(snap.status, SnapshotStatus::Degraded);
        assert!(snap.windows.is_empty());
        let msg = snap.message.as_deref().unwrap_or("");
        assert!(
            msg.to_ascii_lowercase().contains("unified")
                || msg.to_ascii_lowercase().contains("no weekly"),
            "expected hint, got {msg:?}"
        );
    }
}
