use crate::domain::constants::clamp_opacity;
use crate::domain::types::{
    AppSettings, CardTint, DataSource, DiagnosticsSnapshot, PersistedState, ProviderConfig,
    ProviderId, ProviderSnapshot, SnapshotStatus,
};
use crate::infrastructure::store::save_state;
use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

const DIAG_CAP: usize = 40;

pub struct AppCore {
    inner: Mutex<CoreInner>,
}

struct CoreInner {
    state: PersistedState,
    app_data_dir: PathBuf,
    snapshots: HashMap<ProviderId, ProviderSnapshot>,
    visible: bool,
    diag: VecDeque<String>,
}

impl AppCore {
    pub fn new(state: PersistedState, app_data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(CoreInner {
                state,
                app_data_dir,
                snapshots: HashMap::new(),
                visible: true,
                diag: VecDeque::new(),
            }),
        })
    }

    fn lock(&self) -> MutexGuard<'_, CoreInner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn get_state(&self) -> PersistedState {
        self.lock().state.clone()
    }

    pub fn get_snapshots(&self) -> Vec<ProviderSnapshot> {
        ordered_snapshots(&self.lock().snapshots)
    }

    pub fn set_visible(&self, visible: bool) {
        self.lock().visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.lock().visible
    }

    pub fn refresh_all(&self) -> Vec<ProviderSnapshot> {
        let (enabled, app_data_dir) = {
            let guard = self.lock();
            let enabled: Vec<ProviderId> = ProviderId::all()
                .into_iter()
                .filter(|id| provider_config(&guard.state.settings, *id).enabled)
                .collect();
            (enabled, guard.app_data_dir.clone())
        };

        let fetched: Vec<(ProviderId, Result<ProviderSnapshot, String>)> =
            std::thread::scope(|scope| {
                let handles: Vec<_> = enabled
                    .iter()
                    .copied()
                    .map(|id| {
                        scope.spawn(move || {
                            (
                                id,
                                crate::infrastructure::providers::quota::fetch(id),
                            )
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .zip(enabled.iter().copied())
                    .map(|(h, id)| {
                        h.join()
                            .unwrap_or_else(|_| (id, Err("refresh panicked".into())))
                    })
                    .collect()
            });

        let mut next = HashMap::new();
        let mut source_notes: Vec<String> = Vec::new();
        for (id, result) in fetched {
            match result {
                Ok(snap) => {
                    source_notes.push(format!("{}:vendor", id.as_str()));
                    next.insert(id, snap);
                }
                Err(e) => {
                    let safe = sanitize_error_message(&e);
                    source_notes.push(format!("{}:vendor_fail({})", id.as_str(), safe));
                    source_notes.push(format!("{}:unavailable", id.as_str()));
                    next.insert(id, missing_quota_snapshot(id, Some(safe.as_str())));
                }
            }
        }

        let mut guard = self.lock();
        let count = next.len();
        guard.snapshots = next;
        push_diag(
            &mut guard.diag,
            format!(
                "{} refreshed {} providers · [{}] (dir={})",
                Utc::now().to_rfc3339(),
                count,
                source_notes.join(" "),
                app_data_dir.display()
            ),
        );
        ordered_snapshots(&guard.snapshots)
    }

    pub fn persist(&self) -> Result<(), String> {
        let (dir, state) = {
            let guard = self.lock();
            (guard.app_data_dir.clone(), guard.state.clone())
        };
        save_state(&dir, &state)
    }

    fn mutate_settings<T>(
        &self,
        f: impl FnOnce(&mut AppSettings) -> T,
    ) -> Result<T, String> {
        let out = {
            let mut guard = self.lock();
            f(&mut guard.state.settings)
        };
        self.persist()?;
        Ok(out)
    }

    pub fn set_opacity(&self, opacity: f64) -> Result<f64, String> {
        let opacity = clamp_opacity(opacity);
        self.mutate_settings(|s| {
            s.opacity = opacity;
            opacity
        })
    }

    pub fn set_autostart(&self, enabled: bool) -> Result<(), String> {
        self.mutate_settings(|s| {
            s.autostart = enabled;
        })
    }

    pub fn set_hover_detail(&self, enabled: bool) -> Result<(), String> {
        self.mutate_settings(|s| {
            s.hover_detail = enabled;
        })
    }

    pub fn set_always_show_notch(&self, enabled: bool) -> Result<(), String> {
        self.mutate_settings(|s| {
            s.always_show_notch = enabled;
        })
    }

    pub fn set_show_orbit(&self, enabled: bool) -> Result<(), String> {
        self.mutate_settings(|s| {
            s.show_orbit = enabled;
        })
    }

    pub fn set_show_icon_glow(&self, enabled: bool) -> Result<(), String> {
        self.mutate_settings(|s| {
            s.show_icon_glow = enabled;
        })
    }

    pub fn set_notch_placement(
        &self,
        placement: crate::domain::notch::NotchPlacement,
    ) -> Result<(), String> {
        placement.validate()?;
        // A failed write must not turn a transient drag into committed state.
        let mut guard = self.lock();
        let mut next = guard.state.clone();
        next.settings.notch = placement;
        save_state(&guard.app_data_dir, &next)?;
        guard.state = next;
        Ok(())
    }

    pub fn set_provider_enabled(
        &self,
        id: ProviderId,
        enabled: bool,
    ) -> Result<Vec<ProviderSnapshot>, String> {
        {
            let mut guard = self.lock();
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

    pub fn set_provider_tint(&self, id: ProviderId, tint: CardTint) -> Result<(), String> {
        self.mutate_settings(|s| {
            provider_config_mut(s, id).card_tint = tint;
        })
    }

    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        let guard = self.lock();
        let mut lines: Vec<String> = guard.diag.iter().cloned().collect();
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

    pub fn note_diag(&self, message: impl Into<String>) {
        let mut guard = self.lock();
        push_diag(&mut guard.diag, message.into());
    }
}

fn sanitize_error_message(err: &str) -> String {
    let safe = err.replace('\n', " ");
    safe.chars().take(120).collect()
}

fn push_diag(diag: &mut VecDeque<String>, message: String) {
    if diag.len() >= DIAG_CAP {
        diag.pop_front();
    }
    diag.push_back(message);
}

fn ordered_snapshots(map: &HashMap<ProviderId, ProviderSnapshot>) -> Vec<ProviderSnapshot> {
    ProviderId::all()
        .into_iter()
        .filter_map(|id| map.get(&id).cloned())
        .collect()
}

fn provider_config(settings: &AppSettings, id: ProviderId) -> &ProviderConfig {
    match id {
        ProviderId::Claude => &settings.claude,
        ProviderId::Codex => &settings.codex,
        ProviderId::Grok => &settings.grok,
        ProviderId::Agy => &settings.agy,
    }
}

fn provider_config_mut(settings: &mut AppSettings, id: ProviderId) -> &mut ProviderConfig {
    match id {
        ProviderId::Claude => &mut settings.claude,
        ProviderId::Codex => &mut settings.codex,
        ProviderId::Grok => &mut settings.grok,
        ProviderId::Agy => &mut settings.agy,
    }
}

/// When vendor quota misses: show a card, not a local token estimate.
fn missing_quota_snapshot(id: ProviderId, vendor_err: Option<&str>) -> ProviderSnapshot {
    let err = vendor_err.unwrap_or("");
    let lower = err.to_ascii_lowercase();
    let auth = lower.contains("auth")
        || lower.contains("login")
        || lower.contains("expired")
        || lower.contains("credentials");
    let message = if !err.is_empty() {
        err.to_string()
    } else {
        match id {
            ProviderId::Claude => "No Claude quota — login with `claude` CLI".into(),
            ProviderId::Codex => "No Codex quota — login with `codex` CLI".into(),
            ProviderId::Agy => "No Antigravity quota - sign in with `agy`".into(),
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
        source: DataSource::Unavailable,
        as_of: Utc::now().to_rfc3339(),
        message: Some(message),
        primary_resets_at: None,
        primary_used_percent: None,
    }
}
