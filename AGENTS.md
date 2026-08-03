# Agent instructions (TokenUsage)

If you are an automated coding agent in a new session:

1. **Code map / product shape:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
2. **On Windows:** [`docs/windows-dev.md`](docs/windows-dev.md)
3. **Releases / updater:** [`docs/release.md`](docs/release.md)
4. **Tests:** [`docs/testing.md`](docs/testing.md)

## Product constraints

- Floating **usage monitor widget**, not a billing dashboard or team admin console.
- **Primary data:** direct vendor OAuth quota (Claude / Codex / Grok). **Fallback:** `tokscale usage --json`. No local JSONL estimates.
- **No** browser scraping of vendor dashboards without an explicit design decision.
- **Notifications** are out of scope until requested.
- **Antigravity (AGY)** deferred in-app; tokscale may support AGY via `antigravity sync` (macOS/Linux) — not wired here yet.
- UI: keep **fixed column geometry** for tracks (label width shared across single-limit cards); dual vs single layouts may differ.
- **Progress:** Quiet Luxury pill bars (glow / sheen / end-cap) — prefer glanceable bars over experimental gauges unless explicitly requested.
- **Grok:** map primary period credit only; do **not** surface `productUsage` product rows (GrokBuild / GrokChat).
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
