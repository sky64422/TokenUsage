# Token Usage

Screen-edge concave notch widget for Windows that tracks **Claude / Codex / Grok / Antigravity** coding-agent **quota usage vs reset times** and live working activity — personal CLI OAuth and official vendor CLI, with **reset-time–first** display.

**Current release:** [v0.3.5](https://github.com/sky64422/TokenUsage/releases/tag/v0.3.5)

---

## Features (v0.3.5)

- **Concave Edge Notch:** 64 DIP depth with two tangent 32 DIP circular arcs, 44 DIP content inset, and 40 DIP provider rings (12 DIP lateral margins). Transparent HWND passes clicks to desktop apps; bottom docking overlays the Windows taskbar.
- **Inward Quota Detail:** Click a provider ring to inspect inward details. Shows name, period (`5h` / `Week` / `30d`), and reset time on row 1; Quiet Luxury 6px pill bar and percentage on row 2. Dual and grouped rows use compact 8px spacing.
- **Four AI Providers:**
  - **Claude Code:** Direct Anthropic OAuth (`~/.claude/.credentials.json`)
  - **Codex:** Direct ChatGPT OAuth (`~/.codex/auth.json` with token auto-refresh)
  - **Grok:** Direct xAI billing quota (`~/.grok/auth.json` with OIDC refresh)
  - **Antigravity (AGY):** Official installed `agy --sandbox --print-timeout 30s --print /usage` run via Windows ConPTY; disk cached (`quota-cache/agy_snapshot.json`) for instant 0ms cold-start; background CLI completion updates UI immediately.
- **Live Activity Affordances:**
  - Background activity monitor samples local CLI turns and active sessions.
  - Active-only rotating orbit around the provider ring.
  - Active-only gentle mark glow during model execution.
- **Physical Drag & Edge Placement:** Drag anywhere on the notch (including rings) to move along the screen edge or dock at any of the 4 screen edges or across multiple monitors.
- **3-Tab Settings Sheet:**
  - **모양 (Appearance):** 불투명도 (opacity meter with ticks), 노치 항상 표시, 마우스 올릴 때 상세 열기, 작업 중 강조 효과
  - **서비스 (Services):** Individual service toggle grid (`표시` / `숨김`, minimum 1 provider locked)
  - **일반 (General):** Windows 시작 시 실행 (autostart), 앱 정보 (버전 및 인라인 업데이트 확인), 로그 복사 (diagnostics), 종료
- **Pure Vendor Data:** Direct vendor OAuth and official CLI only. No web scraping, no tokscale, no local JSONL token estimates.
- **Fixed Refresh:** Fixed 5-second polling cycle.
- **Hotkey:** `Ctrl+Shift+U` (toggle hide/show).
- **System Tray:** Background tray icon with Show / Hide / Quit affordances.

---

## Data Sources

Uses OAuth already stored by each CLI or the official vendor CLI (no in-app login). Metadata HTTP only — does not spend coding tokens.

| Provider | Method | Auth / Command | Quota Windows |
|:---|:---|:---|:---|
| **Claude** | Direct HTTPS | `~/.claude/.credentials.json` | `5h` rolling limit, optional 7-day buckets |
| **Codex** | Direct HTTPS | `~/.codex/auth.json` | `5h`, `Week` (or Free `30d`) |
| **Grok** | Direct HTTPS | `~/.grok/auth.json` | `Week` primary credit pool |
| **Antigravity** | Official CLI | `agy --sandbox --print-timeout 30s --print /usage` | Gemini & Claude/GPT `5h` / `Week` |

Grok maps **period credit %** only; vendor `productUsage` breakdown is ignored. Period present with omitted % stays unknown/degraded.  
If vendor quota fails or credentials expire, the card explicitly displays **Unavailable** or **AuthRequired** (never a false 0%).

---

## Development

```bash
# Install dependencies
npm install

# Run frontend + backend in development mode
npm run tauri dev
```

```bash
# Run tests (vitest + cargo test --lib + integration tests)
npm test

# Build production bundle (tsc + vite)
npm run build

# Build Windows installer (NSIS / MSI)
npm run tauri build
```

---

## Documentation

- **Agent Guide & Rules:** [`AGENTS.md`](AGENTS.md)
- **Architecture & System Contracts:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- **Provider Integration Guide:** [`docs/providers.md`](docs/providers.md)
- **Visual Design & Tokens:** [`DESIGN.md`](DESIGN.md)
- **Product Definition:** [`PRODUCT.md`](PRODUCT.md)
- **Windows Platform Notes:** [`docs/windows-dev.md`](docs/windows-dev.md)
- **Testing & Verification Matrix:** [`docs/testing.md`](docs/testing.md)
- **Release & In-App Updater:** [`docs/release.md`](docs/release.md)
