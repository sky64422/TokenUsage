# TokenUsage Architecture

**Stack:** Tauri 2 + Rust + TypeScript (Vite), screen-edge notch with inward quota details.  
**Current ship:** v0.3.6 — release notes: [docs/release.md](./release.md), GitHub [v0.3.6](https://github.com/sky64422/TokenUsage/releases/tag/v0.3.6).

**Product / visual context:** [PRODUCT.md](../PRODUCT.md) (Operate mode), [DESIGN.md](../DESIGN.md) (tokens + contracts), [docs/providers.md](./providers.md) (provider adapters).

## Runtime

```
Web UI (provider rings, inward quota details, 3-tab settings sheet)
        │ invoke / events (snapshots-updated, provider-activity)
        │ set_notch_surface / notch-layout (physical edge geometry)
Rust AppCore
        │ refresh_all()  (parallel per provider via thread scope)
1) Direct vendor OAuth (Claude, Codex, Grok) ──► source: vendor (in-memory 45s cache)
2) Official CLI (Antigravity ConPTY)         ──► source: vendor (disk cache + background fetch + notify_refresh)
        │ miss / fail
3) Unavailable / AuthRequired card          ──► source: unavailable (no local JSONL / tokscale)
```

## UI layout contracts

- `src/ui/app.ts` coordinates settings, snapshots, activities, and the detail/rail state machine.
- `src/ui/notch-surface.ts` serializes native surface requests, deduplicates identical requests, and only paints the latest requested response. `notch-drag.ts` owns gesture sessions, pointer capture, click suppression and input registration cleanup; native placement remains in Rust.
- `src/ui/update-controller.ts` owns updater IPC, phases, feedback timers and event subscriptions. `settings-panel.ts` renders that state; `provider-catalog.ts` shares provider marks/labels between settings and the rail.
- `src/ui/notch.ts` renders keyed provider rings (40 DIP) and the concave SVG silhouette (64 DIP depth, 32 DIP arcs, 44 DIP end insets). Ring DOM survives snapshot refreshes.
- `src/ui/notch-state.ts` owns hover/pin/settings/escape transitions, delayed close, and headline presentation.
- `domain/notch.rs` computes physical monitor/working-area bounds and shape hit regions (64 DIP depth, 72 DIP provider cells, 12 DIP lateral margins). Negative coordinates and per-monitor scale are valid.
- `domain/notch/drag.rs` owns pure edge/monitor drag selection, reexported through `domain::notch::NotchDrag` to preserve the existing API.
- `infrastructure/notch_window.rs` serializes native layout requests, samples the native cursor every 32ms for transparent input, and rechecks display geometry about once per second.
- The main transparent HWND expands inward for details. The frontend renders at local DIP coordinates from the returned physical layout; it never moves or resizes the HWND itself.
- No DWM rounded clipping, resize handles, legacy 240px width floor, or MutationObserver content-hug loop. Custom SVG owns the shape.
- Details retain 6px Quiet Luxury pill tracks, fixed name/period/refill columns and `1fr / 2.9em` metrics with 2px gutter. Multi-group and dual rows use compact `--usage-row-gap: 8px`. Opacity readability floors remain.
- Settings sheet uses a 3-tab layout (`모양`, `서비스`, `일반`) with fixed header and tab bar while the body scrolls. Appearance provides `불투명도` (opacity), `노치 항상 표시` (always show), `마우스 올릴 때 상세 열기` (hover detail), and unified `작업 중 강조 효과` (activity animations). Service badges mean shown/hidden (`표시`/`숨김`), not account health; minimum 1 provider is locked. General tab docks footer actions (`로그 복사`, `종료`) with symmetric margins and precision icons. Tab panel heights are unified and scrollbar flash eliminated. Toggle saves disable pending controls and roll back failed changes; update actions use explicit text.
- Provider failures use a compact status summary once per card; full vendor messages remain in the element title and diagnostic data.
- Legacy `settings.window` is accepted on load and omitted on save. `settings.notch` defaults to right / centre / primary display.
- `begin_notch_drag` / `finish_notch_drag` and `notch-drag` events coordinate session IDs, capture cleanup and frontend detail restoration. Native cursor sampling selects edges/displays in physical coordinates; release outside the HWND is detected by native button state. Keyboard arrows do not move the widget.
- Drag preview is transient; only completed placement changes persist. Missing monitor hints fall back to primary without destroying the saved hint.
- `ProviderSnapshot.primary_used_percent` currently means maximum vendor window utilization, not necessarily the session. UI shows its matching period; no adapter policy changes.

## Autostart

- Preference persisted in app state; OS login item via `tauri-plugin-autostart`.
- **`enable` only in release builds** (`sync_os_autostart`) so `tauri dev` never registers `target/debug/token-usage.exe` (Vite `devUrl` would fail at boot).
- Disable still runs in debug so a bad Run key can be cleared.

## Providers

See [`docs/providers.md`](./providers.md) for full integration details.

### Direct vendor quota (personal OAuth & official CLI)

Reads local CLI auth or official CLI only (no in-app login). Always on:

| Id | Method | Auth file / Command | Quota Endpoint |
|----|--------|---------------------|----------------|
| `claude` | Direct HTTPS | `~/.claude/.credentials.json` (+ OAuth refresh) | `api.anthropic.com/api/oauth/usage` |
| `codex` | Direct HTTPS | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` |
| `grok` | Direct HTTPS | `~/.grok/auth.json` (+ OIDC refresh) | `cli-chat-proxy.grok.com/v1/billing?format=credits` |
| `agy` | Official CLI | `agy --sandbox --print-timeout 30s --print /usage` | ConPTY terminal parser + disk cache |

- **Antigravity (AGY):** official installed CLI run via Windows ConPTY with `AGY_CLI_DISABLE_AUTO_UPDATE=true`. Quota is cached to disk (`quota-cache/agy_snapshot.json`) for instant 0ms cold-start; completion notifies via `poll::notify_refresh()`. Gemini and Claude/GPT 5h/week quotas remain separate.
- **Grok:** map primary period credit only; `productUsage` array (GrokBuild, GrokChat) is ignored. Period present with omitted % stays unknown/degraded (does not claim 0%).
- Shared HTTP body cache implementation + HTTP status mapping (`quota/http.rs`), with 45s Claude/Codex and 15s Grok TTLs; snapshot primaries in `quota/snapshot.rs`. Window labels are short at the adapter (`5h` / `Week` / `Month` / `30d`).
- Env `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` for tests.

### No local JSONL / tokscale

Session log estimates and tokscale were removed.  
If vendor quota misses, the card shows **Unavailable** / **AuthRequired** with a short hint (`source: unavailable`).  
There is no `PlanLimits` / local-event estimate path. Poll interval is `RefreshPolicy::DEFAULT_REFRESH_SECS` (5s); persisted `refresh_secs` is ignored.

## Non-goals

- Push notifications / tray alerts  
- HTTP scraping of vendor dashboards  
- Local JSONL / plan-limit token estimates  
- tokscale / `npx tokscale` integration  
- Browser scraping of Antigravity / AI Studio dashboards  
- Perfect billing parity with official subscription meters  
- Per-product Grok breakdown (GrokBuild vs GrokChat) in the UI  
- Token ledger (input / output / cache read / write) from a different vendor usage API  

## Registered Tauri Commands

`AppSettings` owns provider config access and enabled-provider ordering used by both
quota refresh and native geometry. Setting writes commit the cloned in-memory state
only after persistence succeeds, including provider visibility and notch placement.
`infrastructure/poll.rs::refresh_and_emit` is shared by boot, periodic/AGY wakeups and
show-window refresh; worker and event-delivery failures are recorded in diagnostics.
The original command names and JSON payloads are unchanged.

- State & Snapshots: `get_state`, `get_snapshots`
- Preferences & Controls: `set_opacity`, `set_autostart`, `set_hover_detail`, `set_always_show_notch`, `set_show_animation`, `set_show_orbit`, `set_show_icon_glow`, `set_show_period` (legacy compat), `set_provider_enabled`, `set_provider_tint`
- Notch & Window Geometry: `get_notch_monitors`, `get_notch_reveal`, `set_notch_focus`, `set_notch_placement`, `begin_notch_drag`, `finish_notch_drag`, `set_notch_surface`
- Activity & Diagnostics: `get_provider_activity`, `get_diagnostics`, `check_for_updates`, `quit_app`

## Updater

- Plugin: `tauri-plugin-updater`
- Endpoint: GitHub `releases/latest/download/latest.json`
- Startup auto-check in release builds (`infrastructure/updater.rs`)
- Manual: Settings > 일반 > 앱 정보 > 업데이트 확인 (`btn-check-update`) → `check_for_updates`
- Publish: `npm run release:publish` — see [release.md](./release.md)




