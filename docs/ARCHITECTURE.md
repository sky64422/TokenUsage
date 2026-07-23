# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), glass floating widget modeled on EconomyWarRoom.

## Runtime

```
Web UI (progress rows, reset countdown, settings)
        │ invoke / events (snapshots-updated)
Rust AppCore
        │ refresh_all()
Local adapters (Claude / Codex / Grok JSONL + optional Claude rate_limits dump)
```

## Providers

### Primary: tokscale

`tokscale usage --json` (or `npx tokscale usage --json`) once per refresh (45s process cache).  
Maps vendor `used_percent` + `resets_at` → `ProviderSnapshot` (`source: tokscale`).  
Setting: `use_tokscale` (default true).

### Fallback: local JSONL

| Id | Scanner | Windows |
|----|---------|---------|
| `claude` | `~/.claude/projects/**/*.jsonl` usage; optional companion rate_limits JSON | 5h + weekly |
| `codex` | `~/.codex/sessions/**/*.jsonl` `token_count` deltas | 5h + weekly |
| `grok` | `~/.grok/sessions/**/updates.jsonl` `totalTokens` deltas | 5h + weekly |

Local percentages use **user-configured token limits**. Tokscale uses vendor %.

## Non-goals (v0.1)

- Push notifications / tray alerts  
- HTTP scraping of vendor dashboards  
- Google Antigravity (AGY) — future generic CLI adapter  
- Perfect billing parity with official subscription meters  

## Commands

`get_state`, `get_snapshots`, `refresh_now`, `set_theme`, `set_opacity`, `set_autostart`, `set_refresh_secs`, `set_use_tokscale`, `set_window_geometry`, `set_provider_enabled`, `set_provider_limits`, `hide_widget`, `quit_app`, `get_diagnostics`, `set_content_min_size`, `check_for_updates`

## Updater

- Plugin: `tauri-plugin-updater`
- Endpoint: GitHub `releases/latest/download/latest.json`
- Startup auto-check in release builds (`infrastructure/updater.rs`)
- Manual: header **⬆** → `check_for_updates`
- Publish: `npm run release:publish` — see [release.md](./release.md)

## Hotkey

Default: `Ctrl+Shift+U` (toggle visibility; refresh on show).
