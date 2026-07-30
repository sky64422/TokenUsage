# Agent instructions (TokenUsage)

If you are an automated coding agent in a new session:

1. **Read first:** [`docs/HANDOFF.md`](docs/HANDOFF.md)
2. **On Windows:** also [`docs/windows-dev.md`](docs/windows-dev.md)
3. **Code map:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
4. **Releases / updater:** [`docs/release.md`](docs/release.md)

## Product constraints

- Floating **usage monitor widget**, not a billing dashboard or team admin console.
- **Primary data:** direct vendor OAuth quota (Claude / Codex / Grok). **Fallback:** `tokscale usage --json`. No local JSONL estimates.
- **No** browser scraping of vendor dashboards without an explicit design decision.
- **Notifications** are out of scope until requested.
- **Antigravity (AGY)** deferred in-app; tokscale may support AGY via `antigravity sync` (macOS/Linux) — not wired here yet.
- UI: keep **fixed column geometry** for tracks (label width shared across single-limit cards); dual vs single layouts may differ.
- Prefer thin `commands.rs`; put logic in `application` / `domain` / provider adapters.
- Keep `tmp/updater.key` **out of git** (signing private key).

## Verify before claiming done

```text
npm test
npm run build
# optional: npm run test:coverage   (needs cargo-tarpaulin; bash)
# UI: npm run tauri dev  (Windows preferred)
```

Default branch: **`main`**. See also [`docs/testing.md`](docs/testing.md).
