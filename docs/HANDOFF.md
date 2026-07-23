# Handoff — TokenUsage

**Updated:** 2026-07-23  
**Branch of truth:** `main`  
**Remote:** https://github.com/sky64422/TokenUsage

## What it is

Windows always-on-top glass widget showing **Claude / Codex / Grok** coding-agent **quota usage vs reset times**.

## Stack

- Tauri 2 + Rust + TypeScript (Vite)
- Design/shell patterns from **EconomyWarRoom**
- Optional primary: **tokscale** CLI (`usage --json`)

## Done (v0.1)

- Glass panel, hotkey `Ctrl+Shift+U`, theme/opacity/autostart
- tokscale adapter + local JSONL fallback (claude/codex/grok)
- Settings: use_tokscale, per-provider enable + local limits
- In-app updater (Tauri plugin + `npm run release:publish`)
- Unit tests for parsers / usage math

## Not done

- Notifications / tray alerts
- Antigravity provider
- Full Windows UI automation smoke in CI
- Coverage gate like WarRoom tarpaulin 85%

## Key paths

| Area | Path |
|------|------|
| UI | `src/ui/` |
| Providers | `src-tauri/src/infrastructure/providers/` |
| Core | `src-tauri/src/application/service.rs` |
| Updater | `src-tauri/src/infrastructure/updater.rs` |
| Publish | `scripts/publish-release.mjs`, `docs/release.md` |

## Verify

```powershell
npm test
npm run build
npm run tauri dev
```

## Security notes

- Never commit `tmp/updater.key`.
- tokscale path uses local OAuth credentials via that CLI; TokenUsage only spawns the process and parses JSON.
