# PRODUCT.md — TokenUsage

**Updated:** 2026-10-03 · **Ship:** v0.3.4
**Platform:** desktop (Tauri 2 / Windows primary; WebView UI)
**Mode (Impeccable):** **Operate** — scan and act quickly; brand lives in precise details, not persuasion.

## Approved notch interaction (2026-09-30)

Default right-centred notch; left/right vertical and top/bottom horizontal. Click on a model icon opens quota details inward (click again or press Escape to close); hover preview is an option in settings (default off). Right-click the notch for settings; drag anywhere on the notch to move along an edge or dock at another screen edge/display. Escape cancels a drag; arrow keys do not reposition the widget. Quota polling remains 5 seconds. Bottom docking overlays the taskbar; other taskbar-blocked edges fall back visibly.

## Users

- Individual developers using Claude Code, Codex CLI, Grok CLI, and/or Antigravity.
- Often multi-monitor; widget sits above other work (always-on-top, skip taskbar).
- Reading **usage remaining / risk level** at a glance, not reconciling invoices.

## Purpose

A screen-edge **usage monitor notch** that shows personal vendor quota (Claude / Codex / Grok OAuth and Antigravity official CLI) with provider rings and Quiet Luxury detail tracks.

**Claim a neighbor cannot copy:** desktop glass widget + vendor quota only — not a web billing dashboard, not session-log estimates.

## Positioning

| We are | We are not |
|--------|------------|
| Glanceable personal quota | Team admin / org billing console |
| Vendor OAuth / official CLI quota | Local JSONL / tokscale estimates |
| Black concave edge notch | Purple-gradient SaaS landing aesthetic |
| Edge-attached compact monitor | Full-window dashboard |

## Antigravity and auto-hide (2026-10-02)

- **Antigravity (AGY):** official installed `agy --sandbox --print-timeout 30s --print /usage` only (added by user request 2026-10-02). Cached to disk (`quota-cache/agy_snapshot.json`) for 0ms cold-start; background CLI completion notifies immediately via `poll::notify_refresh()`. Gemini and Claude/GPT 5h/week quotas remain separate. No legacy cloudcode-pa API, browser scraping, or token estimates.
- The rail folds to a thin edge tab after the pointer leaves; hover reveals it. Local activity evidence drives a separate ring, never quota estimates. See [implementation and limits](docs/notch-polish.md).

## Evidence / constraints

- Auth: local CLI credential files and official CLI only (no in-app login flow).
- HTTP usage endpoints are metadata (do not burn coding tokens).
- Default hotkey: `Ctrl+Shift+U` (toggle; refresh on show).
- In-app updater via signed GitHub releases (`latest.json`).
- Autostart: OS login item for **release/install** binary only (never `tauri dev` debug path).

## Brand commitments

- **Voice:** calm, short labels, no marketing buzzwords (“supercharge”, “unlock potential”).
- **Dark-only** UI (no light theme switch).
- Standardized, concise Korean UI chrome (`설정`, `모양`, `서비스`, `일반`, `불투명도`, `노치 항상 표시`, `마우스 올릴 때 상세 열기`, `작업 중 궤도 회전`, `작업 중 아이콘 발광`, `로그 복사`, `종료`); local time for reset stamps.

## Non-goals (until explicitly requested)

- Push notifications / tray alerts
- Browser scraping of vendor dashboards
- Local JSONL / plan-limit token estimates / tokscale
- Perfect parity with every vendor subscription UI
- Per-product Grok breakdown (GrokBuild / GrokChat) in the widget
- Token ledger (input / output / cache read / cache write) — quota APIs do not return it
- Subscription-tier chips (vendors may send `plan_type` / `subscription_type`; often omitted)

## Surfaces

| Surface | Mode | Job |
|---------|------|-----|
| Main notch (provider rings) | Operate | See % used / risk / reset at a glance |
| Inward settings sheet | Operate | 3-tab layout: 불투명도, 노치 항상 표시, 마우스 올릴 때 상세 열기, 애니메이션, 서비스 표시/숨김, autostart, 버전, 로그 복사, 종료 |
| System tray | Operate | Show / hide / quit affordances |

## Anti-references

- Side-tab colored card borders, nested cards-in-cards
- Hero metric grids, feature-card icon tiles, Inter-only SaaS homepage
- Glass/neon as pure decoration (our glass is **layering on the desktop**, not a marketing effect)
- Billing dashboards, tables of invoices, multi-page settings
- Decorative charts or gauges beyond the approved summary rings

## Related docs

- Architecture & layout contracts: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- Provider integration guide: [`docs/providers.md`](docs/providers.md)
- Visual system: [`DESIGN.md`](DESIGN.md)
- UI references: [ui.shadcn.com](https://ui.shadcn.com), [impeccable.style](https://impeccable.style) (also listed in `DESIGN.md`)
- Agent rules: [`AGENTS.md`](AGENTS.md)
- Releases: [`docs/release.md`](docs/release.md)
