# TokenUsage visual system — edge notch

Updated: 2026-10-05 (v0.3.6). Implements the user-approved [edge notch design](docs/superpowers/specs/2026-09-30-edge-notch-design.md). Windows is the native validation target.

## Purpose

A calm personal quota monitor integrated with the physical screen edge. The concave shoulders connecting the black notch to the edge are essential, not optional decoration. Reference: [CodeNotch](https://github.com/vinzdg/codenotch). Controls retain the semantic token/focus conventions from [shadcn/ui](https://ui.shadcn.com) and the restrained Operate-mode density from [Impeccable](https://impeccable.style).

## Main surface

- Right edge, centred by default. Four edge choices; left/right are vertical and top/bottom horizontal.
- 64 DIP depth, 72 DIP provider cells, two tangent 32 DIP circular arcs at each end, with no flat ledge between them: **44 DIP content inset at both ends**. Content sits within the curved end region with breathing room around the first ring and last label. Rings are 40 DIP (12 DIP lateral margins); quiet grey track, colored usage arc, monochrome provider logo.
- The notch shows percentage only; periods are available in quota details. There is no period-display preference. Backend primary percentage is retained; do not silently relabel it as session usage.
- Missing/auth-required values show a dash with explicit status, never a zero. Overage preserves the number while only the arc is clamped.
- Provider icons identify Claude, Codex, Grok, and Antigravity. Source/license notices live in `src/assets/marks`.
- Dark-only; Pretendard, tabular numbers; no continuous decorative ring motion.

## Details and settings

- Drag any part of the notch (including rings) to move or dock on another edge/display. A 5 DIP two-dimensional threshold preserves ring clicks. New edges engage within 64 DIP with 16 DIP hysteresis; monitor seams use 24 DIP entry depth. There are no move/settings buttons; right-click the notch or press Shift+F10/Context Menu to open settings. Arrow keys do not move the widget. Escape cancels a held drag; release commits once.

- Hover/focus opens inward; click keeps a provider open. Escape closes settings first, then pinned details, and restores focus without reopening them.
- The provider whose detail is open stays highlighted while crossing into the panel; pinned state uses a stronger border. Grouped quotas show the service name above model groups so AGY's Gemini/Claude rows retain their source identity.
- 180ms leave grace permits crossing into the detail. 160ms reveal; reduced-motion disables motion.
- Detail cards use a near-black surface, 16px radius, thin quiet border, 16px padding. Native width 260 DIP, max native height 560 DIP, bounded to work area. Scroll only on actual overflow.
- Settings request 400 DIP height so normal appearance controls fit without scrolling; smaller work areas retain body-only scrolling. Action buttons use 32 DIP targets and 8px corners. A pinned provider has a quiet rounded background and border distinct from hover.
- Existing detail rows keep name / period / refill and `1fr` bar / `2.9em` percentage columns, with 2px gutter. Dual and grouped rows use compact `--usage-row-gap: 8px`. Quiet Luxury 6px pill tracks remain.
- Settings sheet uses a 3-tab layout (`모양`, `서비스`, `일반`) with fixed header and tab bar while the body scrolls. Standardized controls: `불투명도` (opacity meter), `노치 항상 표시` (always show), `마우스 올릴 때 상세 열기` (hover detail), `작업 중 강조 효과` (show_animation, unified activity animation toggle), `서비스` on/off grid (`표시`/`숨김`, minimum 1 locked). The General tab features docked footer actions (`로그 복사`, `종료` with precision power icon geometry and 100% symmetric margins). Tab panel heights are unified with `scrollbar-gutter: stable` to eliminate tab-switch scrollbar flashes. Edge and display placement is managed directly by dragging the notch.
- Opacity slider remains neutral with off-white thumb. `applyPanelOpacity` and semantic `max(...)` floors preserve readability. Card tints apply only to quota detail surfaces. Hovering over a provider cell reinforces clear visual identity and stable inward anchor geometry.

## Native contracts

- Transparent rectangular HWND, no native shadow; `DWMWCP_DONOTROUND`. The former 8px DWM panel contract applies to neither the notch nor its shoulders.
- Rust is the sole physical geometry owner. SVG/CSS draw within returned local rectangles; no frontend resize loop or free window resizing.
- Opening or switching details keeps the native canvas and notch local origin fixed. Reserve transparent space for supported detail sizes; only visible notch/detail/bridge regions receive input. Moving the HWND to fit each provider races WebView layout and causes a one-frame jump.
- Transparent regions must pass input to other processes. The open detail bridge intentionally accepts pointer travel. Hover must not steal focus.
- Position uses physical screen bounds, display scale and normalized along-edge offset. Do not mix physical and logical positions or reject negative monitor coordinates.
- Bottom docking explicitly overlays the taskbar at the physical screen bottom (user preference). Other taskbar-blocked edges fall back visibly. The notch does not float above the taskbar.

## Tokens and colors

Sources: `src/styles/tokens.css`, `src/styles/notch.css`. Existing semantic foreground/muted/primary/border/ring and opacity floors remain. Safe/warning/critical colors come from existing quota risk helpers. Settings chrome stays neutral; color communicates usage and state.

## Verification

Use actual Windows WebView2 on desktop backgrounds: no square black/white fringe, no clipped shoulders, stable outer contact during expand/collapse. Validate native click-through and focus separately from browser screenshot layout. See `docs/testing.md` for the matrix and recorded limitations.
