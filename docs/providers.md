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
   - Activity controls icon emphasis and sheen inside filled detail bars; it does **not** modify or fake quota percentages. The notch uses one continuous usage ring, without a secondary activity ring (user request 2026-10-18).

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

### Recorded token details (expanded 2026-10-19)

The user-authorized `infrastructure/token_usage/` reader indexes exact recorded
usage/model metadata for Codex, AGY, Claude and Grok. Independent bounded background
workers and ledgers attach optional `token_usage` after vendor fetching, including
quota failures. Vendor quotas still come exclusively from their existing official
OAuth/CLI sources; recorded tokens never replace or estimate a vendor quota.
The UI exposes Today, Yesterday and calendar-month summaries with total tokens and
cache-read share (cache-read / inclusive input), followed by a blue daily chart
for the current calendar month. Future and unrecorded days show faint baseline slots, distinct from the brighter baseline for recorded zero. No bottom model breakdown or period tabs are shown. The UI has no visible provenance,
update time, status copy or explanatory footnotes (user request 2026-10-19); the
internal Monday-based week summary remains available for compatibility. Cumulative
reports and copied sessions are deduplicated. Cache and reasoning are subsets,
never extra quota or token totals. Partial scans/unknown fields remain explicit.
Only numeric/model metadata is indexed or emitted. Incomplete file lines use
bounded transient parsing buffers; rewrite detection retains length/hash
fingerprints rather than raw transcript samples. No conversation content is
written to the index or diagnostics.

Main bars keep vendor quota width and percentage. Optional token `windows`
summaries cover reset-minus-five-hours / reset-minus-seven-days through the summary
timestamp, matched by kind, reset instant, group and label. Their model shares shade only the
filled portion. Visible legends contain only matching color dots and short model
names; model percentages were removed at user request (2026-10-19). Accessible
labels identify local token composition, not a vendor-reported model quota. Missing, expired or unmatched reset data leaves a plain quota
bar. Calendar token summaries remain independent. Retention includes the preceding
seven days across month boundaries so weekly quota periods remain available.
Preview fixture mode disables real reads for every provider.

| Provider | Recorded source | Available details / normalization |
| --- | --- | --- |
| Codex | `sessions/**/*.jsonl` usage events | Existing model/context and cumulative-delta handling; input includes cache and output includes reasoning. |
| AGY | `antigravity*/brain/*/.system_generated/logs/transcript.jsonl` | Completed model steps: input + separate cache-read becomes inclusive input; output is recorded. Existing history has no model name, cache-write or reasoning count, so those remain unknown. Ignore `transcript_full.jsonl`. |
| Claude | `projects/**/*.jsonl`, top-level assistant usage | Response ID deduplicates repeated chunks. Inclusive input sums uncached input, cache-read and cache-write. Recorded model is retained; reasoning remains unknown. Ignore nested progress/tool summaries and synthetic responses. |
| Grok | `sessions/**/updates.jsonl`, completed turn metadata | Exact per-model counts; cache is already inside input and reasoning inside output. Validate model totals, replace corrected turn maps atomically and select one complete version across copied files. Ignore capacity metadata and chat/event mirrors. |

AGY's unknown historical model is never inferred from today's selected model or
attributed to a Gemini/Claude quota family. Claude Opus/Sonnet weekly buckets are
filtered independently even when resets match. Daily/monthly credit bars remain
plain because their exact start boundary is not supplied to the token ledger.
Token disclosures retain open state and focus across refreshes. Missing numeric fields remain `null` and display an em dash.

#### Optional AGY model hook (2026-10-19)

Run `python scripts/install-agy-token-hook.py` to opt in on this machine. It copies
the Python standard-library handler to `~/.gemini/config/tokenusage/` and merges
the named `tokenusage-models` PostInvocation hook into `~/.gemini/config/hooks.json`.
An existing config is backed up before adding the entry; other hooks are preserved.
Python must remain installed at the registered executable path. New AGY sessions
load the hook; restart an already-running session to ensure it is loaded.
To stop collection, set this named hook's `enabled` to `false` in that file.

The callback uses official `modelName` and `initialNumSteps`, reads at most the
last 8 MiB of the completed compact transcript, and appends only exact numeric,
timestamp, step and model metadata to sibling `tokenusage-models.jsonl`. Installed
CLI payloads can name `transcript_full.jsonl`; the handler validates its location
then reads the compact sibling. Original transcripts are never modified. No prompt,
response text or workspace list is stored. Missing/oversized/not-yet-flushed records
produce a content-free hook diagnostic and retain unknown model attribution.
The token reader discovers the sidecar under its existing bounded budgets and
deduplicates transcript/hook copies by conversation and step, preferring the explicit
model observation. Historical steps before the hook remain unknown. Sidecars remain
beside their conversations on disk; the in-memory index uses existing retention.
Official quota collection is unchanged. A live Windows CLI invocation verified
`gemini-3.8-flash-high` metadata capture; App/IDE hooks use the same documented
global configuration but were not separately exercised.

Schema references: [AGY transcript contract](https://www.antigravity.google/docs/hooks),
[AGY token examples](https://www.antigravity.google/docs/cli/headless/),
[Claude disjoint input buckets](https://platform.claude.com/docs/en/build-with-claude/prompt-caching),
[Grok durable update source](https://github.com/xai-org/grok-build).

Collection is incremental in memory, with at most one active worker per enabled
provider. Each provider has independent cursors, coverage, retention and caches;
initialization of a disabled reader performs no discovery. Per provider/pass it
discovers at most 512 entries, reads at most 8 MiB (1 MiB per file), and visits
at most 64 files. Recent files, including unread sessions, receive a reserved 2 MiB allowance;
background files continue in stable rotation. Retained incomplete-line
allocations total at most 8 MiB; oversized/discarded buffers are released.
Confirmed missing files release cursor slots while known numeric history stays
until retention expiry; permission failures remain partial. Retention pruning
runs only when its date advances. Summaries are cached until records, coverage,
calendar boundaries or quota-window boundaries change. Records written after a
scan captures its timestamp retain numeric metadata and activate only when that
time arrives. Record-limit coverage warnings expire with the latest excluded
event date, not merely when a slot is freed. Timestamp-only updates
do not request an extra quota refresh. Startup still rebuilds the metadata index
from session files; there is no persistent token index, so large histories may
need multiple bounded passes. These limits are not measured CPU/RAM savings.


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
