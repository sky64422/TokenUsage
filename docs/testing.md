# Testing & coverage

**Updated:** 2026-08-14

## Snapshot

| Layer | Location | Purpose |
|-------|----------|---------|
| Unit | `src-tauri/src/**` `#[cfg(test)]` | usage_math, claude/codex/grok quota parsers |
| Grok | `quota/grok.rs` tests | weekly credits, legacy cents, **ignores productUsage breakdown** |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Corrupt JSON, AppCore limits/visibility, legacy settings |
| GUI | Manual `npm run tauri dev` / `run:exe` | Glass chrome, Quiet Luxury tracks, hotkey, updater, opacity floors |

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

## Manual UI smoke (pre-release)

| Check | How |
|-------|-----|
| Low opacity | Header slider → 35–50%; meta/reset/labels still readable; slider fill stays neutral |
| Live motion | Idle track static; active fill may sheen; critical+active may breathe |
| Settings | No refresh chips; footer shows `vX.Y.Z`; last provider chip locked |
| Autostart (release only) | Install build; Run key should be install path, not `target\debug` |
| Updater | Prior signed build → header ↻ / auto-check to newer `latest.json` |

## Coverage gate

- Tool: `cargo tarpaulin`
- Script: `scripts/coverage.sh` (bash); fail-under threshold for business logic packages
- Optional on Windows: WSL or Git Bash with tarpaulin installed

## Verify before claiming done

```text
npm test
npm run build
# optional: npm run test:coverage
# UI: npm run tauri dev  (Windows preferred)
```
