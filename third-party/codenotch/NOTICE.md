# CodeNotch reference

Source: https://github.com/vinzdg/codenotch
Revision: `6e8b0f828741233240d5fb10f7d52cf8eaf6efe4` (reviewed 2026-10-02), `72fb2169ef316834ad632f85de27a415cd0d2298` (reviewed 2026-10-05).

`src-tauri/src/infrastructure/providers/quota/agy_cli.rs` adapts the official
Antigravity CLI discovery, terminal sanitization, Windows argument quoting and
ConPTY transport from `windows/codenotch/src/agy_cli.rs`. Copyright 2026 Vinz;
MIT license included here. TokenUsage adds its own quota mapping, cache and UI.

The 2026-10-05 review brought in:
- Grok auth session filtering (xAI issuer priority) and `grok models` CLI renewal fallback
- Windows topmost band watchdog / HWND_TOPMOST reassertion (`topmost.rs`)
- Codex Desktop SQLite (`thread_turns`) real-time turn activity detection

The edge-fold interaction and independent activity ring are informed by
CodeNotch's UI. Their implementation uses TokenUsage's own native geometry and
input model. Provider artwork attribution is in `src/assets/marks/NOTICE.md`.
