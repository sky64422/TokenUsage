//! HTTP + short cache for Claude OAuth usage (excluded from coverage gate).

use super::http::{check_usage_http, TtlBodyCache};
use std::time::Duration;

static CACHE: TtlBodyCache = TtlBodyCache::new(45);

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

pub fn get_usage_json(access_token: &str) -> Result<String, String> {
    if let Some(body) = CACHE.get() {
        return Ok(body);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("claude-cli/token-usage")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .send()
        .map_err(|e| format!("claude usage request: {e}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| format!("claude usage body: {e}"))?;
    check_usage_http("claude", "run `claude` login", status, &body)?;
    CACHE.set(body.clone());
    Ok(body)
}

pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
}

pub fn refresh_access_token(
    client_id: &str,
    refresh_token: &str,
) -> Result<RefreshedTokens, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("claude-cli/token-usage")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": client_id,
    });

    let resp = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("claude token refresh: {e}"))?;

    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("claude token body: {e}"))?;
    if !status.is_success() {
        let snippet: String = text.chars().take(120).collect();
        return Err(format!("claude refresh HTTP {status}: {snippet}"));
    }

    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("claude refresh parse: {e}"))?;
    let access = v
        .get("access_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "claude refresh missing access_token".to_string())?
        .to_string();
    let refresh = v
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let expires_in = v
        .get("expires_in")
        .and_then(|x| x.as_i64())
        .unwrap_or(28_800);

    CACHE.clear();

    Ok(RefreshedTokens {
        access_token: access,
        refresh_token: refresh,
        expires_in,
    })
}
