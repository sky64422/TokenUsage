# DESIGN.md — TokenUsage

**Updated:** 2026-08-13 · **Ship:** v0.1.28  

Visual system for the floating usage widget. Source of truth for tokens: [`src/styles/tokens.css`](src/styles/tokens.css). Layout contracts also live in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

**Surface mode:** Operate (scanability over expression).

## UI references (external)

Canonical external design references for this product family (floating glass widgets — TokenUsage and sibling EconomyWarRoom):

| Site | URL | How we use it |
|------|-----|----------------|
| **shadcn/ui** | [ui.shadcn.com](https://ui.shadcn.com) | Control primitives & patterns: focus rings, segmented controls, switches, chips, dialogs/sheets, semantic token names (`--primary`, `--muted-foreground`, …). Adapt — do not paste full React/shadcn stacks into the vanilla TS widget. |
| **Impeccable** | [impeccable.style](https://impeccable.style) | Taste / density / anti-slop: Operate-mode restraint, hierarchy, no cardocalypse, no decorative motion. Aligns with “Quiet Luxury” progress and distilled chrome. |

When UI diverges, prefer **product constraints** in [`PRODUCT.md`](PRODUCT.md) over generic landing-page patterns from either site.

## Principles

1. **Glance first** — % and risk color beat dense numbers; used/limit is hover/detail.
2. **Quiet Luxury progress** — single pill track with glow / sheen / end-cap; not gauges or charts.
3. **Fixed geometry** — shared label column (`--win-label-min`); dual vs single layouts may differ, columns stay stable.
4. **Glass with purpose** — panel translucency + blur for desktop layering; settings sheet is **opaque** so controls stay readable.
5. **Distill** — no nested cards, no redundant helper copy, no decorative motion without live data.

## Color

Dark-only. Opacity slider drives `--panel-opacity`, `--fg-opacity`, `--accent-opacity`, `--chrome-opacity` together.

### Channels (RGB)

| Token | RGB | Role |
|-------|-----|------|
| `--bg-glass-rgb` | 28, 28, 30 | Panel / popover base |
| `--text-rgb` | 245, 245, 247 | Primary text |
| `--accent-rgb` | 10, 132, 255 | Primary / ring / interactive |
| `--ok-rgb` / bright | 48,209,88 / 52,199,89 | Safe usage fill |
| `--warn-rgb` / bright | 255,159,10 / 255,214,10 | Elevated usage |
| `--critical-rgb` / bright | 255,69,58 / 255,105,97 | High / over |
| `--reset-rgb` | 255, 105, 97 | Reset stamp (warm coral) |

### Semantic (prefer these in new CSS)

| Token | Use |
|-------|-----|
| `--foreground` / `--muted-foreground` / `--subtle-foreground` | Text hierarchy |
| `--muted` / `--card` / `--secondary` | Surfaces / hover |
| `--primary` / `--ring` | Actions, focus, chips on |
| `--success` / `--warning` / `--destructive` | Status / footer actions |
| `--border` / `--border-soft` / `--input` | Edges |
| `--popover` | Settings sheet (solid) |
| `--track` / `--track-inset` | Progress rail |

Legacy aliases (`--text`, `--accent`, `--ok`, …) map to the semantic layer — keep for tracks/levels.

### Do / don't

- **Do** use level colors only on % text and track fills.  
- **Don't** purple–cyan AI gradients, gradient text headings, cream/beige “tasteful” marketing surfaces.  
- **Don't** side-tab thick accent borders on cards.

## Typography

| Role | Spec |
|------|------|
| Family | **Pretendard** (bundled), fallback Segoe UI / system-ui |
| Base | 13px / line-height ~1.35 |
| Title (header) | 13px semibold, slight negative tracking |
| Provider name | ~12.5px semibold |
| Header % | 11px semibold, tabular nums |
| Window label | 10px semibold uppercase (short: `5h`, `Week`) |
| Reset / meta / window label | 11px; functional text ≥11px; avoid long all-caps body |

Hierarchy must stay stepped (name → % → label → reset). No display serif heroes.

## Spacing & radius

| Token | Value |
|-------|--------|
| `--space-1` … `--space-4` | 4 / 8 / 12 / 16 px |
| `--pad-x` | 12px |
| `--header-height` | 38px |
| `--radius` (panel) | **0** — OS DWM rounds HWND; avoid double AA fringe |
| `--radius-sm` … `--radius-xl` | 6 / 8 / 10 / 12 |
| `--radius-card` | 12px |
| `--radius-full` | pills / switches |

Prefer spacing scale over one-off px.

## Layout contracts

### Provider card

- Card: `--card` fill, soft border, `--radius-card`.
- Head: name + source meta left; **% only in header** (dual: `a% / b%` with per-leg level color).
- Windows block under hairline: dual `1fr 1fr` or single stack.

### Window row

- Grid: `max-content | 1fr` with `--win-label-min` (4ch) shared.
- Meta under track indented past label column; **reset only** (no used/limit under bar).

### Settings

- Absolute overlay; providers fade out; **window height does not grow**.
- Solid `--popover` background.
- Controls: opacity meter (5% steps), segmented refresh, switch, provider chips (last enabled locked).
- Footer: equal **Copy Log** / **Quit**.

### Height

Frontend measures content; Rust snaps window height to content floor (not grow-only).

## Components (primitives)

| Primitive | Class / area | Notes |
|-----------|--------------|--------|
| Icon button | `.icon-btn` | 28×28, `--radius-md`, focus ring |
| Segmented | `.segmented` | Refresh presets; `aria-pressed` |
| Switch | `.settings-switch` | Autostart; ring on focus-visible |
| Chip | `.provider-chip` | on/off; last-on locked |
| Progress track | `.track` / `.track-fill` | Quiet Luxury only |
| Action pair | `.settings-debug` / `.settings-quit` | warn / destructive |

### Progress (Quiet Luxury) — do not replace casually

- Pill rail + level gradient fill  
- Soft outer glow, slow diagonal sheen, partial-fill end-cap  
- Critical/over: soft breathe  
- Respect `prefers-reduced-motion` and `prefers-reduced-transparency`

Motion only for **live state** (usage risk, update download) — no decorative idle pulses.

## Focus & a11y

- Focus ring: `2px solid var(--ring)`, offset 2px; **focus-visible only**.  
- Opacity meter: `:focus-within` border/ring.  
- Reduced transparency: blur off, opacities → 1.

## Elevation & glass

- Panel: `backdrop-filter` blur + saturate; border; inset highlight only (no outer halo in transparent WebView).  
- Settings: no glass — full opacity popover.  
- Glass is **desktop layering**, not marketing glassmorphism everywhere.

## Copy

- Short: `Usage`, `5h`, `Week`, `over`, `Copy Log`, `Quit`.  
- Reset: `↻ M/D HH:mm` local; empty when idle / no `resets_at`.  
- Settings footer: `{hotkey} · ↻ update` (distilled).  
- No em-dash cadences, no SaaS buzzwords.

## Low opacity

`applyPanelOpacity` maps `--fg-opacity` / `--accent-opacity` / `--chrome-opacity` **close to the panel glass** so type and green/amber/red fade with the slider (soft floors ~0.28–0.40, not ~0.7 solid on thin glass). Token `max(...)` floors for muted/border are similarly soft.

## Anti-patterns (Impeccable-aligned)

See [impeccable.style](https://impeccable.style). Reject or fix if introduced:

- Nested cards, side-tab borders, cardocalypse  
- Pulsing dots without live download/state  
- Layout thrash from animating height of the window for settings  
- Flat type (everything same size/weight)  
- Functional text &lt; 10px for primary actions  
- Registering `target/debug` as OS autostart  

## File map

| Path | Role |
|------|------|
| `src/styles/tokens.css` | Design tokens |
| `src/styles/app.css` | Layout + primitives |
| `src/ui/*.ts` | Header, providers, settings mount |
| `PRODUCT.md` | Product / mode / non-goals |
