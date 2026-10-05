# Notch polish and Antigravity (2026-10-02)

User-requested: remove floating-card remnants, adopt edge conceal/reveal and
activity affordances, restore AGY via the official CLI. Preserve the existing
drag-between-displays and anchor jitter fixes. [CodeNotch source/license](../third-party/codenotch/NOTICE.md).

## Interaction

- After 800ms outside, fold to a 10x80 DIP edge target. Hover reveals the rail.
  Native geometry stays fixed; hidden space passes input through. Detail,
  settings, drag and keyboard focus hold it open. Right-click settings remains.
- The activity arc is independent of quota. Codex uses local explicit lifecycle
  events, expiring unmatched starts after 120s without structural progress.
  Claude shows inferred recent activity for 15s. Grok detects turn start/end in
  session event logs (120s TTL). AGY detects recent local session transcripts
  (Running <= 45s, Recent <= 120s). No hooks are installed, prompts retained, or
  quota estimated from transcripts.
- Activity discovery is bounded and read-only. Web/remote sessions, old date
  directories and long silent turns may be missed. Unknown never becomes working.
- Reduced-motion disables orbit/reveal animation; folded cells are inert.

## Quota

CodeNotch uses the same Claude OAuth usage, Codex wham/usage and Grok billing
endpoints. Those adapters and maximum-window primary policy remain.
AGY uses Google's [official `/usage` command](https://www.antigravity.google/docs/cli/commands/usage)
through hidden Windows ConPTY: `agy --sandbox --print-timeout 30s --print /usage`.
Gemini and Claude/GPT each retain their 5h/week windows and supplied reset times.
Remaining percentage becomes used percentage. This is quota, not a token count.

The existing Windows crate provides ConPTY/job objects; no dependency is added.
One read runs at a time with a 40s outer timeout, 64KiB output cap and descendant
cleanup, in a dedicated cache directory. No shell or automatic login is used.
Results/errors are cached for 60s; normal UI polling remains 5s and never waits
for CLI startup. Failed refreshes mark last-known values degraded and preserve
their timestamp. Missing/malformed output is unavailable, never 0%.

## Removed remnants

Old window geometry/defaults/clamping, panel/header/content styles and unused
tokens, token-count formatters, placement-preview/manual-refresh/header-hide IPC.
Legacy `window` JSON loads but is omitted on save. Current placement, opacity,
provider tint and placement preferences remain supported.

## Verification

Run `npm test`, `npm run build`, then isolated Windows preview and
`python -X utf8 scripts/notch-polish.py` for repeated four-edge hover, stable
geometry, settings, keyboard, activity and reduced-motion screenshots.
An explicit live AGY check (prints only count/percentage) is available:

```powershell
cd src-tauri
cargo test --lib infrastructure::providers::quota::agy::tests::live_official_cli_quota -- --ignored --nocapture
```

`TOKENUSAGE_SKIP_DIRECT_QUOTA=1` skips both quota and local activity reads.
