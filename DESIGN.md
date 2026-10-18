# TokenUsage visual system — edge notch

Updated: 2026-10-17 (v0.4.2). Implements the user-approved [edge notch design](docs/superpowers/specs/2026-09-30-edge-notch-design.md). Windows is the native validation target.

## Purpose

A calm personal quota monitor integrated with the physical screen edge. The concave shoulders connecting the black notch to the edge are essential, not optional decoration. Reference: [CodeNotch](https://github.com/vinzdg/codenotch). Controls retain the semantic token/focus conventions from [shadcn/ui](https://ui.shadcn.com) and the restrained Operate-mode density from [Impeccable](https://impeccable.style).

## Main surface

- Right edge, centred by default. Four edge choices; left/right are vertical and top/bottom horizontal.
- 64 DIP depth, 72 DIP provider cells, two tangent 32 DIP circular arcs at each end, with no flat ledge between them: **44 DIP content inset at both ends**. Content sits within the curved end region with breathing room around the first ring and last label. Rings are 40 DIP (12 DIP lateral margins); quiet grey track, colored usage arc, monochrome provider logo.
- The notch shows percentage only; periods are available in quota details. There is no period-display preference. Backend primary percentage is retained; do not silently relabel it as session usage.
- Summary rings use one continuous circular track and usage arc (user-requested 2026-10-18). No quarter gaps or secondary activity ring. The actual percentage and risk color remain; zero/missing usage has no colored dot. Activity retains the existing icon emphasis.
- Missing/auth-required values show a dash with explicit status, never a zero. Overage preserves the number while only the arc is clamped.
- Provider icons identify Claude, Codex, Grok, and Antigravity. Source/license notices live in `src/assets/marks`.
- Three themes: Dark retains the original surface; Glass uses a neutral translucent surface with a mirrored rim; Liquid uses an undistorted live backdrop, blur and colored edge reflections. Pretendard and tabular numbers remain.

## Details and settings

- Drag any part of the notch (including rings) to move or dock on another edge/display. A 5 DIP two-dimensional threshold preserves ring clicks. New edges engage within 64 DIP with 16 DIP hysteresis; monitor seams use 24 DIP entry depth. There are no move/settings buttons; right-click the notch to open settings. App-specific keyboard controls are removed; mouse release commits once.

- Hover/focus opens inward; click keeps a provider open. Click the selected icon again to close a pinned detail; use the settings close button to close settings.
- The provider whose detail is open stays highlighted while crossing into the panel; pinned state uses a stronger border. Grouped quotas show the service name above model groups so AGY's Gemini/Claude rows retain their source identity.
- 180ms leave grace permits crossing into the detail. 160ms reveal; reduced-motion disables motion.
- Detail cards use a near-black surface, 16px radius, thin quiet border, 16px padding. Native width 260 DIP; height follows intrinsic content up to the current work area (user-requested 2026-10-18). The native canvas reserves that capacity from the start so token expansion never moves the notch/window. Vertical-edge cards keep a height-independent header anchor and grow downward first, shifting upward only when the bottom work boundary requires it. Top-edge cards expand down; bottom-edge cards use the available space above. Scroll only when content exceeds the work area.
- Codex token details use `오늘 / 이번 주 / 이번 달` with the shared settings-tab sliding indicator and subtle active light. Week starts Monday at local midnight. Model shading belongs in the **main quota bars above token details** (user correction 2026-10-18), with one dot/short-name legend directly below each bar. Keep vendor fill width/% and its risk hue; largest recorded model usage gets the darkest shade, smaller amounts progressively lighter shades of that hue. Model blocks use 1px corners and 2px internal gaps within the unchanged quota fill width. Legends use compact 10px text placed close to the bar; show dots/names only, without leading source/status copy (explicit user request). Accessible labels/titles identify local input+output token shares for the matching vendor reset period, not model-specific quota consumption. Hide shading if data/reset is missing or stale. Calendar tab selection only changes numeric token detail. Legends scroll horizontally on overflow.
- Token metric columns align cache share under input on the left and reasoning under output on the right.
- **Settings height is shared across all three tabs (user-confirmed 2026-10-17).** Measure the tallest tab's intrinsic content plus actual surrounding chrome. Switching tabs on the same monitor must keep the panel height fixed; never resize from only the active tab. Recompute the shared height for monitor/DPI changes. Smaller work areas retain body-only scrolling with a fixed header and tabs. The 440 DIP value is only a fallback before measurement. Change this contract only at the user's explicit request. Action buttons use 32 DIP targets and 8px corners. A pinned provider has a quiet rounded background and border distinct from hover.
- Existing detail rows keep name / period / refill and `1fr` bar / `2.9em` percentage columns, with 2px gutter. Dual and grouped rows use compact `--usage-row-gap: 8px`. Quiet Luxury 6px pill tracks remain. Sheen and fill light stay clipped to the filled portion; running activity must not add a halo or blur to the empty track.
- Settings sheet uses a 3-tab layout (`모양`, `서비스`, `일반`) with fixed header and tab bar while the body scrolls. Standardized controls: `불투명도` (opacity meter), `노치 항상 표시` (always show), `마우스 올릴 때 상세 열기` (hover detail), `작업 중 강조 효과` (show_animation, unified activity animation toggle), `서비스` on/off grid (`표시`/`숨김`, minimum 1 locked). The General tab features docked footer actions (`로그 복사`, `종료` with precision power icon geometry and 100% symmetric margins). Tab panel heights are unified with `scrollbar-gutter: stable` to eliminate tab-switch scrollbar flashes. Edge and display placement is managed directly by dragging the notch.
- Opacity slider remains neutral with off-white thumb. `applyPanelOpacity` and semantic `max(...)` floors preserve readability. Hovering over a provider cell reinforces clear visual identity and stable inward anchor geometry.

## Native contracts

- Transparent rectangular HWND, no native shadow; `DWMWCP_DONOTROUND`. The former 8px DWM panel contract applies to neither the notch nor its shoulders.
- Rust is the sole physical geometry owner. SVG/CSS draw within returned local rectangles; no frontend resize loop or free window resizing.
- Opening or switching details keeps the native canvas and notch local origin fixed. Reserve transparent space for supported detail sizes; only visible notch/detail/bridge regions receive input. Moving the HWND to fit each provider races WebView layout and causes a one-frame jump.
- Transparent regions must pass input to other processes. The open detail bridge intentionally accepts pointer travel. Hover must not steal focus.
- Position uses physical screen bounds, display scale and normalized along-edge offset. Do not mix physical and logical positions or reject negative monitor coordinates.
- Bottom docking explicitly overlays the taskbar at the physical screen bottom (user preference). Other taskbar-blocked edges fall back visibly. The notch does not float above the taskbar.

## Tokens and colors

Sources: `src/styles/tokens.css`, `src/styles/notch.css`. Existing semantic foreground/muted/primary/border/ring and opacity floors remain. Safe/warning/critical colors come from existing quota risk helpers. Settings chrome stays neutral; color communicates usage and state.

Quota rings, percentage text and detail bars share inclusive upper bounds: 0–50% green, >50–75% yellow, >75–90% orange, >90% red (including overage). Missing values and explicit idle states retain neutral styling.

### Glass and Liquid materials

- Dark, Glass and Liquid are separate persisted choices. Old system/light values migrate to Dark without losing preferences.
- Glass replaces the previous painted foil finish with the supplied lock-screen reference: neutral smoked translucency, a narrow white reflective edge, restrained refraction and no stacked label scrims. Adaptive neutral shading keeps bright backdrops legible.
- Liquid uses an undistorted live backdrop with its existing blur, colored reflections and dark text scrims. Displacement is fixed at 0: skip the SVG refraction filter and distance/displacement-map generation. Glass retains its lens and neutral 3px reflective rim with a 0.7px specular edge.
- Both capture only visible desktop rectangles in memory; foreground marks, rings, percentages and controls remain sharp. Native silhouette, hit regions and settings sizing are unchanged.
- One serialized renderer batch uses a dedicated GDI worker with two reusable DIB surfaces. Raw RGBA packets replace per-frame BMP/image decoding. Identical pixels for the same region and generation skip IPC image payloads and painting. Legacy diagnostics may still request BMP.
- Target intervals remain 30 Hz at rest, 60 Hz while dragging and 4 Hz with reduced motion. These intervals are not proof of presented FPS. Fold stops capture; explicit hide also releases frontend backing stores. Theme changes/reload/expired leases restore capture affinity after pending reads finish.
- Windows excludes the HWND from OS capture/sharing while either live material is active to avoid recursion. Dark does not capture. Capture failures report an error and retain the tint fallback.
- The tray contains Show and Quit. App-specific keyboard controls are removed; automatic mouse fold/reveal remains.

### Monitor-aware notch size

- Resolve the monitor's logical short side as `min(width, height) / DPI scale`.
  Scale the notch by `clamp(1.0 - (1.0 - sqrt(shortSide / 1440)) * 0.5, 0.92, 1)`; current QHD
  size is the upper limit and smaller screens reduce gently with an attenuated ratio gap. This policy
  applies to every resolution rather than matching particular display names.
- Scale the silhouette, cell spacing, rings, marks, folded target and native input
  geometry together. Summary percentages use a 12.5 DIP base and an 11.5 DIP minimum.
- Details and settings scale proportionally with `content_scale` (`260px * content_scale` width,
  intrinsic detail height bounded to work area, and a shared measured settings height),
  with scaled padding, tabs, card heights, and corner radii. Fixed column grid contracts
  and minimum readability font floors (11–13px) are strictly preserved across all resolutions.
- `layout.scale` remains the actual Windows DPI factor; `metrics.content_scale`
  separately describes the compact notch. Monitor hints and normalized placement
  offsets retain their persistence/drag behavior.

## Verification

Use actual Windows WebView2 on desktop backgrounds: no square black/white fringe, no clipped shoulders, stable outer contact during expand/collapse. Validate native click-through and focus separately from browser screenshot layout. See `docs/testing.md` for the matrix and recorded limitations.
