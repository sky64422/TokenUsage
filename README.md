# Token Usage

Floating Windows widget that tracks **Claude / Codex / Grok** coding-agent **quota usage vs reset times** — personal CLI OAuth, with **reset-time–first** display. Design language follows [EconomyWarRoom](../EconomyWarRoom) (glass, always-on-top, hotkey).

**Current release:** [v0.1.34](https://github.com/sky64422/TokenUsage/releases/tag/v0.1.34)

## Features (v0.1.34)

- Always-on-top **dark** glass panel; **opacity slider** (neutral chrome) tints panel, text, and bar colors together (readable floors at low opacity)
- Arrow keys nudge the window when focus is on the body (**4px**; **Shift+arrow** **16px**; **Ctrl+arrow** **1px**; not while Settings or the opacity slider is focused)
- Providers: **Claude Code**, **Codex**, **Grok**
- **Codex:** refreshes expired ChatGPT OAuth tokens (same client as `codex` CLI)
- Data: **direct vendor OAuth quota** only (no tokscale / local JSONL / plan-limit estimates)
- First quota refresh is **non-blocking** (window shows, then cards fill)
- **Progress rows:** name + period (`5h` / `Week` / `30D`) + refill on row 1; 6px Quiet Luxury pill + **%** on row 2; dual windows stack two blocks
- **Live-only motion:** sheen / critical breathe only on active fills; update badge pulse while downloading
- **Reset stamp:** coral `↻ M/D HH:mm`; hover title keeps long form / tokens
- **Grok:** one primary period track only (no GrokBuild / GrokChat product rows); omitted 0% at weekly refill still shows Week + reset
- **Content-hug height:** window min size tracks card content (grow + shrink); panel fills HWND (8px radius matches DWM)
- **Settings overlay:** list fades under opaque sheet (no window expand)
  - Opacity (header) · launch at login
  - Provider chips (horizontal on/off; last enabled locked)
  - Refresh interval fixed at 5s (no control)
  - Footer: **Copy Log** / **Quit**; meta `{hotkey} · ↻ update`; app version
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
These quota URLs return **window % + reset** (and sometimes a plan string). They do **not** return input / output / cache token counts.

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
