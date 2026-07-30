//! Codex / ChatGPT OAuth from `~/.codex/auth.json` (or `CODEX_HOME`).

use crate::infrastructure::providers::paths::codex_home;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
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
    parse_auth_json(&text)
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

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
}
