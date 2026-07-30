//! Claude Code OAuth from `~/.claude/.credentials.json`.

use crate::infrastructure::providers::paths::claude_home;
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Public Claude Code OAuth client id (embedded in Claude CLI).
pub const DEFAULT_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix ms when access expires (Claude CLI stores ms).
    pub expires_at_ms: Option<i64>,
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
    pub scopes: Vec<String>,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CredFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeAiOauth>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeAiOauth {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: Option<Value>,
    #[serde(default)]
    subscription_type: Option<String>,
    #[serde(default)]
    rate_limit_tier: Option<String>,
    #[serde(default)]
    scopes: Option<Vec<String>>,
}

pub fn load() -> Result<ClaudeCredentials, String> {
    let root = claude_home().ok_or_else(|| "Home directory not found".to_string())?;
    load_from_dir(&root)
}

pub fn load_from_dir(claude_dir: &Path) -> Result<ClaudeCredentials, String> {
    let path = claude_dir.join(".credentials.json");
    if !path.is_file() {
        return Err(format!("Claude credentials not found ({})", path.display()));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read credentials: {e}"))?;
    parse_credentials_json(&text, path)
}

pub fn parse_credentials_json(raw: &str, path: PathBuf) -> Result<ClaudeCredentials, String> {
    let f: CredFile =
        serde_json::from_str(raw).map_err(|e| format!("Claude credentials parse: {e}"))?;
    let oa = f
        .claude_ai_oauth
        .ok_or_else(|| "Claude credentials missing claudeAiOauth".to_string())?;
    let access = oa
        .access_token
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let refresh = oa
        .refresh_token
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if access.is_none() && refresh.is_none() {
        return Err("Claude OAuth empty; run `claude` login".into());
    }

    let expires_at_ms = parse_expires_ms(oa.expires_at.as_ref());

    Ok(ClaudeCredentials {
        access_token: access.unwrap_or_default(),
        refresh_token: refresh,
        expires_at_ms,
        subscription_type: oa.subscription_type.filter(|s| !s.is_empty()),
        rate_limit_tier: oa.rate_limit_tier.filter(|s| !s.is_empty()),
        scopes: oa.scopes.unwrap_or_default(),
        path,
    })
}

fn parse_expires_ms(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    if let Some(n) = v.as_i64() {
        return normalize_expires_ms(n);
    }
    if let Some(n) = v.as_f64() {
        return normalize_expires_ms(n as i64);
    }
    if let Some(s) = v.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Some(dt.timestamp_millis());
        }
    }
    None
}

/// Claude CLI stores `expiresAt` as unix **milliseconds**.
/// Values in the unix-seconds range (~1e9–1e11) are converted to ms.
fn normalize_expires_ms(n: i64) -> Option<i64> {
    if n <= 0 {
        return None;
    }
    // Already ms (e.g. Date.now() ~ 1.7e12)
    if n >= 100_000_000_000 {
        return Some(n);
    }
    // Unix seconds (~1e9–1e11) → ms
    if n >= 1_000_000_000 {
        return Some(n * 1000);
    }
    // Tiny numbers (tests / sentinels): keep as-is
    Some(n)
}

pub fn expires_at_utc(creds: &ClaudeCredentials) -> Option<DateTime<Utc>> {
    let ms = creds.expires_at_ms?;
    Utc.timestamp_millis_opt(ms).single()
}

pub fn needs_refresh(creds: &ClaudeCredentials, skew: chrono::Duration) -> bool {
    if creds.access_token.is_empty() {
        return true;
    }
    match expires_at_utc(creds) {
        None => false,
        Some(exp) => exp <= Utc::now() + skew,
    }
}

/// Atomic update of access/refresh/expiry under `claudeAiOauth`.
pub fn write_refreshed(
    path: &Path,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at_ms: i64,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read credentials: {e}"))?;
    let mut root: Value =
        serde_json::from_str(&text).map_err(|e| format!("credentials parse: {e}"))?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| "credentials root not object".to_string())?;
    let oa = obj
        .entry("claudeAiOauth".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let oa = oa
        .as_object_mut()
        .ok_or_else(|| "claudeAiOauth not object".to_string())?;
    oa.insert("accessToken".into(), Value::String(access_token.into()));
    if let Some(rt) = refresh_token {
        oa.insert("refreshToken".into(), Value::String(rt.into()));
    }
    oa.insert("expiresAt".into(), Value::Number(expires_at_ms.into()));
    let out = serde_json::to_string_pretty(&root).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, out.as_bytes()).map_err(|e| format!("write temp: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace credentials: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_oauth_block() {
        let raw = r#"{
          "claudeAiOauth": {
            "accessToken": "at",
            "refreshToken": "rt",
            "expiresAt": 1780000000000,
            "subscriptionType": "max",
            "rateLimitTier": "default_claude_max_5x",
            "scopes": ["user:inference"]
          }
        }"#;
        let c = parse_credentials_json(raw, PathBuf::from("c")).unwrap();
        assert_eq!(c.access_token, "at");
        assert_eq!(c.refresh_token.as_deref(), Some("rt"));
        assert_eq!(c.expires_at_ms, Some(1780000000000));
        assert_eq!(c.subscription_type.as_deref(), Some("max"));
    }

    #[test]
    fn empty_tokens_error() {
        let raw = r#"{"claudeAiOauth":{"accessToken":"","refreshToken":"","expiresAt":0}}"#;
        assert!(parse_credentials_json(raw, PathBuf::from("c")).is_err());
    }

    #[test]
    fn write_refreshed_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".credentials.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{"claudeAiOauth":{{"accessToken":"old","refreshToken":"r1","expiresAt":1}}}}"#
        )
        .unwrap();
        write_refreshed(&path, "new", Some("r2"), 99).unwrap();
        let c = load_from_dir(dir.path()).unwrap();
        assert_eq!(c.access_token, "new");
        assert_eq!(c.refresh_token.as_deref(), Some("r2"));
        assert_eq!(c.expires_at_ms, Some(99));
    }
}
