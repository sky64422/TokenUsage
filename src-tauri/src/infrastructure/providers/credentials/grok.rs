//! Grok Build OAuth from `~/.grok/auth.json` (issuer-keyed map).

use crate::infrastructure::providers::paths::grok_home;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct GrokCredentials {
    /// Map key in auth.json (e.g. `https://auth.x.ai::<client_id>`).
    pub storage_key: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub user_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub oidc_issuer: String,
    pub oidc_client_id: Option<String>,
    pub email: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct GrokAuthEntry {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    oidc_issuer: Option<String>,
    #[serde(default)]
    oidc_client_id: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

pub fn load() -> Result<GrokCredentials, String> {
    let root = grok_home().ok_or_else(|| "Home directory not found".to_string())?;
    load_from_dir(&root)
}

pub fn load_from_dir(grok_dir: &Path) -> Result<GrokCredentials, String> {
    let path = grok_dir.join("auth.json");
    if !path.is_file() {
        return Err(format!("Grok auth not found ({})", path.display()));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read auth.json: {e}"))?;
    parse_auth_json(&text, path)
}

pub fn parse_auth_json(raw: &str, path: PathBuf) -> Result<GrokCredentials, String> {
    let map: Value =
        serde_json::from_str(raw).map_err(|e| format!("Grok auth.json parse: {e}"))?;
    let obj = map
        .as_object()
        .ok_or_else(|| "Grok auth.json must be an object map".to_string())?;
    if obj.is_empty() {
        return Err("Grok auth.json has no accounts".into());
    }

    // Prefer entries with a non-empty access key; first wins.
    for (storage_key, val) in obj {
        let entry: GrokAuthEntry = match serde_json::from_value(val.clone()) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let access = entry
            .key
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let Some(access_token) = access else {
            continue;
        };
        let expires_at = entry
            .expires_at
            .as_deref()
            .and_then(parse_expires_at);
        let issuer = entry
            .oidc_issuer
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "https://auth.x.ai".into());
        return Ok(GrokCredentials {
            storage_key: storage_key.clone(),
            access_token: access_token.to_string(),
            refresh_token: entry.refresh_token.filter(|s| !s.is_empty()),
            user_id: entry.user_id.filter(|s| !s.is_empty()),
            expires_at,
            oidc_issuer: issuer,
            oidc_client_id: entry.oidc_client_id.filter(|s| !s.is_empty()),
            email: entry.email.filter(|s| !s.is_empty()),
            path,
        });
    }
    Err("Grok auth.json has no usable access token".into())
}

fn parse_expires_at(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // Accept fractional without timezone quirks by appending Z if missing
    let normalized = if s.ends_with('Z') || s.contains('+') {
        s.to_string()
    } else {
        format!("{s}Z")
    };
    DateTime::parse_from_rfc3339(&normalized)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// True when access token is missing expiry or expires within `skew`.
pub fn needs_refresh(creds: &GrokCredentials, skew: chrono::Duration) -> bool {
    match creds.expires_at {
        None => false, // unknown expiry — try as-is first
        Some(exp) => exp <= Utc::now() + skew,
    }
}

/// Persist refreshed tokens into auth.json under the same map key (atomic best-effort).
pub fn write_refreshed(
    path: &Path,
    storage_key: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: DateTime<Utc>,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read auth.json: {e}"))?;
    let mut root: Value =
        serde_json::from_str(&text).map_err(|e| format!("auth.json parse: {e}"))?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| "auth.json root not object".to_string())?;
    let entry = obj
        .get_mut(storage_key)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| format!("auth entry missing: {storage_key}"))?;
    entry.insert("key".into(), Value::String(access_token.into()));
    if let Some(rt) = refresh_token {
        entry.insert("refresh_token".into(), Value::String(rt.into()));
    }
    entry.insert(
        "expires_at".into(),
        Value::String(expires_at.to_rfc3339()),
    );
    let out = serde_json::to_string_pretty(&root).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, out.as_bytes()).map_err(|e| format!("write temp auth: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace auth.json: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_issuer_map() {
        let raw = r#"{
          "https://auth.x.ai::client": {
            "key": "atk",
            "refresh_token": "rtk",
            "user_id": "uid-1",
            "expires_at": "2026-08-01T00:00:00Z",
            "oidc_issuer": "https://auth.x.ai",
            "oidc_client_id": "client",
            "email": "a@b.c"
          }
        }"#;
        let c = parse_auth_json(raw, PathBuf::from("auth.json")).unwrap();
        assert_eq!(c.access_token, "atk");
        assert_eq!(c.refresh_token.as_deref(), Some("rtk"));
        assert_eq!(c.user_id.as_deref(), Some("uid-1"));
        assert!(c.expires_at.is_some());
    }

    #[test]
    fn empty_map_errors() {
        assert!(parse_auth_json("{}", PathBuf::from("a")).is_err());
    }

    #[test]
    fn write_refreshed_updates_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{"k":{{"key":"old","refresh_token":"r1","user_id":"u"}}}}"#
        )
        .unwrap();
        let exp = Utc::now() + chrono::Duration::hours(6);
        write_refreshed(&path, "k", "newtok", Some("r2"), exp).unwrap();
        let c = load_from_dir(dir.path()).unwrap();
        assert_eq!(c.access_token, "newtok");
        assert_eq!(c.refresh_token.as_deref(), Some("r2"));
    }
}
