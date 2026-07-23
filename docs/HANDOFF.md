# Handoff — TokenUsage

**Updated:** 2026-07-24  
**Branch of truth:** `main`  
**Remote:** https://github.com/sky64422/TokenUsage

## What it is

Windows always-on-top glass widget showing **Claude / Codex / Grok** coding-agent **quota usage vs reset times**.

## Stack

- Tauri 2 + Rust + TypeScript (Vite)
- Design/shell patterns from **EconomyWarRoom**
- Optional primary: **tokscale** CLI (`usage --json`)

## Done (v0.1 + UI polish)

- Glass panel, hotkey `Ctrl+Shift+U`, theme/opacity/autostart
- tokscale adapter + local JSONL fallback (claude/codex/grok)
- Settings: use_tokscale, per-provider enable + local limits
- In-app updater (Tauri plugin + `npm run release:publish`)
- Unit tests for parsers / usage math
- Risk scenarios + tokscale fixture
- CI: Ubuntu rust/frontend, Windows test/build, security audit, Dependabot
- Coverage script (`npm run test:coverage`, fail-under 70)
- **Content-hug:** `measureContentHugHeight` + `snap_height_to_content` (min size grow/shrink with cards)
- **Layout:** dual limits 2-col grid; single limit full-width track; fixed label column (`3.25em`) so tracks align across Codex/Grok
- **Progress:** thick pill bars; opacity-linked `--fg-opacity` / `--accent-opacity` / `--chrome-opacity`
- **Reset UI:** one-line `↻ M/D HH:mm` (coral); no Idle label; no countdown; tokens under track when present
- **Type scale:** provider name 16px; header % 13px; detail band 11px; card gap 10px

## Not done

- Notifications / tray alerts
- **Antigravity (AGY)** card — tokscale supports `tokscale antigravity sync` (docs: macOS/Linux); not wired in TokenUsage
- Full NSIS installer CI (signed release remains local `release:publish`)
- GUI automation smoke

## Key paths

| Area | Path |
|------|------|
| UI | `src/ui/` (`providers.ts`, `format.ts`, `content-size.ts`, `app.ts`) |
| Styles | `src/styles/app.css`, `tokens.css` |
| Providers | `src-tauri/src/infrastructure/providers/` |
| Window hug | `window_ctl.rs` (`snap_height_to_content`), `commands::set_content_min_size` |
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
