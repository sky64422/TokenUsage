//! Risk scenarios: corrupt state, AppCore settings, provider visibility.

use std::fs;
use std::sync::Once;
use tempfile::tempdir;

static SKIP_NETWORK: Once = Once::new();

fn ensure_skip_network() {
    SKIP_NETWORK.call_once(|| {
        std::env::set_var("TOKENUSAGE_SKIP_DIRECT_QUOTA", "1");
    });
}

use token_usage_lib::application::service::AppCore;
use token_usage_lib::domain::types::{DataSource, PersistedState, ProviderId};
use token_usage_lib::infrastructure::store::{default_state, load_state, save_state, state_path};

#[test]
fn risk_corrupt_state_json_is_err_and_keeps_backup() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let path = state_path(dir.path());
    fs::write(&path, "{ not valid json !!!").unwrap();
    let err = load_state(dir.path()).unwrap_err();
    assert!(err.contains("corrupt"), "{err}");
    assert!(!path.exists());
    let bak = path.with_file_name(format!(
        "{}.corrupt",
        path.file_name().unwrap().to_string_lossy()
    ));
    assert!(bak.exists());
}

#[test]
fn risk_empty_state_file_is_corrupt() {
    let dir = tempdir().unwrap();
    fs::write(state_path(dir.path()), "").unwrap();
    let err = load_state(dir.path()).unwrap_err();
    assert!(err.contains("corrupt"), "{err}");
}

#[test]
fn risk_partial_state_deserializes_with_defaults() {
    let dir = tempdir().unwrap();
    fs::write(
        state_path(dir.path()),
        r#"{
          "version": 1,
          "settings": {
            "opacity": 0.5,
            "window": { "x": 1.0, "y": 2.0, "width": 300.0, "height": 400.0 },
            "hotkey": "Ctrl+Shift+U",
            "autostart": false,
            "refresh_secs": 45
          }
        }"#,
    )
    .unwrap();
    let loaded = load_state(dir.path()).unwrap();
    assert!((loaded.settings.opacity - 0.5).abs() < 0.001);
    assert!(loaded.settings.claude.enabled);
    assert_eq!(loaded.settings.refresh_secs, 45);
    assert_eq!(loaded.settings.notch, Default::default());
    save_state(dir.path(), &loaded).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(state_path(dir.path())).unwrap()).unwrap();
    assert!(saved["settings"].get("window").is_none());
    assert_eq!(load_state(dir.path()).unwrap(), loaded);
}

#[test]
fn risk_legacy_fields_ignored() {
    let dir = tempdir().unwrap();
    fs::write(
        state_path(dir.path()),
        r#"{
          "version": 1,
          "settings": {
            "theme": "system",
            "opacity": 0.9,
            "window": { "x": 1.0, "y": 2.0, "width": 300.0, "height": 400.0 },
            "hotkey": "Ctrl+Shift+U",
            "autostart": true,
            "refresh_secs": 10,
            "use_tokscale": true,
            "use_direct_quota": false,
            "claude": {
              "enabled": true,
              "limits": { "five_hour_tokens": 1, "weekly_tokens": 2 },
              "card_tint": "mint"
            }
          }
        }"#,
    )
    .unwrap();
    let loaded = load_state(dir.path()).unwrap();
    assert!(loaded.settings.autostart);
    assert!((loaded.settings.opacity - 0.9).abs() < 0.001);
    assert_eq!(
        loaded.settings.claude.card_tint,
        token_usage_lib::domain::types::CardTint::Mint
    );
}

#[test]
fn risk_store_round_trip_clamps_opacity() {
    let dir = tempdir().unwrap();
    let mut state = default_state();
    state.settings.opacity = 0.05;
    save_state(dir.path(), &state).unwrap();
    let loaded = load_state(dir.path()).unwrap();
    assert!(loaded.settings.opacity >= 0.35);
}

#[test]
fn risk_appcore_visibility_and_unavailable_without_vendor() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    assert!(core.is_visible());
    core.set_visible(false);
    assert!(!core.is_visible());

    let snaps = core.refresh_all();
    assert!(!snaps.is_empty());
    for s in &snaps {
        assert_eq!(s.source, DataSource::Unavailable);
    }
}

#[test]
fn risk_appcore_disable_provider_excludes_snapshot() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    core.set_provider_enabled(ProviderId::Grok, false).unwrap();
    let snaps = core.refresh_all();
    assert!(snaps.iter().all(|s| s.provider_id != ProviderId::Grok));
    assert!(snaps.iter().any(|s| s.provider_id == ProviderId::Claude));
}

#[test]
fn risk_last_provider_cannot_be_disabled() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    core.set_provider_enabled(ProviderId::Grok, false).unwrap();
    core.set_provider_enabled(ProviderId::Codex, false).unwrap();
    core.set_provider_enabled(ProviderId::Agy, false).unwrap();
    let err = core
        .set_provider_enabled(ProviderId::Claude, false)
        .unwrap_err();
    assert!(err.to_ascii_lowercase().contains("at least one"));
}

#[test]
fn risk_diagnostics_include_refresh_notes() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    let _ = core.refresh_all();
    let diag = core.diagnostics();
    assert!(!diag.lines.is_empty());
    assert!(diag.lines.iter().any(|l| l.contains("refreshed")));
}

#[test]
fn risk_persisted_state_version_round_trip() {
    let dir = tempdir().unwrap();
    let state = PersistedState {
        version: 2,
        settings: default_state().settings,
    };
    save_state(dir.path(), &state).unwrap();
    let loaded = load_state(dir.path()).unwrap();
    assert_eq!(loaded.version, 2);
}

#[test]
fn risk_default_state_uses_notch_placement_without_legacy_window() {
    let state = default_state();
    assert_eq!(state.settings.notch, Default::default());
    let saved = serde_json::to_value(&state).unwrap();
    assert!(saved["settings"].get("window").is_none());
    assert_eq!(
        serde_json::from_value::<PersistedState>(saved).unwrap(),
        state
    );
}
