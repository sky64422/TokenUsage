use crate::domain::constants::OpacityPolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Codex,
    Grok,
    Agy,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Codex => "codex",
            ProviderId::Grok => "grok",
            ProviderId::Agy => "agy",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderId::Claude => "Claude",
            ProviderId::Codex => "Codex",
            ProviderId::Grok => "Grok",
            ProviderId::Agy => "Antigravity",
        }
    }

    pub fn all() -> [ProviderId; 4] {
        [ProviderId::Claude, ProviderId::Codex, ProviderId::Grok, ProviderId::Agy]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    Rolling5h,
    Weekly,
    Daily,
    Monthly,
    Session,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageUnit {
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    Ok,
    Degraded,
    Unavailable,
    AuthRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// Vendor quota from OAuth endpoints or the official Antigravity CLI.
    Vendor,
    /// Vendor miss — no secondary estimate path.
    Unavailable,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub kind: WindowKind,
    /// Used percentage reported by the vendor.
    pub used: f64,
    pub limit: Option<f64>,
    pub unit: UsageUnit,
    /// ISO-8601 UTC when this window resets, if known.
    pub resets_at: Option<String>,
    /// 0.0–100.0 when computable.
    pub used_percent: Option<f64>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderSnapshot {
    pub provider_id: ProviderId,
    pub display_name: String,
    pub windows: Vec<UsageWindow>,
    pub status: SnapshotStatus,
    pub source: DataSource,
    pub as_of: String,
    pub message: Option<String>,
    /// Primary (most urgent) reset ISO time for glanceable UI.
    pub primary_resets_at: Option<String>,
    pub primary_used_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub notch: super::notch::NotchPlacement,
    pub opacity: f64,
    pub autostart: bool,
    #[serde(default)]
    pub hover_detail: bool,
    #[serde(default)]
    pub always_show_notch: bool,
    #[serde(default = "default_true")]
    pub show_orbit: bool,
    #[serde(default = "default_true")]
    pub show_icon_glow: bool,
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub codex: ProviderConfig,
    #[serde(default)]
    pub grok: ProviderConfig,
    #[serde(default)]
    pub agy: ProviderConfig,
}

fn default_true() -> bool {
    true
}


impl Default for AppSettings {
    fn default() -> Self {
        Self {
            notch: super::notch::NotchPlacement::default(),
            opacity: OpacityPolicy::DEFAULT,
            autostart: true,
            hover_detail: false,
            always_show_notch: false,
            show_orbit: true,
            show_icon_glow: true,
            claude: ProviderConfig::default(),
            codex: ProviderConfig::default(),
            grok: ProviderConfig::default(),
            agy: ProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub settings: AppSettings,
    #[serde(default)]
    pub version: u32,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            settings: AppSettings::default(),
            version: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsSnapshot {
    pub lines: Vec<String>,
}

impl AppSettings {
    pub fn provider_config(&self, id: ProviderId) -> &ProviderConfig {
        match id {
            ProviderId::Claude => &self.claude,
            ProviderId::Codex => &self.codex,
            ProviderId::Grok => &self.grok,
            ProviderId::Agy => &self.agy,
        }
    }

    pub fn provider_config_mut(&mut self, id: ProviderId) -> &mut ProviderConfig {
        match id {
            ProviderId::Claude => &mut self.claude,
            ProviderId::Codex => &mut self.codex,
            ProviderId::Grok => &mut self.grok,
            ProviderId::Agy => &mut self.agy,
        }
    }

    pub fn enabled_provider_ids(&self) -> Vec<ProviderId> {
        ProviderId::all()
            .into_iter()
            .filter(|id| self.provider_config(*id).enabled)
            .collect()
    }
}
