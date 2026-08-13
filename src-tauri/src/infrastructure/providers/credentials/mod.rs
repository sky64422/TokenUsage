//! Read-only loaders for personal CLI OAuth stores (never invent login flows).
//! Claude / Codex / Grok may write back refreshed tokens after OAuth refresh.

pub mod claude;
pub mod codex;
pub mod grok;
