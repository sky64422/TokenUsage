# Provider Architecture & Integration Guide

This guide explains how personal AI vendor quotas and live activities are integrated into TokenUsage. It is written for AI coding agents and human maintainers extending or debugging provider adapters.

---

## 1. Core Principles & Guardrails

1. **Vendor Quota Only (No Estimates)**:
   - Primary data must come from official vendor OAuth endpoints or official installed vendor CLIs.
   - **Never** compute local token estimates from JSONL files, chat logs, or plan-limit heuristics.
   - **Never** scrape web dashboards or AI Studio pages.
   - **No** tokscale or third-party usage proxies.

2. **Fixed Refresh Architecture**:
   - Polling interval is fixed at **5 seconds** (`domain::constants::RefreshPolicy::DEFAULT_REFRESH_SECS`).
   - Do **not** expose a settings toggle or slider for refresh intervals. Persisted `refresh_secs` is ignored.

3. **Two Distinct Dimensions**:
   - **Quota (`ProviderSnapshot`)**: How much of the rolling/weekly limit has been used (capacity).
   - **Activity (`ProviderActivity`)**: Whether the AI model/agent is actively computing right now (working state).
   - Activity controls visual affordances (rotating orbit, pulsing mark glow); it does **not** modify or fake quota percentages.

---

## 2. Provider Quota Adapters

All quota adapters live in `src-tauri/src/infrastructure/providers/quota/` and are called via `quota::fetch(id)`.

| Provider ID | Method | Auth Storage | Endpoint / Command | Quota Windows |
| :--- | :--- | :--- | :--- | :--- |
| **`claude`** | Direct HTTPS (OAuth) | `~/.claude/.credentials.json` | `api.anthropic.com/api/oauth/usage` | `5h` (rolling), `Week` (7-day buckets) |
| **`codex`** | Direct HTTPS (OAuth) | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` | `5h`, `Week` (or Free `30d`) |
| **`grok`** | Direct HTTPS (OAuth) | `~/.grok/auth.json` | `cli-chat-proxy.grok.com/v1/billing?format=credits` | `Week` (primary credit pool) |
| **`agy`** | Official CLI (ConPTY) | Installed `agy` login | `agy --sandbox --print-timeout 30s --print /usage` | `5h` & `Week` per model family (Gemini, Claude/GPT) |

### Claude Adapter (`claude.rs`, `claude_fetch.rs`)
- Loads OAuth access token from Anthropic CLI credentials.
- Refreshes tokens automatically if expired.
- Maps rate limits into `5h` rolling window and optional 7-day model-specific buckets (`seven_day_opus`, `seven_day_sonnet`).

### Codex Adapter (`codex.rs`, `codex_fetch.rs`)
- Loads ChatGPT OAuth tokens from Codex CLI config.
- Automatically refreshes tokens against ChatGPT backend auth endpoint.
- Classifies window seconds into `5h` or `Week` windows. Supports Free `30d` windows.

### Grok Adapter (`grok.rs`, `grok_fetch.rs`)
- Fetches credit pool percentage from xAI billing endpoint.
- **Rule**: Map primary period credit only. **Ignore** `productUsage` breakdown (GrokBuild vs GrokChat) to avoid glance clutter.
- **Degradation Rule**: If period metadata is present but percent is omitted by vendor (common on free tiers or at refill boundary), mark as degraded/unknown (`null`) rather than falsely claiming 0%. Only explicit zero is treated as 0%.

### Antigravity Adapter (`agy.rs`, `agy_cli.rs`)
- **Execution**: Runs `agy --sandbox --print-timeout 30s --print /usage` inside a pseudo-terminal (Windows ConPTY) with `AGY_CLI_DISABLE_AUTO_UPDATE=true` in a clean Unicode environment block.
- **Multi-group parsing**: Antigravity reports separate quotas for Gemini models and Claude/GPT models. Quotas are parsed into separate grouped `UsageWindow` objects (`group: "Gemini"`, `group: "Claude"`).
- **Disk Caching (`quota-cache/agy_snapshot.json`)**:
  - Because `agy` CLI startup on ConPTY takes ~5.3 seconds, the last known valid snapshot is cached to disk in `dirs::cache_dir()/TokenUsage/quota-cache/agy_snapshot.json`.
  - On app launch, the cache is read immediately (0ms cold start).
  - Background thread fetches fresh quota without blocking UI or other providers.
- **Instant Refresh (`poll::notify_refresh`)**:
  - When the background CLI read finishes, it triggers `poll::notify_refresh()`.
  - The 5s polling loop wakes immediately via `tokio::sync::Notify` and broadcasts `"snapshots-updated"` to the UI.
- **Pending Status**: If no cache exists yet (first run), the pending state returns `SnapshotStatus::Ok` with `"Reading Antigravity CLI quota"`, preventing false `Unavailable` / warning badge (`!`) alarms.

---

## 3. Provider Activity Monitoring

`src-tauri/src/infrastructure/activity.rs` owns the background loop and Tauri events.
`activity/monitor.rs` owns bounded file discovery, tail reads and AGY file timestamps;
`activity/evidence.rs` parses structural lifecycle evidence and applies TTL/aggregation
without filesystem, quota-cache or Tauri calls. `ActivityMonitor` remains reexported
at its original path. Accepted Grok completion records report cache invalidations
to the file adapter, preserving the existing quota-refresh behavior.

- Runs an independent background sampling loop every 2 seconds.
- Detects whether each provider is currently `Running`, `Recent`, `Idle`, or `Unknown`.
- **Sampling Methods**:
  - **Claude**: Samples recent transcript turns in `~/.claude/projects/`.
  - **Codex**: Samples active CLI session execution timestamps in `~/.codex/sessions/`.
  - **Grok**: Detects `turn_started` / `turn_ended` in `events.jsonl` below `~/.grok/sessions/`.
  - **Antigravity**: Uses transcript modification times below `~/.gemini/antigravity*/brain/*/.system_generated/logs/`; it does not parse transcript content or infer quota.
- Emits `provider-activity` events to the frontend whenever activity state changes.

---

## 4. Error Handling & Degradation Matrix

| Scenario | Snapshot Status | UI Notch Presentation | UI Detail Card Presentation |
| :--- | :--- | :--- | :--- |
| **Normal / Healthy** | `SnapshotStatus::Ok` | Colored arc + percentage text | Full Quiet Luxury bars, reset countdown, label |
| **Reading / Initializing** | `SnapshotStatus::Ok` (windows empty) | Quiet track + `—` / `No data` | `"Reading Antigravity CLI quota"` message, no warning badge |
| **Degraded (Cache fallback)** | `SnapshotStatus::Degraded` | Last known % + amber `!` badge | `"Last known quota: {reason}"` chip |
| **Auth Required** | `SnapshotStatus::AuthRequired` | Empty track + `!` badge + `"Sign in"` | `"No {provider} quota — sign in with CLI"` |
| **Unavailable / Error** | `SnapshotStatus::Unavailable` | Empty track + `!` badge + `"No data"` | Full error description in status text |

---

## 5. Adding or Modifying a Provider

When adding a new provider adapter:
1. Add variant to `ProviderId` in `src-tauri/src/domain/types.rs` and `src/ui/types.ts`.
2. Add mark SVG to `src/assets/marks/{id}.svg`.
3. Implement `fetch()` in `src-tauri/src/infrastructure/providers/quota/{id}.rs`.
4. Register in `quota::fetch()` match statement.
5. If asynchronous CLI execution is required, follow the `agy.rs` disk cache + `poll::notify_refresh()` pattern.
6. Implement activity detection in `infrastructure/activity.rs`.
7. Add unit tests for quota parsing, error conditions, and terminal/HTTP sanitation.
8. Verify with `npm test` and `npm run build`.
