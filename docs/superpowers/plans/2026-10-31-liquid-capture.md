# Liquid capture implementation plan

**Goal:** Reduce idle wakeups and react promptly to desktop changes.
**Architecture:** Retain bounded GDI capture/cache. Schedule one timer before each capture, coalesce refreshes during in-flight work, and accelerate when changed pixels arrive. Windows window events invalidate the backdrop; a one-second heartbeat remains for changes without window events.
**Tech stack:** Existing TypeScript, Rust, Tauri and Windows APIs.
**Spec:** User-approved approach in the preceding discussion; preserve drag pause, capture exclusion release, opacity and geometry contracts.

- [x] Test and implement timer scheduling, coalescing, disable/restart and stale frame disposal in liquid-loop.
- [x] Update liquid-glass cadence from actual painted frames, expose event refresh, preserve reduced motion and drag pause.
- [x] Add bounded Windows event invalidation with own-process filtering and overlap checks; retain heartbeat fallback.
- [x] Make capture file logging opt-in; document diagnostics.
- [x] Run npm test, npm run build, deterministic renderer and isolated native checks. Review final diff.

Review focus: refresh storms must not starve timers; events during capture must coalesce; stopped renderers must remain stopped; moved-away windows must invalidate old coverage; capture affinity must be restored.

Execution: continue in this checkout to preserve the user's existing uncommitted drag fix. No commit or release requested.

Review: fixed rolling capture-exclusion renewal and dropped trailing invalidation. Native screenshot harness now selects the Tauri class rather than a same-title Shell proxy.

## WGC copy optimization follow-up (2026-11-01)

User authorized reducing whole-monitor copy/storage and reusing rail/detail staging buffers. Keep the default GDI backend and debug-only WGC gate.

- Ruling: retaining one checked-out WGC frame was rejected after native comparison. With a two-buffer pool, observed latency rose to p50/p95 65/73ms; immediate-return control measured 31/37ms. Do not trade responsiveness for memory savings.
- Ruling: copy the stable native window canvas intersected with the monitor, not only currently requested surface rectangles. This includes unopened detail regions, so a quiet desktop can satisfy a new ROI or generation without waiting for another source frame. Cost: some transparent canvas pixels are copied too. WGC's own monitor-sized pool remains.
- Keep source frame Close on every successful acquisition, including copy errors. Retain only the cropped private texture. Keep two independently sized staging textures.
- Verify: known-color static ROIs at distinct positions/sizes across 40 generations; reset/error recovery; OS screenshots and independent WGC observer; actual event-to-pixel latency; native hover/settings; before/after CPU, memory and GPU samples; npm test/build.
- Complete: cropped canvas + per-slot staging implemented. Static ROI pixels 40/40, OS 300/300, independent WGC 200/200, latency p50/p95 31/63ms, native themes/settings, 8 monitor-edge placements, 10 reloads and 60.7s observation passed. npm test/build passed. Fresh review identified the held-frame latency and insufficient same-position pixel check; both were addressed by the final copy strategy and distinct-position RGBA fixture. No commit/release requested.
