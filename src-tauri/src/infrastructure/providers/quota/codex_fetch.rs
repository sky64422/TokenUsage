//! HTTP + short cache for Codex wham/usage (excluded from coverage gate).

use super::http::{check_usage_http, TtlBodyCache};
use std::time::Duration;

static CACHE: TtlBodyCache = TtlBodyCache::new(45);

pub fn get_usage_json(
    url: &str,
    access_token: &str,
    account_id: Option<&str>,
) -> Result<String, String> {
    if let Some(body) = CACHE.get() {
        return Ok(body);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("TokenUsage/0.1 (personal quota; Codex OAuth)")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let mut req = client
        .get(url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/json");
    if let Some(id) = account_id {
        req = req.header("ChatGPT-Account-ID", id);
    }

    let resp = req
        .send()
        .map_err(|e| format!("codex usage request: {e}"))?;
    let status = resp.status();
    let body = resp.text().map_err(|e| format!("codex usage body: {e}"))?;
    check_usage_http("codex", "run `codex` login again", status, &body)?;
    let body = anchor_relative_resets(&body, chrono::Utc::now())?;
    CACHE.set(body.clone());
    Ok(body)
}

/// Cache an absolute reset once, so a relative countdown cannot drift on replay.
fn anchor_relative_resets(
    raw: &str,
    fetched: chrono::DateTime<chrono::Utc>,
) -> Result<String, String> {
    fn visit(value: &mut serde_json::Value, fetched: chrono::DateTime<chrono::Utc>) {
        match value {
            serde_json::Value::Object(object) => {
                if object
                    .get("reset_at")
                    .is_none_or(serde_json::Value::is_null)
                {
                    if let Some(reset) = object
                        .get("reset_after_seconds")
                        .and_then(serde_json::Value::as_i64)
                        .filter(|after| *after >= 0)
                        .and_then(chrono::Duration::try_seconds)
                        .and_then(|duration| fetched.checked_add_signed(duration))
                    {
                        object.insert("reset_at".into(), reset.to_rfc3339().into());
                    }
                }
                for child in object.values_mut() {
                    visit(child, fetched);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    visit(child, fetched);
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::from_str(raw).map_err(|e| format!("codex usage parse: {e}"))?;
    visit(&mut value, fetched);
    serde_json::to_string(&value).map_err(|e| format!("codex usage parse: {e}"))
}

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
/// Same public client id Codex CLI uses (`app_EMoamEEZ73f0CkXaXp7hrann`).
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
}

pub fn refresh_access_token(refresh_token: &str) -> Result<RefreshedTokens, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("TokenUsage/0.1 (personal quota; Codex OAuth)")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let body = serde_json::json!({
        "client_id": CLIENT_ID,
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
    });

    let resp = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("codex token refresh: {e}"))?;

    let status = resp.status();
    let text = resp.text().map_err(|e| format!("codex token body: {e}"))?;
    if !status.is_success() {
        let snippet: String = text.chars().take(120).collect();
        return Err(format!("codex refresh HTTP {status}: {snippet}"));
    }

    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("codex refresh parse: {e}"))?;
    let access = v
        .get("access_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "codex refresh missing access_token".to_string())?
        .to_string();
    let refresh = v
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let id_token = v
        .get("id_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    CACHE.clear();

    Ok(RefreshedTokens {
        access_token: access,
        refresh_token: refresh,
        id_token,
    })
}

/// Test helper: clear process cache.
#[cfg(test)]
#[allow(dead_code)]
pub fn clear_cache() {
    CACHE.clear();
}

#[cfg(test)]
mod tests {
    use super::anchor_relative_resets;
    #[test]
    fn cached_relative_resets_match_completed_token_worker_periods() {
        use crate::domain::token_usage::{TokenLedger, TokenUsageStatus};
        use chrono::{TimeZone, Utc};
        let fetched = Utc.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let body = anchor_relative_resets(r#"{"rate_limit":{"primary_window":{"used_percent":40,"limit_window_seconds":18000,"reset_after_seconds":3600}}}"#, fetched).unwrap();
        let first = super::super::codex::parse_wham_usage(&body).unwrap();
        let completed = TokenLedger::default().summarize(
            fetched.with_timezone(&chrono::Local),
            TokenUsageStatus::Ready,
            &first.windows,
        );
        // Replaying the cached response after worker completion must keep its reset instant.
        let refreshed = super::super::codex::parse_wham_usage(&body).unwrap();
        assert_eq!(first.windows[0].resets_at, refreshed.windows[0].resets_at);
        assert_eq!(
            chrono::DateTime::parse_from_rfc3339(&completed.windows[0].resets_at).unwrap(),
            fetched + chrono::Duration::hours(1)
        );
        assert_eq!(
            refreshed.windows[0].resets_at.as_deref(),
            Some("2026-10-18T13:00:00+00:00")
        );
    }
}
