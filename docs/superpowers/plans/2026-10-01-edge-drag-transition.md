# Edge Drag Transition Implementation Plan

Use superpowers:executing-plans inline; final independent review.
Goal: switch dock edges and monitors while dragging, retaining accepted geometry.
Architecture: pure Rust drag selection, existing native cursor loop and controller,
thin commands, frontend capture and state restoration. No new packages.
Spec: ../specs/2026-10-01-edge-drag-transition-design.md

- [x] Add failing domain tests for 2D threshold, corners, all edges, taskbar,
  monitor hysteresis, scaling, anchoring and zero travel; implement NotchDrag in
  domain/notch.rs, using existing calculate_layout to filter candidates.
- [x] Add session-owned begin/finish/cancel in infrastructure/notch_window.rs;
  reuse native cursor loop with bounded main-thread dispatch. Preserve rollback,
  ignore late IDs, sample final pointer, handle release outside and topology changes.
- [x] Replace frontend axis math with native session events. Preserve clicks,
  pinned/settings state, Escape, capture loss and context menu; clear late events.
- [x] Run npm test and npm run build. Native preview: drag all available edges,
  corners and monitors, release away from HWND, Escape and failure recovery.
  Review actual captures and preserve 72/104/50/36 visual geometry.
- [x] Independent code review, fix findings, update docs and restart live widget.

Ruling: continue on the existing feature branch already isolated from main.
User has approved implementation; no additional plan approval is needed.

Execution record:
- Pure domain tests first failed for missing drag behavior; seam and bottom-policy
  regression tests also failed before their fixes.
- User removed arrow-key repositioning and explicitly allowed bottom taskbar overlay.
- Independent review found capture cleanup, seam hysteresis and keyboard persistence
  defects; all corrected and re-reviewed with no remaining important findings.
- npm test: 31 Vitest + 31 library + 11 risk + 13 notch = 86 passed; build passed.
- Native smoke/regression and new notch-edge-drag script passed. Actual desktop
  screenshot tmp/drag-bottom-desktop.png confirms bottom overlaps the taskbar.
- Native mixed DPI, monitor unplug and auto-hide taskbar are not claimed tested.
- No commit, push or release performed for this follow-up implementation.
