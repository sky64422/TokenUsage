//! Risk scenarios: corrupt state, tokscale parse edge cases, AppCore settings.

use std::fs;
use std::sync::{Arc, Once};
use tempfile::tempdir;

static SKIP_TOKSCALE: Once = Once::new();

fn ensure_skip_tokscale() {
    SKIP_TOKSCALE.call_once(|| {
        // Avoid spawning npx/tokscale in unit/integration risk tests.
        std::env::set_var("TOKENUSAGE_SKIP_TOKSCALE", "1");
    });
}
use token_usage_lib::application::service::AppCore;
use token_usage_lib::domain::types::{
    DataSource, PersistedState, PlanLimits, ProviderId, ThemeMode,
};
use token_usage_lib::infrastructure::providers::tokscale;
use token_usage_lib::infrastructure::store::{default_state, load_state, save_state, state_path};

#[test]
fn risk_corrupt_state_json_falls_back_to_default() {
    ensure_skip_tokscale();
    let dir = tempdir().unwrap();
    let path = state_path(dir.path());
    fs::write(&path, "{ not valid json !!!").unwrap();
    let loaded = load_state(dir.path());
    assert_eq!(loaded.settings.theme, ThemeMode::System);
    assert!(loaded.settings.use_tokscale);
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
    // Missing use_tokscale / providers → serde defaults
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
    assert!(loaded.settings.use_tokscale); // default true
    assert!(loaded.settings.claude.enabled);
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
fn risk_tokscale_fixture_maps_all_three_providers() {
    let raw = include_str!("fixtures/tokscale_usage.json");
    let map = tokscale::parse_usage_json(raw).expect("fixture parse");
    assert!(map.contains_key(&ProviderId::Claude));
    assert!(map.contains_key(&ProviderId::Codex));
    assert!(map.contains_key(&ProviderId::Grok));

    let claude = map.get(&ProviderId::Claude).unwrap();
    assert_eq!(claude.source, DataSource::Tokscale);
    assert_eq!(claude.windows.len(), 2);
    assert!(claude.primary_resets_at.is_some());
    assert!((claude.primary_used_percent.unwrap() - 40.0).abs() < 0.01);

    let codex = map.get(&ProviderId::Codex).unwrap();
    // 5h + 30d
    assert_eq!(codex.windows.len(), 2);
}

#[test]
fn risk_tokscale_empty_array_errors() {
    let err = tokscale::parse_usage_json("[]").unwrap_err();
    assert!(err.contains("no matching") || err.contains("tokscale"));
}

#[test]
fn risk_tokscale_garbage_errors() {
    assert!(tokscale::parse_usage_json("not-json").is_err());
    assert!(tokscale::parse_usage_json("{\"foo\":1}").is_err());
}

#[test]
fn risk_tokscale_unknown_provider_skipped() {
    let raw = r#"[
      {"provider":"Cursor","plan":"Pro","metrics":[{"label":"Weekly","used_percent":1.0}]},
      {"provider":"Codex","plan":"Free","metrics":[{"label":"5h","used_percent":9.0,"resets_at":"2026-08-01T00:00:00Z"}]}
    ]"#;
    let map = tokscale::parse_usage_json(raw).unwrap();
    assert_eq!(map.len(), 1);
    assert!(map.contains_key(&ProviderId::Codex));
}

#[test]
fn risk_appcore_disable_tokscale_and_visibility() {
    ensure_skip_tokscale();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    assert!(core.is_visible());
    core.set_visible(false);
    assert!(!core.is_visible());

    core.set_use_tokscale(false).unwrap();
    let state = core.get_state();
    assert!(!state.settings.use_tokscale);

    // Local fallback still returns something for enabled providers
    let snaps = core.refresh_all();
    assert!(!snaps.is_empty());
    // After refresh without tokscale, sources should not be tokscale (or may be if cache—fresh process so local)
    for s in &snaps {
        assert_ne!(
            s.source,
            DataSource::Tokscale,
            "expected local path when tokscale disabled"
        );
    }
}

#[test]
fn risk_appcore_provider_limits_persist() {
    ensure_skip_tokscale();
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
    ensure_skip_tokscale();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    core.set_use_tokscale(false).unwrap();
    core.set_provider_enabled(ProviderId::Grok, false).unwrap();
    let snaps = core.refresh_all();
    assert!(snaps.iter().all(|s| s.provider_id != ProviderId::Grok));
    assert!(snaps.iter().any(|s| s.provider_id == ProviderId::Claude));
}

#[test]
fn risk_diagnostics_include_refresh_notes() {
    ensure_skip_tokscale();
    let dir = tempdir().unwrap();
    let core = AppCore::new(default_state(), dir.path().to_path_buf());
    core.set_use_tokscale(false).unwrap();
    let _ = core.refresh_all();
    let diag = core.diagnostics();
    assert!(!diag.lines.is_empty());
    assert!(diag
        .lines
        .iter()
        .any(|l| l.contains("tokscale disabled") || l.contains("refreshed")));
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
