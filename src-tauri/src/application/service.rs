use crate::domain::constants::{clamp_geometry, clamp_opacity, clamp_refresh_secs};
use crate::domain::types::{
    AppSettings, DataSource, DiagnosticsSnapshot, PersistedState, PlanLimits, ProviderConfig,
    ProviderId, ProviderSnapshot, SnapshotStatus, WindowGeometry,
};
use crate::infrastructure::store::save_state;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppCore {
    inner: Mutex<CoreInner>,
}

struct CoreInner {
    state: PersistedState,
    app_data_dir: PathBuf,
    snapshots: HashMap<ProviderId, ProviderSnapshot>,
    visible: bool,
    diag: Vec<String>,
}

impl AppCore {
    pub fn new(state: PersistedState, app_data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(CoreInner {
                state,
                app_data_dir,
                snapshots: HashMap::new(),
                visible: true,
                diag: Vec::new(),
            }),
        })
    }

    pub fn get_state(&self) -> PersistedState {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn get_snapshots(&self) -> Vec<ProviderSnapshot> {
        let guard = self.inner.lock().unwrap();
        let order = ProviderId::all();
        order
            .into_iter()
            .filter_map(|id| guard.snapshots.get(&id).cloned())
            .collect()
    }

    pub fn set_visible(&self, visible: bool) {
        self.inner.lock().unwrap().visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.inner.lock().unwrap().visible
    }

    pub fn refresh_all(&self) -> Vec<ProviderSnapshot> {
        let (settings, app_data_dir) = {
            let guard = self.inner.lock().unwrap();
            (guard.state.settings.clone(), guard.app_data_dir.clone())
        };

        // Direct vendor OAuth quota only (no local JSONL / tokscale).
        let mut next = HashMap::new();
        let mut source_notes: Vec<String> = Vec::new();
        for id in ProviderId::all() {
            let cfg = provider_config(&settings, id);
            if !cfg.enabled {
                continue;
            }

            let mut vendor_err: Option<String> = None;

            // Direct vendor OAuth quota (always on; only data path).
            if let Some(result) = crate::infrastructure::providers::quota::try_fetch(id) {
                match result {
                    Ok(snap) => {
                        source_notes.push(format!("{}:vendor", id.as_str()));
                        next.insert(id, snap);
                        continue;
                    }
                    Err(e) => {
                        let safe = e.replace('\n', " ");
                        let safe: String = safe.chars().take(120).collect();
                        source_notes.push(format!("{}:vendor_fail({})", id.as_str(), safe));
                        vendor_err = Some(safe);
                    }
                }
            }

            // Vendor missed — surface unavailable / auth (no secondary path)
            source_notes.push(format!("{}:unavailable", id.as_str()));
            next.insert(id, missing_quota_snapshot(id, vendor_err.as_deref()));
        }

        let mut guard = self.inner.lock().unwrap();
        let count = next.len();
        guard.snapshots = next;
        let sources = source_notes.join(" ");
        guard.diag.push(format!(
            "{} refreshed {} providers · [{}] (dir={})",
            chrono::Utc::now().to_rfc3339(),
            count,
            sources,
            app_data_dir.display()
        ));
        if guard.diag.len() > 40 {
            let drain = guard.diag.len() - 40;
            guard.diag.drain(0..drain);
        }
        let order = ProviderId::all();
        order
            .into_iter()
            .filter_map(|id| guard.snapshots.get(&id).cloned())
            .collect()
    }

    pub fn persist(&self) -> Result<(), String> {
        let guard = self.inner.lock().unwrap();
        save_state(&guard.app_data_dir, &guard.state)
    }

    pub fn set_opacity(&self, opacity: f64) -> Result<f64, String> {
        let opacity = clamp_opacity(opacity);
        {
            let mut guard = self.inner.lock().unwrap();
            guard.state.settings.opacity = opacity;
        }
        self.persist()?;
        Ok(opacity)
    }

    pub fn set_autostart(&self, enabled: bool) -> Result<(), String> {
        {
            let mut guard = self.inner.lock().unwrap();
            guard.state.settings.autostart = enabled;
        }
        self.persist()
    }

    pub fn set_refresh_secs(&self, secs: u64) -> Result<u64, String> {
        let secs = clamp_refresh_secs(secs);
        {
            let mut guard = self.inner.lock().unwrap();
            guard.state.settings.refresh_secs = secs;
        }
        self.persist()?;
        Ok(secs)
    }

    pub fn set_window_geometry(&self, geometry: WindowGeometry) -> Result<(), String> {
        let geometry = clamp_geometry(&geometry);
        {
            let mut guard = self.inner.lock().unwrap();
            guard.state.settings.window = geometry;
        }
        self.persist()
    }

    pub fn set_provider_enabled(
        &self,
        id: ProviderId,
        enabled: bool,
    ) -> Result<Vec<ProviderSnapshot>, String> {
        {
            let mut guard = self.inner.lock().unwrap();
            // Keep at least one provider visible so the widget never goes blank by accident
            if !enabled {
                let others_on = ProviderId::all().into_iter().any(|other| {
                    other != id && provider_config(&guard.state.settings, other).enabled
                });
                if !others_on {
                    return Err("Keep at least one provider visible".into());
                }
            }
            provider_config_mut(&mut guard.state.settings, id).enabled = enabled;
        }
        self.persist()?;
        Ok(self.refresh_all())
    }

    pub fn set_provider_limits(&self, id: ProviderId, limits: PlanLimits) -> Result<(), String> {
        {
            let mut guard = self.inner.lock().unwrap();
            provider_config_mut(&mut guard.state.settings, id).limits = limits;
        }
        self.persist()?;
        let _ = self.refresh_all();
        Ok(())
    }

    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        let guard = self.inner.lock().unwrap();
        let mut lines = guard.diag.clone();
        for (id, snap) in &guard.snapshots {
            lines.push(format!(
                "{}: status={:?} source={:?} windows={} msg={}",
                id.as_str(),
                snap.status,
                snap.source,
                snap.windows.len(),
                snap.message.clone().unwrap_or_default()
            ));
        }
        DiagnosticsSnapshot { lines }
    }

    pub fn refresh_secs(&self) -> u64 {
        self.inner.lock().unwrap().state.settings.refresh_secs
    }

    pub fn note_diag(&self, message: impl Into<String>) {
        let mut guard = self.inner.lock().unwrap();
        guard.diag.push(message.into());
        if guard.diag.len() > 40 {
            let drain = guard.diag.len() - 40;
            guard.diag.drain(0..drain);
        }
    }
}

fn provider_config(settings: &AppSettings, id: ProviderId) -> ProviderConfig {
    match id {
        ProviderId::Claude => settings.claude.clone(),
        ProviderId::Codex => settings.codex.clone(),
        ProviderId::Grok => settings.grok.clone(),
    }
}

fn provider_config_mut(settings: &mut AppSettings, id: ProviderId) -> &mut ProviderConfig {
    match id {
        ProviderId::Claude => &mut settings.claude,
        ProviderId::Codex => &mut settings.codex,
        ProviderId::Grok => &mut settings.grok,
    }
}

/// When vendor quota misses: show a card, not a local token estimate.
fn missing_quota_snapshot(id: ProviderId, vendor_err: Option<&str>) -> ProviderSnapshot {
    let err = vendor_err.unwrap_or("");
    let auth = err.to_ascii_lowercase().contains("auth")
        || err.to_ascii_lowercase().contains("login")
        || err.to_ascii_lowercase().contains("expired")
        || err.to_ascii_lowercase().contains("credentials");
    let message = if !err.is_empty() {
        err.to_string()
    } else {
        match id {
            ProviderId::Claude => "No Claude quota — login with `claude` CLI".into(),
            ProviderId::Codex => "No Codex quota — login with `codex` CLI".into(),
            ProviderId::Grok => "No Grok quota — login with `grok` CLI".into(),
        }
    };
    ProviderSnapshot {
        provider_id: id,
        display_name: id.display_name().into(),
        windows: vec![],
        status: if auth {
            SnapshotStatus::AuthRequired
        } else {
            SnapshotStatus::Unavailable
        },
        source: DataSource::Manual,
        as_of: Utc::now().to_rfc3339(),
        message: Some(message),
        primary_resets_at: None,
        primary_used_percent: None,
    }
}
