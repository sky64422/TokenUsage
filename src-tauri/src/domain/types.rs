use crate::domain::constants::{HotkeyPolicy, OpacityPolicy, RefreshPolicy, WindowPolicy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Codex,
    Grok,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Codex => "codex",
            ProviderId::Grok => "grok",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderId::Claude => "Claude",
            ProviderId::Codex => "Codex",
            ProviderId::Grok => "Grok",
        }
    }

    pub fn all() -> [ProviderId; 3] {
        [ProviderId::Claude, ProviderId::Codex, ProviderId::Grok]
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
    /// Direct OAuth call to the vendor quota endpoint (personal CLI credentials).
    Vendor,
    /// Vendor miss — no secondary estimate path.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CardTint {
    #[default]
    None,
    Rose,
    Peach,
    Mint,
    Sky,
    Lavender,
    Lemon,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub kind: WindowKind,
    /// Used amount in the unit of this window (tokens, or percent value when unit=percent).
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
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn default_card_tint() -> CardTint {
    CardTint::None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    #[serde(default = "default_card_tint")]
    pub card_tint: CardTint,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            card_tint: CardTint::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub opacity: f64,
    pub window: WindowGeometry,
    pub hotkey: String,
    pub autostart: bool,
    /// Legacy persist field; poll interval is always `RefreshPolicy::DEFAULT_REFRESH_SECS`.
    #[serde(default = "default_refresh_secs")]
    pub refresh_secs: u64,
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub codex: ProviderConfig,
    #[serde(default)]
    pub grok: ProviderConfig,
}

fn default_refresh_secs() -> u64 {
    RefreshPolicy::DEFAULT_REFRESH_SECS
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            opacity: OpacityPolicy::DEFAULT,
            window: WindowGeometry {
                x: 80.0,
                y: 80.0,
                width: WindowPolicy::DEFAULT_WIDTH,
                height: WindowPolicy::DEFAULT_HEIGHT,
            },
            hotkey: HotkeyPolicy::DEFAULT.into(),
            autostart: true,
            refresh_secs: RefreshPolicy::DEFAULT_REFRESH_SECS,
            claude: ProviderConfig::default(),
            codex: ProviderConfig::default(),
            grok: ProviderConfig::default(),
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
