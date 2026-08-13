# Token Usage

Floating Windows widget that tracks **Claude / Codex / Grok** coding-agent **quota usage vs reset times** — personal CLI OAuth, with **reset-time–first** display. Design language follows [EconomyWarRoom](../EconomyWarRoom) (glass, always-on-top, hotkey).

**Current release:** [v0.1.28](https://github.com/sky64422/TokenUsage/releases/tag/v0.1.28)

## Features (v0.1.28)

- Always-on-top **dark** glass panel; **opacity slider** tints panel, text, and bar colors together (readable floors at low opacity)
- Providers: **Claude Code**, **Codex**, **Grok**
- **Codex:** refreshes expired ChatGPT OAuth tokens (same client as `codex` CLI)
- Data: **direct vendor OAuth quota** only (no tokscale / local JSONL)
- **Progress rows:** label + Quiet Luxury pill track (gradient, glow, sheen, end-cap); dual limits (5h | Week) side-by-side, single limit full width
- **Live-only motion:** sheen / critical breathe only on active fills; update badge pulse while downloading
- **Header %:** single value or dual `a% / b%` with per-leg risk color
- **Reset stamp:** coral `↻ M/D HH:mm` (11px meta); hover title keeps long form / tokens
- **Grok:** one primary period track only (no GrokBuild / GrokChat product rows)
- **Content-hug height:** window min size tracks card content (grow + shrink)
- **Settings overlay:** list fades under opaque sheet (no window expand)
  - Opacity · refresh · launch at login
  - Provider chips (horizontal on/off; last enabled locked)
  - Footer: **Copy Log** / **Quit** (half-width each); meta `{hotkey} · ↻ update`
- Hotkey: `Ctrl+Shift+U` (toggle hide/show; independent of EconomyWarRoom’s `Ctrl+Shift+Space`)
- **Autostart:** OS login item uses **release/install** binary only (`tauri dev` does not overwrite Run key)
- **In-app updates** (header ↻ badge + background download → click to restart; release startup check)
- **No notifications yet** (planned later)
- **Antigravity (AGY)** not in app yet — deferred for Windows widget

## Data sources

### Direct vendor (always on)

Uses OAuth already stored by each CLI (no in-app login). Metadata HTTP only — does not spend coding tokens.

| Provider | Auth | Quota API |
|----------|------|-----------|
| Claude | `~/.claude/.credentials.json` | Anthropic `api/oauth/usage` |
| Codex | `~/.codex/auth.json` | ChatGPT `wham/usage` |
| Grok | `~/.grok/auth.json` | `cli-chat-proxy.grok.com` billing |

Grok maps **period credit %** only; vendor `productUsage` breakdown is ignored in-app.

If vendor quota fails, the card shows **Unavailable** / **AuthRequired**.

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

See [docs/testing.md](docs/testing.md). Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Product: [PRODUCT.md](PRODUCT.md). Design: [DESIGN.md](DESIGN.md). Agent notes: [AGENTS.md](AGENTS.md). Release: [docs/release.md](docs/release.md).
