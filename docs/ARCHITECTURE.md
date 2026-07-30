# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), glass floating widget modeled on EconomyWarRoom.

## Runtime

```
Web UI (provider cards, fixed-column tracks, reset stamp, settings)
        │ invoke / events (snapshots-updated)
        │ set_content_min_size (content-hug min + snap height)
Rust AppCore
        │ refresh_all()  cascade per provider:
1) Direct vendor OAuth  ──► source: vendor
        │ miss / fail
2) tokscale usage --json  ──► source: tokscale
        │ miss / fail
3) Unavailable / AuthRequired card (no local JSONL estimate)
```

## UI layout contracts

- **Dual windows** (e.g. Claude 5h + Week): CSS grid `1fr 1fr`; each row `label | track | %` with fixed `--win-label-w` / `--win-pct-w`.
- **Single window** (e.g. Codex 30D, Grok Week): full-width track; `%` only in card header.
- **Reset:** `formatWindowReset` → `↻ M/D HH:mm` (local); empty when idle / no `resets_at`.
- **Opacity:** `applyPanelOpacity` sets `--panel-opacity`, `--fg-opacity`, `--accent-opacity`, `--chrome-opacity` so glass, text, and bar fills fade together.
- **Height:** frontend measures unconstrained panel height; Rust `snap_height_to_content` sets size to content floor (not grow-only).

## Providers

### Primary: direct vendor quota (personal OAuth)

Reads local CLI auth only (no in-app login). Cascade when `use_direct_quota` (default true):

| Id | Auth file | Endpoint |
|----|-----------|----------|
| `claude` | `~/.claude/.credentials.json` (+ OAuth refresh) | `api.anthropic.com/api/oauth/usage` |
| `codex` | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` |
| `grok` | `~/.grok/auth.json` (+ OIDC refresh) | `cli-chat-proxy.grok.com/v1/billing?format=credits` |

Usage/limit HTTP is metadata only (does not consume coding tokens).  
`source: vendor`. 45s response cache. Env `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` for tests.

### Secondary: tokscale

`tokscale usage --json` (or `npx` / `npx.cmd` on Windows) once per refresh (45s process cache).  
Maps vendor `used_percent` + `resets_at` → `ProviderSnapshot` (`source: tokscale`).  
Setting: `use_tokscale` (default true). Used when direct vendor misses or fails.

### No local JSONL

Session log estimates (`~/.claude` / `~/.codex` / `~/.grok` token sums) were removed.  
If both vendor and tokscale miss, the card shows **Unavailable** / **AuthRequired** with a short hint.

## Non-goals (v0.1)

- Push notifications / tray alerts  
- HTTP scraping of vendor dashboards  
- Local JSONL / plan-limit token estimates  
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
