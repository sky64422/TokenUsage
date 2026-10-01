# TokenUsage visual system ? edge notch

Updated: 2026-09-30. Implements the user-approved [edge notch design](docs/superpowers/specs/2026-09-30-edge-notch-design.md). Windows is the native validation target.

## Purpose

A calm personal quota monitor integrated with the physical screen edge. The concave shoulders connecting the black notch to the edge are essential, not optional decoration. Reference: [CodeNotch](https://github.com/vinzdg/codenotch). Controls retain the semantic token/focus conventions from [shadcn/ui](https://ui.shadcn.com) and the restrained Operate-mode density from [Impeccable](https://impeccable.style).

## Main surface

- Right edge, centred by default. Four edge choices; left/right are vertical and top/bottom horizontal.
- 72 DIP depth, 104 DIP provider cells, two tangent 36 DIP circular arcs at each end, with no flat ledge between them: **50 DIP content inset at both ends**. Content sits within the curved end region with breathing room around the first ring and last label. Rings are 40 DIP; quiet grey track, colored usage arc, monochrome provider logo.
- Percentage and period are always visible. Backend primary percentage is retained; currently it is the maximum across windows. Do not silently relabel it as session usage.
- Missing/auth-required values show a dash with explicit status, never a zero. Overage preserves the number while only the arc is clamped.
- Provider icons identify Claude, Codex, and Grok. Source/license notices live in `src/assets/marks`.
- Dark-only; Pretendard, tabular numbers; no continuous decorative ring motion.

## Details and settings

- Drag any part of the notch (including rings) along its edge. A 5px movement threshold preserves ring clicks. There are no move/settings buttons; right-click the notch or press Shift+F10/Context Menu to open settings. Arrow keys move the focused notch along its edge.

- Hover/focus opens inward; click keeps a provider open. Escape closes settings first, then pinned details, and restores focus without reopening them.
- 180ms leave grace permits crossing into the detail. 160ms reveal; reduced-motion disables motion.
- Detail cards use a near-black surface, 16px radius, thin quiet border, 16px padding. Max native height 560 DIP, bounded to work area. Scroll only on actual overflow.
- Existing detail rows keep name / period / refill and `1fr` bar / `2.9em` percentage columns, with 2px gutter. Quiet Luxury 6px pill tracks remain.
- Existing opacity/update/hide controls live in settings, with edge/display/recentre, autostart, providers, Copy Log, Quit and version.
- Opacity slider remains neutral with off-white thumb. `applyPanelOpacity` and semantic `max(...)` floors preserve readability. Card tints apply only to quota detail surfaces.

## Native contracts

- Transparent rectangular HWND, no native shadow; `DWMWCP_DONOTROUND`. The former 8px DWM panel contract applies to neither the notch nor its shoulders.
- Rust is the sole physical geometry owner. SVG/CSS draw within returned local rectangles; no frontend resize loop or free window resizing.
- Transparent regions must pass input to other processes. The open detail bridge intentionally accepts pointer travel. Hover must not steal focus.
- Position uses physical screen bounds, display scale and normalized along-edge offset. Do not mix physical and logical positions or reject negative monitor coordinates.
- A physical edge occupied by taskbar falls back to an available edge, explicitly stated in settings. The notch never floats above the taskbar pretending to touch the screen edge.

## Tokens and colors

Sources: `src/styles/tokens.css`, `src/styles/notch.css`. Existing semantic foreground/muted/primary/border/ring and opacity floors remain. Safe/warning/critical colors come from existing quota risk helpers. Settings chrome stays neutral; color communicates usage and state.

## Verification

Use actual Windows WebView2 on desktop backgrounds: no square black/white fringe, no clipped shoulders, stable outer contact during expand/collapse. Validate native click-through and focus separately from browser screenshot layout. See `docs/testing.md` for the matrix and recorded limitations.
