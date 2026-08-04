# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), glass floating widget modeled on EconomyWarRoom.  
**Current ship:** v0.1.20 — release notes: [docs/release.md](./release.md), GitHub [v0.1.20](https://github.com/sky64422/TokenUsage/releases/tag/v0.1.20).

## Runtime

```
Web UI (provider cards, Quiet Luxury tracks, reset stamp, settings)
        │ invoke / events (snapshots-updated)
        │ set_content_min_size (content-hug min + snap height)
Rust AppCore
        │ refresh_all()  per provider:
1) Direct vendor OAuth  ──► source: vendor
        │ miss / fail
2) Unavailable / AuthRequired card (no local JSONL / tokscale)
```

## UI layout contracts

- **Dual windows** (e.g. Claude/Codex 5h + Week): CSS grid `1fr 1fr`; each cell is `label | track` with shared `--win-label-min` (4ch). **% is only in the card header** (`a% / b%`, per-leg level color).
- **Single window** (e.g. Codex 30D, Grok Week): full-width track; `%` only in card header.
- **Progress (Quiet Luxury):** `.track` / `.track-fill` pill — gradient fill by risk level, soft outer glow, slow diagonal sheen, partial-fill luminous end-cap; critical/over soft breathe. Respect `prefers-reduced-motion`.
- **Reset:** `formatWindowReset` → `↻ M/D HH:mm` (local); empty when idle / no `resets_at`. Hover title may include token pair + long clock.
- **Opacity:** `applyPanelOpacity` sets `--panel-opacity`, `--fg-opacity`, `--accent-opacity`, `--chrome-opacity` so glass, text, and bar fills fade together.
- **Height:** frontend measures unconstrained panel height; Rust `snap_height_to_content` sets size to content floor (not grow-only).
- **Settings:** absolute overlay over provider cards (list fades out); window height does **not** grow for the sheet. **Dark-only** (no theme switch). Providers as horizontal on/off chips; footer **Copy Log** / **Quit** (equal half-width).

## Providers

### Primary: direct vendor quota (personal OAuth)

Reads local CLI auth only (no in-app login). Always on:

| Id | Auth file | Endpoint |
|----|-----------|----------|
| `claude` | `~/.claude/.credentials.json` (+ OAuth refresh) | `api.anthropic.com/api/oauth/usage` |
| `codex` | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` |
| `grok` | `~/.grok/auth.json` (+ OIDC refresh) | `cli-chat-proxy.grok.com/v1/billing?format=credits` |

Usage/limit HTTP is metadata only (does not consume coding tokens).  
`source: vendor`. 45s response cache. Env `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` for tests.

#### Grok window mapping

- **Shown:** one primary period window from `creditUsagePercent` (or legacy cents) + period end → typically **Week** (or Monthly/Daily if API says so).
- **Ignored:** `productUsage` array (GrokBuild, GrokChat, …) — same credit pool detail; too noisy for a glance widget. Tests: `ignores_product_usage_breakdown` in `quota/grok.rs`.

### No local JSONL / tokscale

Session log estimates and tokscale were removed.  
If vendor quota misses, the card shows **Unavailable** / **AuthRequired** with a short hint.

## Non-goals (v0.1+)

- Push notifications / tray alerts  
- HTTP scraping of vendor dashboards  
- Local JSONL / plan-limit token estimates  
- tokscale / `npx tokscale` integration  
- Google Antigravity (AGY) in-widget  
- Perfect billing parity with official subscription meters  
- Per-product Grok breakdown (GrokBuild vs GrokChat) in the UI  

## Commands

`get_state`, `get_snapshots`, `refresh_now`, `set_opacity`, `set_autostart`, `set_refresh_secs`, `set_window_geometry`, `set_provider_enabled`, `set_provider_limits`, `hide_widget`, `quit_app`, `get_diagnostics`, `set_content_min_size`, `check_for_updates`

## Updater

- Plugin: `tauri-plugin-updater`
- Endpoint: GitHub `releases/latest/download/latest.json`
- Startup auto-check in release builds (`infrastructure/updater.rs`)
- Manual: header **⬆** → `check_for_updates`
- Publish: `npm run release:publish` — see [release.md](./release.md)

## Hotkey

Default: `Ctrl+Shift+U` (toggle visibility; refresh on show).
