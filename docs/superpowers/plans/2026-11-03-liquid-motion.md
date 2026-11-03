# Liquid Motion Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make fast background scrolls visible promptly without stretching boundary pixels or continuously transferring unchanged images.

**Architecture:** Signal the bounded native worker on WGC frame arrival. Keep fresh GPU pixels, filter relevant dirty regions, coalesce frontend hints, and read back only new/request-invalidated surfaces. Extend captured pixels by blur support and keep native geometry unchanged.

**Tech Stack:** Existing Rust/windows 0.61, Tauri 2, TypeScript Canvas, Vitest and Python native smoke tools; no new dependencies.

**Spec:** ../specs/2026-11-03-liquid-motion-design.md

## Global Constraints

- Active ceiling 60 Hz; reduced motion 250 ms; static recovery 1 s.
- Preserve opacity, notch shape, physical placement, shared settings height, screen-sharing exclusion and drag rendering.
- No installation, release, version bump or automatic commit; deliver reviewable workspace changes.

## Review Focus

- Reset/theme switch while frame callback runs: discard stale generations and remove handlers.
- Unsupported dirty metadata: conservative refresh, no silent frozen image.
- Negative monitor origins and actual monitor edges: bounded padding with exact dimensions.
- Hidden detail/folded notch: no renderer capture loop when invisible.
- Outside-region animation: avoid unnecessary readback, measure residual capture/notification cost.

## Task 1: Establish native baseline

- [x] Extend native verification with a fixed-window pixel-only pulse and moving scrollbar fixture; identify preview by identifier and executable.
- [x] Save before results and images under tmp; baseline must demonstrate delayed or missed pulses.

## Task 2: Frame arrival and current-frame scheduling

Files: src-tauri/src/infrastructure/{liquid_capture,liquid_wgc,liquid_events}.rs, src/ui/liquid-glass.ts and relevant tests.

- [x] Add failing tests for region intersection, same-frame reuse/invalidation and 60 Hz coalesced hints.
- [x] Register/remove FrameArrived, signal the existing worker without blocking callbacks, retain bounded latest GPU state and expose existing liquid-backdrop-changed hints.
- [x] Read only when pixels or request key changed; safely fall back when dirty rectangles are unsupported.
- [x] Run targeted Vitest/Rust tests and inspect results.

## Task 3: Real blur support

Files: src/ui/liquid-glass.ts, src-tauri/src/infrastructure/{liquid_glass,liquid_capture,liquid_wgc}.rs and relevant tests.

- [x] Add failing tests for padding requests, monitor-clipped sampling and unchanged logical surface geometry.
- [x] Add optional bounded padding to capture command; preserve unpadded callers. Expand WGC cache coverage, copy real surrounding pixels and replicate only at actual monitor boundaries.
- [x] Replace renderer edge stretching with drawing the padded image at its natural offset. Run targeted tests.

## Task 4: Native verification and delivery

- [x] Run npm test, npm run build, npm run test:ui and cargo clippy.
- [x] Compare native pixel-only pulse/motion and outside/idle resource measurements using the same fixture.
- [x] Inspect native screenshots; run settings, WGC recovery and drag regressions; document scope and measured limits in docs/testing.md and a validation note.
- [x] Review full diff, address material findings and report workspace, results and unverified limits.

## Execution ledger

- Initial state: 98a85c5, clean main. Working in C:/dev/TokenUsage-liquid-motion, branch fix/liquid-motion.
- Ruling: user requested planning and execution after approving the reviewed approach; continue without a second approval loop.
- Ruling: retain existing dependencies and shared build cache; installed release process is not modified.

- Task 1: complete. Native baseline missed 6/6 brief pulses; ~14 paints/s; no real outside-edge pixels.
- Task 2: complete. Frame-arrival worker signals, metadata filtering, source/request reuse and 60 Hz hint recheck. Relevant Vitest and Rust regressions failed before fixes, then passed.
- Task 3: complete. Real padded pixels, separately bounded high-DPI buffers, max support reserved initially, monitor-clipped drag reuse, inactive-surface hints cleared.
- Task 4: complete. 299 tests passed (+1 ignored), build/Clippy/UI9 passed; native pulse/motion, drag, settings, recovery and screenshot checks passed. See ../../liquid-motion-validation.md for measured costs and limits.
- Ruling: requested-key cache identity and raw BMP behavior are preserved; diagnostic callers without an active mask default to both surfaces.
- Ruling: fresh reviewer findings were fixed with regression checks. Motion resource cost is higher than baseline and is explicitly reported; no power saving is claimed.
- Delivery: verified changes are copied into the unchanged primary checkout as uncommitted edits; keep this isolated workspace and evidence for review. No commit, push, install or release.
