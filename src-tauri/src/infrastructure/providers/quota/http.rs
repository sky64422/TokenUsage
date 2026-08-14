//! Shared skip flag, body cache, and usage-HTTP status mapping.

use reqwest::StatusCode;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn skip_direct_quota() -> bool {
    std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn skip_err() -> String {
    "skipped by TOKENUSAGE_SKIP_DIRECT_QUOTA".into()
}

pub struct TtlBodyCache {
    inner: Mutex<Option<(Instant, String)>>,
    ttl: Duration,
}

impl TtlBodyCache {
    pub const fn new(ttl_secs: u64) -> Self {
        Self {
            inner: Mutex::new(None),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn get(&self) -> Option<String> {
        let guard = self.inner.lock().ok()?;
        let (at, body) = guard.as_ref()?;
        if at.elapsed() < self.ttl {
            Some(body.clone())
        } else {
            None
        }
    }

    pub fn set(&self, body: String) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some((Instant::now(), body));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }
}

/// Map vendor usage HTTP status/body to a short UI-safe error.
pub fn check_usage_http(
    provider: &str,
    login_hint: &str,
    status: StatusCode,
    body: &str,
) -> Result<(), String> {
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(format!(
            "{provider} auth rejected ({status}); {login_hint}"
        ));
    }
    if status.as_u16() == 429 {
        return Err(format!("{provider} usage rate limited (retry later)"));
    }
    if !status.is_success() {
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("{provider} usage HTTP {status}: {snippet}"));
    }
    if body.trim().is_empty() {
        return Err(format!("{provider} usage empty body"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_round_trip_and_clear() {
        let cache = TtlBodyCache::new(60);
        assert!(cache.get().is_none());
        cache.set("body".into());
        assert_eq!(cache.get().as_deref(), Some("body"));
        cache.clear();
        assert!(cache.get().is_none());
    }

    #[test]
    fn cache_expires() {
        let cache = TtlBodyCache::new(0);
        cache.set("stale".into());
        assert!(cache.get().is_none());
    }

    #[test]
    fn maps_auth_and_empty() {
        let err = check_usage_http(
            "claude",
            "run `claude` login",
            StatusCode::UNAUTHORIZED,
            "",
        )
        .unwrap_err();
        assert!(err.contains("auth rejected"));
        assert!(check_usage_http("grok", "login", StatusCode::OK, "  ").is_err());
        assert!(check_usage_http("codex", "login", StatusCode::OK, "{}").is_ok());
        assert!(check_usage_http("claude", "login", StatusCode::TOO_MANY_REQUESTS, "").is_err());
    }
}
