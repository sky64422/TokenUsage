# Live Liquid backdrop during drag

User-approved behavior: preserve configured translucency while dragging and keep
the background visibly current, with bounded work. This supersedes the older
drag capture pause requirement. Continue in the existing checkout; release remains
paused while the user evaluates behavior.

1. Change renderer tests from pause/frost to live rail-only capture, unchanged
   filter, serialized 30Hz work, and an immediate fresh frame after release.
   Avoid resetting canvas maps/generations for native position-only layout events.
2. Resolve WGC local request coordinates at worker execution time. Reuse the
   monitor session on same-monitor movement; change only cached source region.
   Reuse equal-sized textures, invalidate cached pixels when the origin changes,
   and return no frame instead of reading old-position pixels when no fresh
   source frame exists. During drag copy only the rail ROI, not hidden detail.
3. Verify fake-clock renderer cadence/stale completion tests, Rust coordinate and
   cache invalidation tests, actual native drag over a controlled background,
   session reuse and screenshot visibility. Run npm test/build/Clippy, update
   docs, and leave the normal real-data debug app available for manual review.

Tradeoff: live dragging costs more than paused capture. The target is continuous
appearance without queued captures or per-move session construction, not zero cost.
No new dependencies, no persistent settings change, no automatic release.

## Verification (2026-11-02)

- Renderer RED→GREEN: drag no longer replaces the configured filter or stops
  capture. Deferred old-orientation response is discarded after relayout.
- Real renderer with fake time: 150 layout hints/1.5s produce 45 captures;
  static 3s remains 3 captures, reduced-motion behavior stays bounded.
- Actual mouse drag: 3.65s, 80 changed paints across two static stripe colors,
  zero same-monitor session restarts, unchanged filter, exact final stripe RGB,
  affinity 0, and a fresh frame after release.
- Recovery: invalid monitor/reload, 20 resets and 40 alternating ROI/generation
  pixel checks pass. Seam geometry: 10 continuous crossings + 6 separate drags.
- Native dark/glass/liquid: stable hover/window geometry and all three settings
  tabs 440 DIP. npm test/build/Clippy and diff check pass.
- Review fix: position-only relayout remains cheap, but real geometry changes
  invalidate pending responses immediately. Retain the old allocation (marked
  invalid) across size changes until a new source frame permits replacement;
  don't interpret an existing session's resize as a first-frame timeout.
- Limits: no whole-system CPU/GPU/power saving claimed against paused capture;
  mixed-DPI/HDR, long-duration sessions and third-party screen sharing remain
  separate validation work. The source pool is still monitor-sized.

## Smoothness follow-up (2026-11-02)

User reported visible background stepping. The initial 30Hz target was too low
for continuous movement; switch drag-only delivery to 60Hz. Cache a monitor-clipped
128px margin and reuse it while the rail stays inside, so a moved ROI can read
the existing desktop frame instead of waiting for a new frame. Send only rail
pixels to the frontend. Recenter outside the margin and retain the existing
generation, monitor-switch and first-frame protections.

Regression: cache containment/movement test observed RED then GREEN. Corrected
the native smooth-mouse fixture from PyAutoGUI's default 50ms steps to 8ms steps;
discard the earlier slow-input timings from comparisons. Identical-input control
(30Hz/no margin): 151 paints/5.06s, p50/p95 34.5/35.5ms; optimized: 301/5.08s,
14.3/21.6ms. These are Canvas paint intervals, not present latency or power proof.
The fake-clock test pauses time between CDP calls so transport overhead cannot
inflate the nominal one-second cadence. Full tests/build/Clippy pass.
