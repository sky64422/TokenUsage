pub mod agy;
mod agy_cli;
// Direct vendor quota (OAuth endpoints and official CLI).

pub mod claude;
pub mod claude_fetch;
pub mod codex;
pub mod codex_fetch;
pub mod grok;
pub mod grok_fetch;
pub mod http;
pub mod snapshot;

use crate::domain::types::{ProviderId, ProviderSnapshot};

/// Fetch personal direct quota for a provider.
pub fn fetch(id: ProviderId) -> Result<ProviderSnapshot, String> {
    match id {
        ProviderId::Claude => claude::fetch(),
        ProviderId::Codex => codex::fetch(),
        ProviderId::Grok => grok::fetch(),
        ProviderId::Agy => agy::fetch(),
    }
}
