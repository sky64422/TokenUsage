# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), screen-edge notch with inward quota details.
**Current ship:** v0.1.34 — release notes: [docs/release.md](./release.md), GitHub [v0.1.34](https://github.com/sky64422/TokenUsage/releases/tag/v0.1.34).

**Product / visual context:** [PRODUCT.md](../PRODUCT.md) (Operate mode), [DESIGN.md](../DESIGN.md) (tokens + contracts).

## Runtime

```
Web UI (provider rings, inward quota details, settings)
        │ invoke / events (snapshots-updated)
        │ set_notch_surface / notch-layout (physical edge geometry)
Rust AppCore
        │ refresh_all()  (parallel per provider; spawn_blocking)
1) Direct vendor OAuth  ──► source: vendor
        │ miss / fail
2) Unavailable / AuthRequired card (source: unavailable; no local JSONL / tokscale)
```

## UI layout contracts

- `src/ui/app.ts` coordinates settings, snapshots, and the detail state machine.
- `src/ui/notch.ts` renders keyed provider rings and the concave SVG silhouette. Ring DOM survives snapshot refreshes.
- `src/ui/notch-state.ts` owns hover/pin/settings/escape transitions, delayed close, and headline presentation.
- `domain/notch.rs` computes physical monitor/working-area bounds and shape hit regions. Negative coordinates and per-monitor scale are valid.
- `infrastructure/notch_window.rs` serializes native layout requests, samples the native cursor every 32ms for transparent input, and rechecks display geometry about once per second. This is independent of the unchanged 5s quota refresh.
- The main transparent HWND expands inward for details. The frontend renders at local DIP coordinates from the returned physical layout; it never moves or resizes the HWND itself.
- No DWM rounded clipping, resize handles, legacy 240px width floor, or MutationObserver content-hug loop. Custom SVG owns the shape.
- Details retain 6px pill tracks, fixed name/period/refill columns and `1fr / 2.9em` metrics with 2px gutter. Opacity readability floors remain.
- Settings scroll inside the inward detail area if the monitor cannot fit the content. Header opacity/update/hide controls now live there.
- Existing `settings.window` survives migration but no longer determines placement. `settings.notch` defaults to right / centre / primary display.
- `begin_notch_drag` / `finish_notch_drag` and `notch-drag` events coordinate session IDs, capture cleanup and frontend detail restoration. Native cursor sampling selects edges/displays in physical coordinates; release outside the HWND is detected by native button state. Keyboard arrows do not move the widget.
- Drag preview is transient; only completed placement changes persist. Missing monitor hints fall back to primary without destroying the saved hint.
- `ProviderSnapshot.primary_used_percent` currently means maximum vendor window utilization, not necessarily the session. UI shows its matching period; no adapter policy changes.

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

> **AGY / Gemini:** removed as of 2026-08-15. The unofficial `cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota` endpoint only returns legacy Code Assist free-tier buckets (gemini-2.5-flash etc.) with `remainingFraction: 1` — it does **not** track Antigravity 2.0 / Gemini 3.7 session usage. Re-add when an official public API exists (`TOKENUSAGE_AGY_QUOTA_SOURCE=official` stub is reserved).

#### What these quota APIs return

These endpoints are **rate-limit / credit windows**, not a token ledger.

| Kind | Present? |
|------|----------|
| Period % + reset (`5h` / `Week` / `30d`) | Yes — this is what the widget shows |
| Claude extra weekly buckets `seven_day_opus` / `seven_day_sonnet` | Optional; parsed as extra windows if the JSON has them |
| Plan / tier strings | Sometimes: Codex `plan_type`; Claude `subscription_type` / creds `rate_limit_tier`; Grok `subscriptionTier` (often **absent** on unified billing — live probe 2026-08-15 had `isUnifiedBillingUser` only) |
| Input / output / cache read / cache write token counts | **No** on these URLs |
| Grok `productUsage` | Same credit pool by product name; **ignored** in UI |

Plan strings are stored on `snapshot.message` when present; healthy cards do not render a tier chip.

#### Grok window mapping

- **Shown:** one primary period window from `creditUsagePercent` (or legacy cents) + period end → typically **Week** (or Monthly/Daily if API says so).
- **Period present, percent omitted:** treat as **0%** (vendor drops default 0 at weekly refill). Keep the Week/Month/Daily label and `resets_at`. Do **not** Degrade.
- **Hint** `unified billing — vendor omitted weekly %` only when unified billing has **neither** percent **nor** `currentPeriod` / `billingPeriodEnd`.
- **Ignored:** `productUsage` array (GrokBuild, GrokChat, …) — same credit pool detail; too noisy for a glance widget. Tests: `ignores_product_usage_breakdown` / `unified_billing_omitted_percent_is_zero_when_period_present` in `quota/grok.rs`.

### No local JSONL / tokscale

Session log estimates and tokscale were removed.  
If vendor quota misses, the card shows **Unavailable** / **AuthRequired** with a short hint (`source: unavailable`).  
There is no `PlanLimits` / local-event estimate path. Poll interval is `RefreshPolicy::DEFAULT_REFRESH_SECS` (5s); persisted `refresh_secs` is ignored.

## Non-goals (v0.1+)

- Push notifications / tray alerts  
- HTTP scraping of vendor dashboards  
- Local JSONL / plan-limit token estimates  
- tokscale / `npx tokscale` integration  
- Browser scraping of Antigravity / AI Studio dashboards  

- Perfect billing parity with official subscription meters  
- Per-product Grok breakdown (GrokBuild vs GrokChat) in the UI  
- Token ledger (input / output / cache read / write) from a different vendor usage API  

## Commands

`get_state`, `get_snapshots`, `refresh_now`, `set_opacity`, `set_autostart`, `set_provider_enabled`, `set_provider_tint`, `hide_widget`, `quit_app`, `get_diagnostics`, `check_for_updates`

Additional notch commands: `get_notch_monitors`, `set_notch_placement`, `preview_notch_placement`, `set_notch_surface`. Layout changes emit `notch-layout`.

## Updater

- Plugin: `tauri-plugin-updater`
- Endpoint: GitHub `releases/latest/download/latest.json`
- Startup auto-check in release builds (`infrastructure/updater.rs`)
- Manual: header **↻** → `check_for_updates`
- Publish: `npm run release:publish` — see [release.md](./release.md)

## Hotkey

Default: `Ctrl+Shift+U` (toggle visibility; refresh on show).
