# CodeNotch reference

Source: https://github.com/vinzdg/codenotch
Revision: `6e8b0f828741233240d5fb10f7d52cf8eaf6efe4` (reviewed 2026-10-02).

`src-tauri/src/infrastructure/providers/quota/agy_cli.rs` adapts the official
Antigravity CLI discovery, terminal sanitization, Windows argument quoting and
ConPTY transport from `windows/codenotch/src/agy_cli.rs`. Copyright 2026 Vinz;
MIT license included here. TokenUsage adds its own quota mapping, cache and UI.

The edge-fold interaction and independent activity ring are informed by
CodeNotch's UI. Their implementation uses TokenUsage's own native geometry and
input model. Provider artwork attribution is in `src/assets/marks/NOTICE.md`.
