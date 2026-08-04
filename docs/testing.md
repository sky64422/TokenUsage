# Testing & coverage

**Updated:** 2026-08-04

## Snapshot

| Layer | Location | Purpose |
|-------|----------|---------|
| Unit | `src-tauri/src/**` `#[cfg(test)]` | usage_math, claude/codex/grok quota parsers |
| Grok | `quota/grok.rs` tests | weekly credits, legacy cents, **ignores productUsage breakdown** |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Corrupt JSON, AppCore limits/visibility, legacy settings |
| GUI | Manual `npm run tauri dev` / `run:exe` | Glass chrome, Quiet Luxury tracks, hotkey, updater |

## Commands

```bash
# From repo root
npm test                 # cargo test --lib + risk_scenarios
npm run test:coverage    # scripts/coverage.sh (fail-under 75, business logic)
npm run build            # frontend tsc + vite
```

```bash
cd src-tauri
export TOKENUSAGE_SKIP_DIRECT_QUOTA=1   # avoid vendor HTTP in tests
cargo test --lib
cargo test --test risk_scenarios
```

## Env

| Variable | Effect |
|----------|--------|
| `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` | Direct vendor fetch skipped (unavailable cards in tests) |

Risk tests set this automatically.

## Coverage gate

- Tool: `cargo tarpaulin`
- Fail under: **75%** on domain + store + quota JSON mapping
- Excludes: GUI shell (`lib`/`commands`/`window_ctl`/`updater`), AppCore service, paths + HTTP fetch modules
