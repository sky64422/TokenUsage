# Project Refactor Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for implementation and dispatching-parallel-agents for independent file ownership. Track completed checks below.

**Goal:** Preserve TokenUsage behavior while separating independently changing responsibilities.
**Architecture:** Keep existing IPC and persisted schema. Extract stateful controllers rather than one-call wrappers; reuse existing policy and parsers.
**Tech Stack:** TypeScript/Vite/Vitest, Rust/Tauri 2, Windows WebView2.
**Spec:** ../specs/2026-10-04-project-refactor-design.md (approved by user).

## Global Constraints

- No dependencies, UI redesign, quota policy changes or release/version changes.
- Preserve 5s refresh, fixed native canvas and vendor-specific caches.
- Existing source checkout is the reviewable deliverable. User subsequently authorized commit and push; no release publishing.
- Work on disjoint files concurrently; no global formatter churn.

## Review Focus

- Late surface responses cannot overwrite newer layout requests (Task 1).
- Begin failure/cancel/stale completion cannot retain pointer capture (Task 1).
- Ready update cannot regress on late progress; pending subscriptions are disposed (Task 2).
- File-read failure remains unknown and Grok turn completion invalidates cache (Task 3).
- Persistence failure cannot leave UI rollback inconsistent with memory (Task 3, separate fix).

## Task 1: Notch frontend controllers (primary agent)

Files: `src/ui/app.ts`, new `notch-surface.ts`, `notch-drag.ts`, tests alongside.
Interfaces: surface controller owns `request(input)`, `settled()`, `destroy()` with injected native request and layout/error callbacks. Drag controller owns begin/move/end/notice/session state and DOM binding with injected native begin/finish callbacks; app retains interaction restoration.

- [x] Add async regression tests for latest response, retry, drag cancellation and failure.
- [x] Extract existing request queue and drag lifecycle; integrate app and disposal.
- [x] Run focused Vitest and TypeScript build.
- [x] Verify native drag/hover behavior after integration.

## Task 2: Provider presentation and updater (frontend agent)

Files: `src/ui/notch.ts`, `settings-panel.ts`, new `provider-catalog.ts`, `update-controller.ts` and tests.
Interfaces: catalog exports shared provider label/mark data keyed by `ProviderId`; updater exposes state subscription, action and disposal, accepts IPC/event/timer boundaries. Settings retains existing markup and labels.

- [x] Characterize updater state transitions, retry and disposal including late subscriptions.
- [x] Extract updater logic; unify provider metadata used by notch/settings.
- [x] Run focused tests; report exact new interfaces and touched files.

## Task 3: Rust responsibilities (Rust agent)

Files: `src-tauri/src/domain/{types,notch}.rs`, domain notch drag module; `application/service.rs`, `commands.rs`, `lib.rs`, `infrastructure/{poll,notch_window,activity}.rs`, activity submodules and affected tests.
Interfaces: `AppSettings` provides provider config access and enabled IDs; `poll` provides common refresh/emit operation for boot/poll/show. Preserve existing public notch/activity paths via reexports.

- [x] Reuse baseline parser/geometry tests for mechanical moves; add tests for new boundaries and cache-invalidating evidence.
- [x] Centralize provider settings access and duplicate refresh/emit orchestration.
- [x] Separate activity evidence/file monitoring/runtime and notch drag policy.
- [x] Reproduce settings save failure; correct separately from mechanical moves using save-before-commit policy.
- [x] Run Rust unit/risk/notch tests and Clippy; report evidence and any behavior changes.

## Task 4: Integration and verification (primary agent)

Files: `docs/{ARCHITECTURE,providers,testing}.md`, this ledger and existing smoke scripts only if stale assumptions are demonstrated.

- [x] Inspect combined diff and get independent review; resolve material findings.
- [x] Run `npm test`, `npm run build`, `cargo clippy --all-targets -- -D warnings`.
- [x] Run browser settings/Grok/detail smoke and isolated native notch regression/drag/hover smoke.
- [x] Update architecture and testing documentation with verified scope and limitations.
- [x] Report final outcomes and leave changes reviewable in the working tree.

## Execution record (2026-10-04)

- Task 1: extracted surface/drag controllers and input cleanup; 8 added async/gesture tests pass. Native integration passes.
- Task 2: shared provider presentation and updater controller; 8 added updater tests pass. Settings browser smoke passes.
- Task 3: activity/runtime/parser and drag domain separated without algorithm changes; settings writes now save before committing memory. Two failure regressions reproduced before the fix; accepted Grok invalidation and three refresh delivery/error tests pass.
- Final automatic verification: npm test 148 passed / 1 opt-in ignored; npm run build passed; cargo clippy --all-targets -- -D warnings passed; git diff --check passed.
- Final UI verification: three browser and six native smoke commands passed; see docs/testing.md for counts, hardware and limitations. Native preview was shut down afterward.
- Independent read-only review: no significant introduced regressions. Existing unversioned notch-layout events are outside the surface promise revision guard; native hover stability was verified but this is not a proof of arbitrary native event ordering.
- Execution adjustment: Rust worker stopped on usage limit; primary agent inspected and completed its work, added remaining poll tests and resolved the test-only Clippy type-complexity error.
- Test harness adjustment: obsolete native close/provider selectors, right-click expectation and recent-activity aria assumption were corrected against existing product behavior. A preview-only shared setup now makes fixture preferences explicit.
- No dependencies, persisted schema, IPC names, versions or production styles changed. User subsequently authorized committing and pushing this verified work.
