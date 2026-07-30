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

/// Test helper: clear process cache.
#[cfg(test)]
#[allow(dead_code)]
pub fn clear_cache() {
    if let Ok(mut g) = CACHE.lock() {
        *g = None;
    }
}
