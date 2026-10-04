# Testing & coverage

## Refactor verification (2026-10-04)

- `npm test`: 148 passed (54 frontend, 57 Rust unit, 16 risk, 21 notch), one live AGY opt-in test ignored.
- `npm run build` and `cargo clippy --all-targets -- -D warnings`: passed.
- Browser scripts: `settings-smoke.py`, `grok-usage-smoke.py`, `detail-layout-smoke.py` passed.
- Isolated Windows/WebView2: `notch-smoke.py`, `notch-regression.py`,
  `notch-edge-drag.py`, `notch-seam-drag.py`, `notch-polish.py` passed.
  Covered four edges, focus/click-through, 20 open/close cycles, pointer threshold,
  failed-write rollback, 10 continuous seam crossings plus six separate drags,
  12 auto-hide/reveal cycles, keyboard access and reduced motion.
- `notch-hover-stability.py --port 9223 --fixture`: 260 stable frames with
  74/112 DIP detail heights on the bottom edge; screenshots visually inspected.
- Native hardware: 100% DPI, primary 2560x1440 and negative-x 1024x1280 secondary.
  Mixed-DPI hardware, unplug/replug and live vendor/updater installation remain untested.
- Legacy native scripts initially failed on removed `.detail-close` / provider-chip
  selectors and old right-click/activity expectations; updated to current UI contracts
  and rerun successfully. No product behavior was changed to accommodate these tests.
- A final test run initially encountered Windows' running-executable lock; the isolated
  preview was closed and the complete test command then passed. Close preview before rebuilding.

**Updated:** 2026-10-01 (v0.2.0)

## Snapshot

| Layer | Location | Purpose |
|-------|----------|---------| 
| Unit | `src-tauri/src/**` `#[cfg(test)]` | quota parsers, snapshot finish, TTL cache / HTTP map |
| Front | `src/ui/format.test.ts` | `formatWindowLabel`, `formatCountdown`, percentage formatting |
| Opacity | `src/ui/opacity.test.ts` | opacity snapping, conversions, meter calc, CSS var styling |
| Grok | `quota/grok.rs` tests | weekly credits, legacy cents, **ignores productUsage**, omitted % + period → **unknown**, explicit zero → **0%** |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Corrupt JSON, AppCore visibility, legacy settings |
| GUI | Manual `npm run tauri dev` / `run:exe` | Glass chrome, Quiet Luxury tracks, hotkey, updater, opacity floors |

## Commands

Controller regressions in `src/ui/notch-surface.test.ts`, `notch-drag.test.ts` and
`update-controller.test.ts` exercise deferred native replies, failed begin/retry,
stale drag notices, capture disposal, updater ordering and asynchronous subscription
cleanup without real timers or vendor requests. Rust `infrastructure::poll::tests`
checks snapshot delivery and worker/event failures. Risk tests exercise real failed
filesystem writes for all preference setters and provider visibility.

Native smoke scripts share `scripts/notch_test_support.py`: they assert the preview
identifier before setting fixture preferences, enable all four providers and hover,
and reset details using Escape. They cover the current provider cards and right-click
toggle behavior. The auto-hide smoke deliberately turns always-show off.
`python scripts/notch-hover-stability.py --port 9223 --fixture` prepares two synthetic
quota cards of different heights in that same isolated preview. Without `--fixture`,
the script retains its read-only preference behavior against the selected CDP port.

With Vite running (`npm run dev`), optional Edge/Playwright browser checks:
`python scripts/settings-smoke.py` verifies fixed settings navigation with body scrolling,
save failure rollback/retry, pending controls, the last-provider lock and updater states.
`python scripts/grok-usage-smoke.py` verifies unknown/zero/nonzero quota rendering.
Both use simulated data and do not change persisted settings or install updates.
`python scripts/detail-layout-smoke.py` reproduces the Grok status-margin overflow and
checks that a single-row detail fits without a scrollbar. Legacy `show_period` values
are ignored and omitted on the next settings save (Rust risk regression).

```bash
# From repo root
npm test                 # vitest + cargo test --lib + risk_scenarios + notch
npm run test:coverage    # scripts/coverage.sh (fail-under 75, business logic)
npm run build            # frontend tsc + vite
```

```bash
cd src-tauri
export TOKENUSAGE_SKIP_DIRECT_QUOTA=1   # avoid vendor HTTP in tests
cargo test --lib
cargo test --test risk_scenarios
```

## Env

| Variable | Effect |
|----------|--------|
| `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` | Direct vendor fetch skipped (unavailable cards in tests) |

Risk tests set this automatically.

## Manual UI smoke (pre-release)

| Check | How |
|-------|-----|
| Low opacity | Settings opacity meter → 35–50%; meta/reset/labels still readable; meter fill stays neutral |
| Live motion | Idle track static; active fill may sheen; critical+active may breathe |
| Settings | No refresh chips; footer shows `vX.Y.Z`; last provider chip locked |
| Autostart (release only) | Install build; Run key should be install path, not `target\debug` |
| Updater | Prior signed build → header ↻ / auto-check to newer `latest.json` |

## Coverage gate

- Tool: `cargo tarpaulin`
- Script: `scripts/coverage.sh` (bash); fail-under threshold for business logic packages
- Optional on Windows: WSL or Git Bash with tarpaulin installed

## Verify before claiming done

```text
npm test
npm run build
# optional: npm run test:coverage
# UI: npm run tauri dev  (Windows preferred)
```

## Edge notch regression (2026-10-01)

`npm test` also runs `src-tauri/tests/notch.rs`: physical edge anchoring,
100/125/150/200% scaling math, negative origins, taskbar fallback, transparent
corners and bridges, settings migration and failed persistence rollback.
`src/ui/notch-state.test.ts` covers pin/settings/Escape, provider removal,
fake-clock close cancellation, missing/zero/overage,
and secondary quota warnings.

Hover stability is a required regression when changing notch/detail geometry.
`changing_detail_preserves_native_canvas_and_notch_position` checks all four edges,
100/125/150/200% scales, start/middle/end placements, one through four providers,
first/last targets, settings, and 78/150/234/400/560 DIP detail heights. The native
canvas and notch must stay identical when opening or changing detail; every detail
must fit and reserved pixels must remain noninteractive when closed.

For the native WebView timing regression, launch a debug app with
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9250`, enable hover
and two providers with unequal detail heights, close settings and unpin details.
Run `python scripts/notch-hover-stability.py --port 9250` with the pointer away.
It alternates visible providers ten times and checks window origin/size and notch
local geometry on every animation frame, including the first frame after switching.
Evidence is saved to `tmp/notch-hover-stability.json` and `.png`. This catches the
one-frame HWND/WebView offset race that final-position assertions alone miss.

Optional Windows smoke: see `windows-dev.md`. Native evidence collected on 100%
displays (2560x1440 primary and negative-x 1024x1280 secondary): hover focus retained,
transparent notch corner routes to another HWND, 20 expand/close cycles, settings
retained when disabling the selected provider, last-provider lock, missing-display
fallback, reduced motion and failed drag save releasing click-through capture.
A real pointer drag moved the widget along its edge; placement was persisted only
after mouse release (offset 0.5 to 0.588).

Bottom docking now explicitly overlays the taskbar at the physical screen edge.
Native mixed-DPI, physical monitor unplug/replug, sleep/resume and auto-hidden
taskbar remain manual
checks; do not infer those passed from the pure geometry tests.

Whole-notch gesture regression additionally covers background and ring drags on
both axes, no persistence before release, drag-click suppression, 2px movement
retaining ring pinning, native right-click (including repeated right-click), and
Shift+F10 settings access. Move/settings buttons and their reserved area are removed.

## Edge transitions

`scripts/notch-edge-drag.py` uses the isolated preview and actual mouse input.
It checks right/top/left/right transitions, corner retention, bottom docking over the taskbar,
release outside the HWND, Escape rollback and immediate capture release while
LMB remains down, ignored stale completion, negative-monitor transfer including
travel along a top edge, failed real drag-save rollback, no arrow-key movement,
and ring containment in horizontal/vertical layouts. PNG captures are saved to tmp/drag-*.png.
Pure Rust tests cover all four edges, 100/125/150/200% math, horizontal and stacked
monitor seams, zero travel and preserving the grab ratio. The physical desktop
used here has a bottom taskbar; physical bottom docking over it was exercised. Mixed-DPI hardware,
monitor unplug and auto-hide taskbar remain separate manual checks.

## Fast shared-monitor seam regression

The old 24..64 DIP entry band could be skipped between native cursor samples.
The selector now also tests the swept segment at the destination entry band.
`scripts/notch-seam-drag.py` verifies 10 continuous crossings and 6 separate drags
between the left monitor's right edge and the right monitor's left edge, at
instant, 0.11s and 0.5s mouse moves. Both directions and persisted targets passed.
Domain tests reproduce a successful first crossing followed by a skipped return
sample, plus vertically stacked displays and 100/125/150/200% geometry.
