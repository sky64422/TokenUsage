# Token Usage

Floating Windows widget that tracks **Claude / Codex / Grok** coding-agent usage vs configurable limits — **local files & CLI data only**, with **reset-time–first** display. Design language follows [EconomyWarRoom](../EconomyWarRoom) (glass, always-on-top, hotkey).

## Features (v0.1+)

- Always-on-top glass panel (Apple-like light/dark); **opacity slider** tints panel, text, and bar colors together
- Providers: **Claude Code**, **Codex**, **Grok Build**
- Primary data: **`tokscale usage --json`** (vendor quotas); local JSONL fallback
- **Progress rows:** label + thick pill track; dual limits (5h | Week) side-by-side, single limit full width
- **Reset stamp:** coral `↻ M/D HH:mm` (no countdown clutter); hover title keeps long form
- **Content-hug height:** window min size tracks card content (grow + shrink)
- Settings: theme, opacity, refresh, tokscale toggle, autostart, per-provider limits
- Hotkey: `Ctrl+Shift+U` (toggle hide/show)
- **In-app updates** (Tauri updater; header **⬆** + release startup check)
- **No notifications yet** (planned later)
- **Antigravity (AGY)** not in app yet — tokscale has AGY sync (macOS/Linux); deferred for Windows widget

## Data sources

### Primary — tokscale (default on)

```bash
tokscale usage --json
# or: npx --yes tokscale usage --json
```

Vendor-reported session/weekly quotas (same family of numbers as provider dashboards).  
Requires [tokscale](https://github.com/junhoyeo/tokscale) on PATH or working `npx`.  
Results are cached ~45s. Toggle off in Settings → **Use tokscale**.

### Fallback — local JSONL

| Provider | Path / source | Notes |
|----------|---------------|--------|
| Claude | `~/.claude/projects/**/*.jsonl` | Sums assistant `usage`; optional rate_limits dump files |
| Codex | `~/.codex/sessions/**/*.jsonl` | `token_count` deltas |
| Grok | `~/.grok/sessions/**/updates.jsonl` | `totalTokens` deltas |

Local path uses **user-configured token limits** for %. Tokscale path uses **used_percent + resets_at** from the CLI.

## Dev

```bash
npm install
npm run tauri dev
```

```bash
npm run tauri build
npm test
# npm run test:coverage   # tarpaulin gate (bash + cargo-tarpaulin)
```

See [docs/testing.md](docs/testing.md).

### Release / updater

See [docs/release.md](docs/release.md).

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "C:\dev\TokenUsage\tmp\updater.key"
npm run release:publish -- --dry-run   # signed build + latest.json only
# npm run release:publish              # also upload GitHub Release
```

## Stack

- Tauri 2 + Rust + TypeScript + Vite
- Transparent undecorated window, glass CSS tokens
