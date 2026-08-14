# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), glass floating widget modeled on EconomyWarRoom.  
**Current ship:** v0.1.31 — release notes: [docs/release.md](./release.md), GitHub [v0.1.31](https://github.com/sky64422/TokenUsage/releases/tag/v0.1.31).

**Product / visual context:** [PRODUCT.md](../PRODUCT.md) (Operate mode), [DESIGN.md](../DESIGN.md) (tokens + contracts).

## Runtime

```
Web UI (provider cards, Quiet Luxury tracks, reset stamp, settings)
        │ invoke / events (snapshots-updated)
        │ set_content_min_size (content-hug min + snap height)
Rust AppCore
        │ refresh_all()  (parallel per provider; spawn_blocking)
1) Direct vendor OAuth  ──► source: vendor
        │ miss / fail
2) Unavailable / AuthRequired card (source: unavailable; no local JSONL / tokscale)
```

## UI layout contracts

- **Two-line card:** name + refill on row 1; period label + capsule track + **%** on row 2. Dual windows stack two blocks (name on the first only).
- **Fixed columns:** metrics `2.2em` (right-aligned period) · `1fr` bar · `2.9em` %; gutters 8px / 2px on `.track`. Text must not shift the rail.
- **Progress (Quiet Luxury):** `.track` / `.track-fill` 6px pill — gradient fill by risk level, soft outer glow; **sheen + critical breathe only when `.is-active`** (live usage). Respect `prefers-reduced-motion`.
- **Reset / meta:** 9px; `formatWindowReset` → `↻ M/D HH:mm` (local); empty when idle / no `resets_at`. Hover title may include token pair + long clock.
- **Opacity:** `applyPanelOpacity` sets `--panel-opacity`, `--fg-opacity`, `--accent-opacity`, `--chrome-opacity` with **readability floors**; semantic tokens use `max(...)` alpha floors (see `tokens.css`).
- **Tokens:** shadcn-inspired semantic names (`--foreground`, `--primary`, `--ring`, …) with Quiet Luxury values; legacy aliases (`--text`, `--accent`, `--ok`) kept for tracks.
- **Height:** frontend measures unconstrained panel height; Rust `snap_height_to_content` sets size to content floor (not grow-only).
- **Settings:** absolute overlay over provider cards (list fades out); window height does **not** grow for the sheet. **Dark-only**. Autostart switch; provider chips (last enabled locked); footer **Copy Log** / **Quit**; meta `{hotkey} · ↻ update`; app version. Refresh interval is **fixed at 5s** (no in-settings control).
- **Focus:** `:focus-visible` + `--ring` on interactive controls.

## Autostart

- Preference persisted in app state; OS login item via `tauri-plugin-autostart`.
- **`enable` only in release builds** (`sync_os_autostart`) so `tauri dev` never registers `target/debug/token-usage.exe` (Vite `devUrl` would fail at boot).
- Disable still runs in debug so a bad Run key can be cleared.

## Providers

### Primary: direct vendor quota (personal OAuth)

Reads local CLI auth only (no in-app login). Always on:

| Id | Auth file | Endpoint |
|----|-----------|----------|
| `claude` | `~/.claude/.credentials.json` (+ OAuth refresh) | `api.anthropic.com/api/oauth/usage` |
| `codex` | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` |
| `grok` | `~/.grok/auth.json` (+ OIDC refresh) | `cli-chat-proxy.grok.com/v1/billing?format=credits` |

Usage/limit HTTP is metadata only (does not consume coding tokens).  
`source: vendor`. Shared 45s body cache + HTTP status mapping (`quota/http.rs`); snapshot primaries in `quota/snapshot.rs`.  
Window labels are short at the adapter (`5h` / `Week` / `Month` / `30d`).  
Env `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` for tests.

#### Grok window mapping

- **Shown:** one primary period window from `creditUsagePercent` (or legacy cents) + period end → typically **Week** (or Monthly/Daily if API says so).
- **Ignored:** `productUsage` array (GrokBuild, GrokChat, …) — same credit pool detail; too noisy for a glance widget. Tests: `ignores_product_usage_breakdown` in `quota/grok.rs`.

### No local JSONL / tokscale

Session log estimates and tokscale were removed.  
If vendor quota misses, the card shows **Unavailable** / **AuthRequired** with a short hint (`source: unavailable`).  
There is no `PlanLimits` / local-event estimate path. Poll interval is `RefreshPolicy::DEFAULT_REFRESH_SECS` (5s); persisted `refresh_secs` is ignored.

## Non-goals (v0.1+)

- Push notifications / tray alerts  
- HTTP scraping of vendor dashboards  
- Local JSONL / plan-limit token estimates  
- tokscale / `npx tokscale` integration  
- Google Antigravity (AGY) in-widget  
- Perfect billing parity with official subscription meters  
- Per-product Grok breakdown (GrokBuild vs GrokChat) in the UI  

## Commands

`get_state`, `get_snapshots`, `refresh_now`, `set_opacity`, `set_autostart`, `set_window_geometry`, `set_provider_enabled`, `set_provider_tint`, `hide_widget`, `quit_app`, `get_diagnostics`, `set_content_min_size`, `check_for_updates`

## Updater

- Plugin: `tauri-plugin-updater`
- Endpoint: GitHub `releases/latest/download/latest.json`
- Startup auto-check in release builds (`infrastructure/updater.rs`)
- Manual: header **↻** → `check_for_updates`
- Publish: `npm run release:publish` — see [release.md](./release.md)

## Hotkey

Default: `Ctrl+Shift+U` (toggle visibility; refresh on show).
