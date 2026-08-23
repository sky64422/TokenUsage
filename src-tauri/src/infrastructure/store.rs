use crate::domain::constants::clamp_opacity;
use crate::domain::types::PersistedState;
use std::path::{Path, PathBuf};

pub fn default_state() -> PersistedState {
    PersistedState::default()
}

pub fn state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("token-usage-state.json")
}

fn corrupt_backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "token-usage-state.json".into());
    path.with_file_name(format!("{name}.corrupt"))
}

/// Load persisted state.
///
/// Missing file is a first run → `Ok(default_state())`.
/// Unreadable or invalid JSON is **not** treated as defaults: the file is moved
/// aside to `*.json.corrupt` and this returns `Err`.
pub fn load_state(app_data_dir: &Path) -> Result<PersistedState, String> {
    let path = state_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(default_state()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
        Ok(s) => match serde_json::from_str::<PersistedState>(&s) {
            Ok(mut state) => {
                state.settings.opacity = clamp_opacity(state.settings.opacity);
                Ok(state)
            }
            Err(e) => {
                let bak = corrupt_backup_path(&path);
                std::fs::rename(&path, &bak).map_err(|re| {
                    format!("corrupt state ({e}); also failed to move aside: {re}")
                })?;
                Err(format!(
                    "corrupt state JSON ({e}); moved to {}",
                    bak.display()
                ))
            }
        },
    }
}

pub fn save_state(app_data_dir: &Path, state: &PersistedState) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let path = state_path(app_data_dir);
    let mut cloned = state.clone();
    cloned.settings.opacity = clamp_opacity(cloned.settings.opacity);
    let json = serde_json::to_string_pretty(&cloned).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
