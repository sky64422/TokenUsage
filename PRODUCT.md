# PRODUCT.md — TokenUsage

**Updated:** 2026-08-13 · **Ship:** v0.1.29  
**Platform:** desktop (Tauri 2 / Windows primary; WebView UI)  
**Mode (Impeccable):** **Operate** — scan and act quickly; brand lives in precise details, not persuasion.

## Users

- Individual developers using Claude Code, Codex CLI, and/or Grok CLI.
- Often multi-monitor; widget sits above other work (always-on-top, skip taskbar).
- Reading **usage remaining / risk level** at a glance, not reconciling invoices.

## Purpose

A floating **usage monitor widget** that shows personal vendor OAuth quota (Claude / Codex / Grok) with Quiet Luxury progress tracks.

**Claim a neighbor cannot copy:** desktop glass widget + direct CLI OAuth quota only — not a web billing dashboard, not session-log estimates.

## Positioning

| We are | We are not |
|--------|------------|
| Glanceable personal quota | Team admin / org billing console |
| Vendor OAuth metadata only | Local JSONL / tokscale estimates |
| Quiet Luxury glass monitor | Purple-gradient SaaS landing aesthetic |
| Content-hug floating panel | Full-window dashboard |

## Evidence / constraints

- Auth: local CLI credential files only (no in-app login flow).
- HTTP usage endpoints are metadata (do not burn coding tokens).
- Default hotkey: `Ctrl+Shift+U` (toggle; refresh on show).
- In-app updater via signed GitHub releases (`latest.json`).
- Autostart: OS login item for **release/install** binary only (never `tauri dev` debug path).

## Brand commitments

- **Voice:** calm, short labels, no marketing buzzwords (“supercharge”, “unlock potential”).
- **Dark-only** UI (no light theme switch).
- English UI chrome; local time for reset stamps.

## Non-goals (until explicitly requested)

- Push notifications / tray alerts  
- Browser scraping of vendor dashboards  
- Local JSONL / plan-limit token estimates / tokscale  
- Google Antigravity (AGY) in-widget  
- Perfect parity with every vendor subscription UI  
- Per-product Grok breakdown (GrokBuild / GrokChat) in the widget  

## Surfaces

| Surface | Mode | Job |
|---------|------|-----|
| Main widget (provider cards) | Operate | See % used / risk / reset at a glance |
| Settings overlay | Operate | Opacity, refresh, autostart, providers, quit — no window growth |
| System tray | Operate | Show / hide / quit affordances |

## Anti-references

- Side-tab colored card borders, nested cards-in-cards  
- Hero metric grids, feature-card icon tiles, Inter-only SaaS homepage  
- Glass/neon as pure decoration (our glass is **layering on the desktop**, not a marketing effect)  
- Billing dashboards, tables of invoices, multi-page settings  
- Experimental gauges that replace glanceable pill tracks without an explicit decision  

## Related docs

- Architecture & layout contracts: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)  
- Visual system: [`DESIGN.md`](DESIGN.md)  
- UI references: [ui.shadcn.com](https://ui.shadcn.com), [impeccable.style](https://impeccable.style) (also listed in `DESIGN.md`)  
- Agent rules: [`AGENTS.md`](AGENTS.md)  
- Releases: [`docs/release.md`](docs/release.md)  
