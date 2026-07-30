# Testing & coverage

**Updated:** 2026-07-23

## Snapshot

| Layer | Location | Purpose |
|-------|----------|---------|
| Unit | `src-tauri/src/**` `#[cfg(test)]` | usage_math, claude/codex/grok/tokscale parsers |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Corrupt JSON, fixtures, AppCore limits/visibility |
| Fixture | `src-tauri/tests/fixtures/tokscale_usage.json` | Vendor-shaped tokscale payload |
| GUI | Manual `npm run tauri dev` / `run:exe` | Glass chrome, hotkey, updater |

## Commands

```bash
# From repo root
npm test                 # cargo test --lib + risk_scenarios
npm run test:coverage    # scripts/coverage.sh (fail-under 70, business logic)
npm run build            # frontend tsc + vite
```

```bash
cd src-tauri
export TOKENUSAGE_SKIP_TOKSCALE=1   # avoid spawning tokscale/npx in tests
cargo test --lib
cargo test --test risk_scenarios
```

## Env

| Variable | Effect |
|----------|--------|
| `TOKENUSAGE_SKIP_TOKSCALE=1` | `tokscale::fetch_all` returns Err immediately (local fallback path) |

Risk tests set this automatically.

## Coverage gate

- Tool: `cargo tarpaulin`
- Fail under: **75%** on domain + store + tokscale parse mapping
- Excludes: GUI shell (`lib`/`commands`/`window_ctl`/`updater`), AppCore service, paths + HTTP fetch modules
