//! Risk scenarios: corrupt state, AppCore settings, provider visibility.

use std::fs;
use std::sync::{Arc, Once};
use tempfile::tempdir;

static SKIP_NETWORK: Once = Once::new();

fn ensure_skip_network() {
    SKIP_NETWORK.call_once(|| {
        // Avoid hitting vendor HTTP in risk tests.
        std::env::set_var("TOKENUSAGE_SKIP_DIRECT_QUOTA", "1");
    });
}

use token_usage_lib::application::service::AppCore;
use token_usage_lib::domain::types::{
    DataSource, PersistedState, PlanLimits, ProviderId, ThemeMode,
};
use token_usage_lib::infrastructure::store::{default_state, load_state, save_state, state_path};

#[test]
fn risk_corrupt_state_json_falls_back_to_default() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let path = state_path(dir.path());
    fs::write(&path, "{ not valid json !!!").unwrap();
    let loaded = load_state(dir.path());
    assert_eq!(loaded.settings.theme, ThemeMode::System);
    assert!(loaded.settings.autostart);
}

#[test]
fn risk_empty_state_file_falls_back() {
    let dir = tempdir().unwrap();
    fs::write(state_path(dir.path()), "").unwrap();
    let loaded = load_state(dir.path());
    assert!(loaded.settings.refresh_secs >= 5);
}

#[test]
fn risk_partial_state_deserializes_with_defaults() {
    let dir = tempdir().unwrap();
    // Missing optional fields / providers → serde defaults
    fs::write(
        state_path(dir.path()),
        r#"{
          "version": 1,
          "settings": {
            "theme": "dark",
            "opacity": 0.5,
            "window": { "x": 1.0, "y": 2.0, "width": 300.0, "height": 400.0 },
            "hotkey": "Ctrl+Shift+U",
            "autostart": false,
            "refresh_secs": 45
          }
        }"#,
    )
    .unwrap();
    let loaded = load_state(dir.path());
    assert_eq!(loaded.settings.theme, ThemeMode::Dark);
    assert!((loaded.settings.opacity - 0.5).abs() < 0.001);
    assert!(loaded.settings.claude.enabled);
}

#[test]
fn risk_legacy_quota_toggles_ignored() {
    let dir = tempdir().unwrap();
    // Older installs may still have use_tokscale / use_direct_quota — ignore unknown fields.
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
            "use_direct_quota": false
          }
        }"#,
    )
    .unwrap();
    let loaded = load_state(dir.path());
    assert!(loaded.settings.autostart);
    assert!((loaded.settings.opacity - 0.9).abs() < 0.001);
}

#[test]
fn risk_store_round_trip_clamps_opacity_and_refresh() {
    let dir = tempdir().unwrap();
    let mut state = default_state();
    state.settings.opacity = 0.05; // below min
    state.settings.refresh_secs = 1; // below min
    save_state(dir.path(), &state).unwrap();
    let loaded = load_state(dir.path());
    assert!(loaded.settings.opacity >= 0.35);
    assert!(loaded.settings.refresh_secs >= 5);
}

#[test]
fn risk_appcore_visibility_and_unavailable_without_vendor() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    assert!(core.is_visible());
    core.set_visible(false);
    assert!(!core.is_visible());

    // Without direct vendor, cards are unavailable — not local estimates
    let snaps = core.refresh_all();
    assert!(!snaps.is_empty());
    for s in &snaps {
        assert_ne!(s.source, DataSource::Estimate);
        assert_ne!(s.source, DataSource::LocalFile);
    }
}

#[test]
fn risk_appcore_provider_limits_persist() {
    ensure_skip_network();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    let limits = PlanLimits {
        five_hour_tokens: 12_345.0,
        weekly_tokens: Some(99_000.0),
    };
    core.set_provider_limits(ProviderId::Claude, limits.clone())
        .unwrap();
    let state = core.get_state();
    assert!((state.settings.claude.limits.five_hour_tokens - 12_345.0).abs() < 0.1);
    assert_eq!(state.settings.claude.limits.weekly_tokens, Some(99_000.0));

    // Reload from disk
    let reloaded = load_state(dir.path());
    assert!((reloaded.settings.claude.limits.five_hour_tokens - 12_345.0).abs() < 0.1);
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
    let loaded = load_state(dir.path());
    assert_eq!(loaded.version, 2);
}

// Silence unused import if Arc only needed in some cfgs
#[allow(dead_code)]
fn _arc_typecheck() {
    let _: Option<Arc<()>> = None;
}
