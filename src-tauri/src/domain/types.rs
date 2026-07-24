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
    Session,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageUnit {
    Percent,
    Tokens,
    Messages,
    Credits,
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
    LocalFile,
    Cli,
    Manual,
    Estimate,
    /// `tokscale usage --json` vendor-reported quotas.
    Tokscale,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLimits {
    /// Token budget for the primary 5-hour window (local estimate).
    pub five_hour_tokens: f64,
    /// Optional weekly token budget.
    pub weekly_tokens: Option<f64>,
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            five_hour_tokens: 88_000.0, // Max5-ish default
            weekly_tokens: Some(500_000.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub limits: PlanLimits,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            limits: PlanLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub opacity: f64,
    pub window: WindowGeometry,
    pub hotkey: String,
    pub autostart: bool,
    /// Seconds between usage refresh (clamped on write).
    #[serde(default = "default_refresh_secs")]
    pub refresh_secs: u64,
    /// Prefer `tokscale usage --json` (vendor quotas); local JSONL is fallback.
    #[serde(default = "default_use_tokscale")]
    pub use_tokscale: bool,
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub codex: ProviderConfig,
    #[serde(default)]
    pub grok: ProviderConfig,
}

fn default_refresh_secs() -> u64 {
    5
}

fn default_use_tokscale() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            opacity: 0.92,
            window: WindowGeometry {
                x: 80.0,
                y: 80.0,
                width: 340.0,
                height: 420.0,
            },
            hotkey: "Ctrl+Shift+U".into(),
            autostart: true,
            refresh_secs: 5,
            use_tokscale: true,
            claude: ProviderConfig {
                enabled: true,
                limits: PlanLimits {
                    five_hour_tokens: 88_000.0,
                    weekly_tokens: Some(500_000.0),
                },
            },
            codex: ProviderConfig {
                enabled: true,
                limits: PlanLimits {
                    five_hour_tokens: 200_000.0,
                    weekly_tokens: Some(1_000_000.0),
                },
            },
            grok: ProviderConfig {
                enabled: true,
                limits: PlanLimits {
                    five_hour_tokens: 200_000.0,
                    weekly_tokens: Some(1_000_000.0),
                },
            },
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
