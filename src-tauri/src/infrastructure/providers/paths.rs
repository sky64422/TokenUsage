use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

pub fn claude_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(p));
    }
    home_dir().map(|h| h.join(".claude"))
}

pub fn codex_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CODEX_HOME") {
        return Some(PathBuf::from(p));
    }
    home_dir().map(|h| h.join(".codex"))
}

pub fn grok_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GROK_HOME") {
        return Some(PathBuf::from(p));
    }
    home_dir().map(|h| h.join(".grok"))
}
