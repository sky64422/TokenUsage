//! Codex / ChatGPT OAuth from `~/.codex/auth.json` (or `CODEX_HOME`).

use crate::infrastructure::providers::paths::codex_home;
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct AuthFile {
    #[serde(default)]
    tokens: Option<Tokens>,
    /// Some layouts nest under other keys; also accept top-level.
    #[serde(default)]
    access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Tokens {
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
}

pub fn load() -> Result<CodexCredentials, String> {
    let root = codex_home().ok_or_else(|| "Home directory not found".to_string())?;
    load_from_dir(&root)
}

pub fn load_from_dir(codex_dir: &Path) -> Result<CodexCredentials, String> {
    let path = codex_dir.join("auth.json");
    if !path.is_file() {
        return Err(format!("Codex auth not found ({})", path.display()));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read auth.json: {e}"))?;
    let mut creds = parse_auth_json(&text)?;
    creds.path = path;
    Ok(creds)
}

pub fn parse_auth_json(raw: &str) -> Result<CodexCredentials, String> {
    let f: AuthFile = serde_json::from_str(raw).map_err(|e| format!("auth.json parse: {e}"))?;
    let (access, refresh, account) = if let Some(t) = f.tokens {
        (t.access_token, t.refresh_token, t.account_id)
    } else {
        (f.access_token, None, None)
    };
    let access_token = access
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Codex auth.json missing access_token".to_string())?;
    Ok(CodexCredentials {
        access_token,
        refresh_token: refresh.filter(|s| !s.is_empty()),
        account_id: account.filter(|s| !s.is_empty()),
        path: PathBuf::new(),
    })
}

pub fn jwt_exp_unix(access_token: &str) -> Option<i64> {
    let payload = access_token.split('.').nth(1)?;
    let bytes = b64url_decode(payload)?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    v.get("exp").and_then(|x| x.as_i64())
}

pub fn needs_refresh(creds: &CodexCredentials, skew: chrono::Duration) -> bool {
    match jwt_exp_unix(&creds.access_token) {
        None => false,
        Some(exp) => exp <= (Utc::now() + skew).timestamp(),
    }
}

/// Persist refreshed ChatGPT OAuth tokens into `auth.json` (atomic best-effort).
pub fn write_refreshed(
    path: &Path,
    access_token: &str,
    refresh_token: Option<&str>,
    id_token: Option<&str>,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read auth.json: {e}"))?;
    let mut root: Value =
        serde_json::from_str(&text).map_err(|e| format!("auth.json parse: {e}"))?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| "auth.json root not object".to_string())?;
    let tokens = obj
        .entry("tokens".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let tokens = tokens
        .as_object_mut()
        .ok_or_else(|| "tokens not object".to_string())?;
    tokens.insert("access_token".into(), Value::String(access_token.into()));
    if let Some(rt) = refresh_token {
        tokens.insert("refresh_token".into(), Value::String(rt.into()));
    }
    if let Some(id) = id_token {
        tokens.insert("id_token".into(), Value::String(id.into()));
    }
    obj.insert(
        "last_refresh".into(),
        Value::String(Utc::now().to_rfc3339()),
    );
    let out = serde_json::to_string_pretty(&root).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, out.as_bytes()).map_err(|e| format!("write temp auth: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace auth.json: {e}"))?;
    Ok(())
}

fn b64url_decode(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let cleaned: Vec<u8> = input.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cleaned.len() {
        let a = val(cleaned[i])?;
        let b = if i + 1 < cleaned.len() {
            val(cleaned[i + 1])?
        } else {
            0
        };
        let c = if i + 2 < cleaned.len() {
            val(cleaned[i + 2])?
        } else {
            0
        };
        let d = if i + 3 < cleaned.len() {
            val(cleaned[i + 3])?
        } else {
            0
        };
        out.push((a << 2) | (b >> 4));
        if i + 2 < cleaned.len() {
            out.push((b << 4) | (c >> 2));
        }
        if i + 3 < cleaned.len() {
            out.push((c << 6) | d);
        }
        i += 4;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn b64url_encode(data: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        let mut i = 0;
        while i < data.len() {
            let b0 = data[i];
            let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
            let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };
            out.push(T[(b0 >> 2) as usize] as char);
            out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
            if i + 1 < data.len() {
                out.push(T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
            }
            if i + 2 < data.len() {
                out.push(T[(b2 & 63) as usize] as char);
            }
            i += 3;
        }
        out
    }

    #[test]
    fn parse_nested_tokens() {
        let raw = r#"{
          "auth_mode": "chatgpt",
          "tokens": {
            "access_token": "at-xxx",
            "refresh_token": "rt-yyy",
            "account_id": "user-abc"
          }
        }"#;
        let c = parse_auth_json(raw).unwrap();
        assert_eq!(c.access_token, "at-xxx");
        assert_eq!(c.refresh_token.as_deref(), Some("rt-yyy"));
        assert_eq!(c.account_id.as_deref(), Some("user-abc"));
    }

    #[test]
    fn parse_missing_token_errors() {
        assert!(parse_auth_json(r#"{"tokens":{}}"#).is_err());
    }

    #[test]
    fn load_from_dir_reads_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{"tokens":{{"access_token":"tok","account_id":"acc"}}}}"#
        )
        .unwrap();
        let c = load_from_dir(dir.path()).unwrap();
        assert_eq!(c.access_token, "tok");
        assert_eq!(c.account_id.as_deref(), Some("acc"));
    }

    fn jwt_with_exp(exp: i64) -> String {
        let payload = format!(r#"{{"exp":{exp}}}"#);
        format!("aaa.{}.ccc", b64url_encode(payload.as_bytes()))
    }

    #[test]
    fn needs_refresh_when_jwt_expired() {
        let past = Utc::now().timestamp() - 60;
        let creds = CodexCredentials {
            access_token: jwt_with_exp(past),
            refresh_token: Some("rt".into()),
            account_id: None,
            path: PathBuf::from("auth.json"),
        };
        assert!(needs_refresh(&creds, chrono::Duration::minutes(2)));
    }

    #[test]
    fn needs_refresh_false_when_jwt_fresh() {
        let future = Utc::now().timestamp() + 3_600;
        let creds = CodexCredentials {
            access_token: jwt_with_exp(future),
            refresh_token: Some("rt".into()),
            account_id: None,
            path: PathBuf::from("auth.json"),
        };
        assert!(!needs_refresh(&creds, chrono::Duration::minutes(2)));
    }

    #[test]
    fn write_refreshed_updates_tokens_and_last_refresh() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{"last_refresh":"2026-08-02T00:00:00Z","tokens":{{"access_token":"old","refresh_token":"r1","account_id":"acc","id_token":"id1"}}}}"#
        )
        .unwrap();
        write_refreshed(&path, "new-at", Some("r2"), Some("id2")).unwrap();
        let c = load_from_dir(dir.path()).unwrap();
        assert_eq!(c.access_token, "new-at");
        assert_eq!(c.refresh_token.as_deref(), Some("r2"));
        assert_eq!(c.account_id.as_deref(), Some("acc"));
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["tokens"]["id_token"], "id2");
        assert!(raw["last_refresh"].as_str().unwrap().starts_with("20"));
    }
}
