use crate::domain::constants::{
    clamp_opacity, clamp_refresh_secs, HotkeyPolicy, OpacityPolicy, WindowPolicy,
};
use crate::domain::types::{AppSettings, PersistedState, WindowGeometry};
use std::path::{Path, PathBuf};

pub fn default_state() -> PersistedState {
    PersistedState {
        settings: AppSettings {
            opacity: OpacityPolicy::DEFAULT,
            window: WindowGeometry {
                x: 80.0,
                y: 80.0,
                width: WindowPolicy::DEFAULT_WIDTH,
                height: WindowPolicy::DEFAULT_HEIGHT,
            },
            hotkey: HotkeyPolicy::DEFAULT.into(),
            autostart: true,
            refresh_secs: crate::domain::constants::RefreshPolicy::DEFAULT_REFRESH_SECS,
            ..AppSettings::default()
        },
        version: 1,
    }
}

pub fn state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("token-usage-state.json")
}

pub fn load_state(app_data_dir: &Path) -> PersistedState {
    let path = state_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| default_state()),
        Err(_) => default_state(),
    }
}

pub fn save_state(app_data_dir: &Path, state: &PersistedState) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let path = state_path(app_data_dir);
    let mut cloned = state.clone();
    cloned.settings.opacity = clamp_opacity(cloned.settings.opacity);
    cloned.settings.refresh_secs = clamp_refresh_secs(cloned.settings.refresh_secs);
    let json = serde_json::to_string_pretty(&cloned).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
