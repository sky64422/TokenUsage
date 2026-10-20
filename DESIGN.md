# TokenUsage visual system — edge notch

Updated: 2026-10-20 (v0.5.2). Implements the user-approved [edge notch design](docs/superpowers/specs/2026-09-30-edge-notch-design.md). Windows is the native validation target.

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
- Liquid clarity (2026-10-19): one light translucent wash over the native backdrop;
  no stacked opaque panel/body fills while the material is ready. Bright edge
  reflections define the surface. High-alpha white labels and soft local shadows
  preserve foreground readability over bright backgrounds without
  darkening the whole pane. Retain the opaque fallback when native material fails.
- Liquid depth: use neutral alternating highlights/shade along the rim instead
  of a uniform rainbow stroke, with soft inset reflections and a clear centre.
  User-approved prism accents are restrained cyan/blue/pink highlights confined
  to short rim segments and a faint inner edge; never a rainbow wash across content.
  The convex edge combines a soft inner band with a sharper rim highlight;
  detail panels use asymmetric inset light/shade without changing their geometry.
  Native blur is 3 DIP on the rail and 10 DIP on details. Text retains higher
  alpha floors with a soft shadow and no dark stroke to avoid an embossed appearance on bright
  screens. This is compositor blur and painted edge reflection, not pixel-warp
  refraction; do not reintroduce desktop capture exclusion for a lens effect.

## Details and settings

- Drag any part of the notch (including rings) to move or dock on another edge/display. A 5 DIP two-dimensional threshold preserves ring clicks. New edges engage within 64 DIP with 16 DIP hysteresis; monitor seams use 24 DIP entry depth. There are no move/settings buttons; right-click the notch to open settings. App-specific keyboard controls are removed; mouse release commits once.

- Hover/focus opens inward; click keeps a provider open. Click the selected icon again to close a pinned detail; use the settings close button to close settings.
- The provider whose detail is open stays highlighted while crossing into the panel; pinned state uses a stronger border. Grouped quotas show the service name above model groups so AGY's Gemini/Claude rows retain their source identity.
- 180ms leave grace permits crossing into the detail. 160ms reveal; reduced-motion disables motion.
- Detail cards use a near-black surface, 16px radius, thin quiet border, 16px padding. Native width 260 DIP; height follows intrinsic content up to the current work area (user-requested 2026-10-18). The native canvas reserves that capacity from the start so token expansion never moves the notch/window. Vertical-edge cards keep a height-independent header anchor and grow downward first, shifting upward only when the bottom work boundary requires it. Top-edge cards expand down; bottom-edge cards use the available space above. Scroll only when content exceeds the work area.
- Model shading belongs in the **main quota bars above token details** (user correction 2026-10-18), with one dot/short-name legend directly below each bar. Keep vendor fill width/% and its risk hue; largest recorded model usage gets the darkest shade, smaller amounts progressively lighter shades of that hue. Model blocks use 1px corners and normally 2px internal gaps within the unchanged quota fill width; total gap width is capped at 25% of the fill so tiny nonzero usage remains visible. Legends use compact 10px text placed close to the bar; show only dots and names; omit model percentages (user request 2026-10-19), without leading source/status copy (explicit user request). Accessible labels explain the local token composition for the matching reset period; it is not a vendor-reported model quota. Hide shading if data/reset is missing or stale. Legends scroll horizontally on overflow.
- Token details omit provenance/update time, status copy and explanatory footnotes (user request 2026-10-19).
- Token overview (2026-10-20): aligned `오늘 / 어제 / 최근 30일` rows with total tokens and secondary cache-read share; 30-day fixed daily spark bars (rolling 30-day window) with blue bars for recorded usage, zero distinct from unknown. Spark bars offer micro-hover feedback (brightness, subtle glow, 1px lift). The summary chevron features an ergonomic 32x18px pill hover affordance for reliable fold/unfold clicks. Clear visual hierarchy: period & trend labels use prominent `--foreground` (font-weight 600), while right-side values use muted grey `--muted-foreground` (`tokens · %`, omit literal '캐시' label, preserve percentage); the trend label and spark bars share one row. No bottom model breakdown or period tabs.
- **Settings height is shared across all three tabs (user-confirmed 2026-10-17).** Measure the tallest tab's intrinsic content plus actual surrounding chrome. Switching tabs on the same monitor must keep the panel height fixed; never resize from only the active tab. Recompute the shared height for monitor/DPI changes. Smaller work areas retain body-only scrolling with a fixed header and tabs. The 440 DIP value is only a fallback before measurement. Change this contract only at the user's explicit request. Action buttons use 32 DIP targets and 8px corners. A pinned provider has a quiet rounded background and border distinct from hover.
- Existing detail rows keep name / period / refill and `1fr` bar / `2.9em` percentage columns, with 2px gutter. Dual and grouped rows use compact `--usage-row-gap: 8px`. Quiet Luxury tracks use 7px height (slightly increased from 6px at user request 2026-10-18) and rectangular 1px corners across both single bars and model segments (user correction 2026-10-20). Sheen and fill light stay clipped to the filled portion; running activity must not add a halo or blur to the empty track.
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
- Glass uses neutral smoked translucency and a mirrored 3px rim with a 0.7px specular edge. Liquid retains translucent chromatic dispersion, reflections, and dark text scrims.
- Windows DWM disables `HostBackdropBrush` on inactive/unfocused windows (`WS_EX_NOACTIVATE`), which causes always-on-top monitors to render as opaque dark gray. Therefore, Glass and Liquid sample the desktop behind the notch via a high-efficiency background GDI worker with temporary `WDA_EXCLUDEFROMCAPTURE` leasing (auto-released after 750ms idle) and render blurred backdrops to dedicated surface canvas layers.
- Frame capture uses adaptive throttling (1-4 Hz idle, 12 Hz drag, 24 Hz settling) taking <0.1% CPU. Unchanged frames skip re-rendering.
- Liquid foreground contrast samples the background pixels to toggle adaptive dark/light contrast (`data-contrast="light"`) for legibility across light and dark wallpapers.
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
