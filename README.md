# Token Usage

Floating Windows widget that tracks **Claude / Codex / Grok** coding-agent **quota usage vs reset times** — personal CLI OAuth + optional tokscale, with **reset-time–first** display. Design language follows [EconomyWarRoom](../EconomyWarRoom) (glass, always-on-top, hotkey).

**Current release:** [v0.1.17](https://github.com/sky64422/TokenUsage/releases/tag/v0.1.17)

## Features (v0.1.17)

- Always-on-top glass panel (Apple-like light/dark); **opacity slider** tints panel, text, and bar colors together
- Providers: **Claude Code**, **Codex**, **Grok Build**
- Primary: **direct vendor OAuth quota**; secondary: **`tokscale usage --json`**
- **Progress rows:** label + Quiet Luxury pill track (gradient, glow, sheen, end-cap); dual limits (5h | Week) side-by-side, single limit full width
- **Header %:** single value or dual `a% / b%` with per-leg risk color
- **Reset stamp:** coral `↻ M/D HH:mm` (no countdown clutter); hover title keeps long form / tokens
- **Grok:** one primary period track only (no GrokBuild / GrokChat product rows)
- **Content-hug height:** window min size tracks card content (grow + shrink)
- Settings: theme, opacity, refresh, direct vendor + tokscale toggles, autostart, per-provider visibility
- Hotkey: `Ctrl+Shift+U` (toggle hide/show)
- **In-app updates** (Tauri updater; header **⬆** + release startup check)
- **No notifications yet** (planned later)
- **Antigravity (AGY)** not in app yet — tokscale has AGY sync (macOS/Linux); deferred for Windows widget

## Data sources

### 1) Direct vendor (default on)

Uses OAuth already stored by each CLI (no in-app login). Metadata HTTP only — does not spend coding tokens.

| Provider | Auth | Quota API |
|----------|------|-----------|
| Claude | `~/.claude/.credentials.json` | Anthropic `api/oauth/usage` |
| Codex | `~/.codex/auth.json` | ChatGPT `wham/usage` |
| Grok | `~/.grok/auth.json` | `cli-chat-proxy.grok.com` billing |

Grok maps **period credit %** only; vendor `productUsage` breakdown is ignored in-app.

### 2) tokscale (default on, secondary)

```bash
tokscale usage --json
# or: npx --yes tokscale usage --json
```

Used when direct vendor misses or fails. Cached ~45s. Toggle in Settings.

### No local JSONL

Session log estimates were removed. If both paths fail, the card shows **Unavailable** / **AuthRequired**.

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

See [docs/testing.md](docs/testing.md). Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Agent notes: [AGENTS.md](AGENTS.md).

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
