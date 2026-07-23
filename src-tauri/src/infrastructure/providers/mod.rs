pub mod claude;
pub mod codex;
pub mod grok;
pub mod paths;
pub mod tokscale;

use crate::domain::types::{PlanLimits, ProviderId, ProviderSnapshot};

pub fn fetch_provider(id: ProviderId, limits: &PlanLimits) -> ProviderSnapshot {
    match id {
        ProviderId::Claude => claude::fetch(limits),
        ProviderId::Codex => codex::fetch(limits),
        ProviderId::Grok => grok::fetch(limits),
    }
}
