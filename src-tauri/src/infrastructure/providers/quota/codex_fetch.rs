//! HTTP + short cache for Codex wham/usage (excluded from coverage gate).

use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Cache {
    at: Instant,
    body: String,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(45);

pub fn get_usage_json(
    url: &str,
    access_token: &str,
    account_id: Option<&str>,
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

    let resp = req.send().map_err(|e| format!("codex usage request: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| format!("codex usage body: {e}"))?;

    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(format!(
            "codex auth rejected ({status}); run `codex` login again"
        ));
    }
    if !status.is_success() {
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("codex usage HTTP {status}: {snippet}"));
    }
    if body.trim().is_empty() {
        return Err("codex usage empty body".into());
    }

    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(Cache {
            at: Instant::now(),
            body: body.clone(),
        });
    }
    Ok(body)
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
    let text = resp
        .text()
        .map_err(|e| format!("codex token body: {e}"))?;
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

    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }

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
    if let Ok(mut g) = CACHE.lock() {
        *g = None;
    }
}
