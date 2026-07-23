# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), glass floating widget modeled on EconomyWarRoom.

## Runtime

```
Web UI (provider cards, fixed-column tracks, reset stamp, settings)
        │ invoke / events (snapshots-updated)
        │ set_content_min_size (content-hug min + snap height)
Rust AppCore
        │ refresh_all()
tokscale usage --json  ──► map claude/codex/grok
        │ (fallback)
Local adapters (Claude / Codex / Grok JSONL + optional Claude rate_limits dump)
```

## UI layout contracts

- **Dual windows** (e.g. Claude 5h + Week): CSS grid `1fr 1fr`; each row `label | track | %` with fixed `--win-label-w` / `--win-pct-w`.
- **Single window** (e.g. Codex 30D, Grok Week): full-width track; `%` only in card header.
- **Reset:** `formatWindowReset` → `↻ M/D HH:mm` (local); empty when idle / no `resets_at`.
- **Opacity:** `applyPanelOpacity` sets `--panel-opacity`, `--fg-opacity`, `--accent-opacity`, `--chrome-opacity` so glass, text, and bar fills fade together.
- **Height:** frontend measures unconstrained panel height; Rust `snap_height_to_content` sets size to content floor (not grow-only).

## Providers

### Primary: tokscale

`tokscale usage --json` (or `npx` / `npx.cmd` on Windows) once per refresh (45s process cache).  
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
- Google Antigravity (AGY) in-widget — tokscale has `antigravity sync` (macOS/Linux); TokenUsage does not surface it yet  
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
