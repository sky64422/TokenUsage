use chrono::{DateTime, Utc};
use serde::Serialize;

use super::types::ProviderId;

/// Local CLI evidence only; never a quota or a claim about other devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    Unknown,
    Idle,
    Running,
    Recent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderActivity {
    pub provider_id: ProviderId,
    pub state: ActivityState,
    pub observed_at: Option<DateTime<Utc>>,
}
