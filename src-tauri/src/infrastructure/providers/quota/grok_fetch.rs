//! HTTP + short cache for Grok billing (excluded from coverage gate).

use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Cache {
    at: Instant,
    body: String,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(45);

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const TOKEN_URL_DEFAULT: &str = "https://auth.x.ai/oauth2/token";

pub fn get_billing_json(
    access_token: &str,
    user_id: Option<&str>,
) -> Result<String, String> {
    if let Ok(guard) = CACHE.lock() {
        if let Some(c) = guard.as_ref() {
            if c.at.elapsed() < CACHE_TTL {
                return Ok(c.body.clone());
            }
        }
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("TokenUsage/0.1 (personal quota; Grok OAuth)")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let mut req = client
        .get(BILLING_URL)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("X-XAI-Token-Auth", "xai-grok-cli")
        .header("Accept", "application/json")
        .header("x-grok-client-version", "0.1")
        .header("x-grok-client-mode", "cli");
    if let Some(uid) = user_id {
        req = req.header("x-userid", uid);
    }

    let resp = req.send().map_err(|e| format!("grok billing request: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| format!("grok billing body: {e}"))?;

    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(format!(
            "grok auth rejected ({status}); run `grok login` again"
        ));
    }
    if !status.is_success() {
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("grok billing HTTP {status}: {snippet}"));
    }
    if body.trim().is_empty() {
        return Err("grok billing empty body".into());
    }

    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(Cache {
            at: Instant::now(),
            body: body.clone(),
        });
    }
    Ok(body)
}

pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
}

/// OIDC refresh_token grant against auth.x.ai (or issuer-derived token URL).
pub fn refresh_access_token(
    issuer: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<RefreshedTokens, String> {
    let token_url = if issuer.trim_end_matches('/').eq_ignore_ascii_case("https://auth.x.ai")
    {
        TOKEN_URL_DEFAULT.to_string()
    } else {
        format!("{}/oauth2/token", issuer.trim_end_matches('/'))
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("TokenUsage/0.1")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .post(&token_url)
        .header("Accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ])
        .send()
        .map_err(|e| format!("grok token refresh: {e}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| format!("grok token body: {e}"))?;
    if !status.is_success() {
        let snippet: String = body.chars().take(120).collect();
        return Err(format!("grok refresh HTTP {status}: {snippet}"));
    }

    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("grok refresh parse: {e}"))?;
    let access = v
        .get("access_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "grok refresh missing access_token".to_string())?
        .to_string();
    let refresh = v
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let expires_in = v
        .get("expires_in")
        .and_then(|x| x.as_i64())
        .unwrap_or(21_600);

    // New tokens invalidate billing cache
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }

    Ok(RefreshedTokens {
        access_token: access,
        refresh_token: refresh,
        expires_in,
    })
}
