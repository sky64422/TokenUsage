//! Direct vendor quota HTTP (personal CLI OAuth).

pub mod claude;
pub mod claude_fetch;
pub mod codex;
pub mod codex_fetch;
pub mod grok;
pub mod grok_fetch;

use crate::domain::types::{ProviderId, ProviderSnapshot};

/// Try personal direct quota for a provider.
pub fn try_fetch(id: ProviderId) -> Option<Result<ProviderSnapshot, String>> {
    match id {
        ProviderId::Claude => Some(claude::fetch()),
        ProviderId::Codex => Some(codex::fetch()),
        ProviderId::Grok => Some(grok::fetch()),
    }
}
