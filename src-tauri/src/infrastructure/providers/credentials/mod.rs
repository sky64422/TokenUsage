//! Read-only loaders for personal CLI OAuth stores (never invent login flows).
//! Grok may write back refreshed tokens to auth.json after OIDC refresh.

pub mod claude;
pub mod codex;
pub mod grok;
