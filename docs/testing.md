# Testing & coverage

## Theme-switch geometry regression (2026-10-17)

Run `python scripts/theme-transition-smoke.py --port 1524 --token` with Vite on 1524. It opens a short detail then settings, records all directed theme changes (including the first), holds saves pending, then rejects a save and immediately switches all settings tabs. The virtual-clock cases cover DPR 1/2 and content scale 1/0.92; they assert fixed notch/icon/card/tab/scroll rectangles, rollback selection and the absence of save notices.

Normal Windows theme changes were already geometrically stable. A failed save's inline error did increase the shared height after a tab switch (440→460 px at full scale). That status now shares the existing theme-label row, keeping idle height unchanged and preserving full status text for assistive technology. Both this test and `scripts/settings-smoke.py` pass. Six native Windows theme changes also preserve the HWND, notch/icons and settings tabs; cross-project evidence is at `C:/dev/EconomyWarRoom/tmp/native-theme-jitter.json`. Only the isolated preview was used for TokenUsage native writes; the installed app was left alone.

## Adaptive backdrop and unchanged displays (2026-10-17)

Glass and Liquid capture at 30 Hz while changing, 10 Hz after 500 ms unchanged, and 4 Hz after 2 s unchanged. Dragging retains 60 Hz; reduced motion retains 4 Hz. Detail/reveal/drag transitions reset the cadence. Provider polling remains fixed at 5 s. Equal notch values/attributes skip DOM writes, and equal quota display data plus running state preserve the card DOM while retaining incoming snapshot timestamps. The independent countdown timer still updates reset text and urgency. The small black background behind notch percentages has been removed; text sizing and spacing are preserved.

With Vite on 1524, run `python scripts/display-update-smoke.py --port 1524 --token`, `python scripts/liquid-cadence-smoke.py --port 1524` and again with `--theme glass`, plus `python scripts/liquid-renderer-smoke.py --port 1524`. Virtual-clock checks cover three worst-phase stop/resume cycles (first new pixels within 240 ms), 4/30/60 Hz capture, detail reopening, fold/reveal and countdown becoming urgent while timestamp-only snapshots preserve the card. Both full `npm test` and `npm run build` pass.

Windows Liquid was also tested in the separate `com.tokenusage.notch-preview` profile with a small temporary striped window and three stop/resume cycles: first changes appeared in 79–121 ms, subsequent detail paints were about 30 Hz, settling to 8 total reads/s for two surfaces. No JS long task was observed in those short cycles. Pattern windows were closed and the separate preview was stopped; the installed app was left alone. Evidence: `C:/dev/EconomyWarRoom/tmp/adaptive-native-results.json`, local `tmp/liquid-cadence-liquid-results.json` and `tmp/liquid-cadence-glass-results.json`. These are short debug/renderer measurements, not presented-FPS, power or long-term leak claims. Wait for asynchronous detail placement before positioning a native test backdrop and keep process enumeration outside the sampling window.

## Glass/Liquid update (2026-10-17)

Zero refraction is now the permanent Liquid material. The renderer skips the SVG filter URL and distance/displacement-map allocation, while retaining capture, blur, saturation, brightness and the existing reflected rim. Glass still uses its own refraction. Both real-renderer lifecycle fixtures enforce no map builds/no SVG filter for Liquid and restore the Glass lens when switching. Both full suites and production builds pass; native debug inspection confirms a blur-only filter with no lens-image references.

A short headless Edge A/B (baseline zero-strength SVG versus bypass, ABBA order, two six-second samples per 30/60 Hz workload, synthetic changing pixels, two surfaces) measured whole-machine-normalized browser-group CPU 2.12% -> 1.52% at 30 Hz and 2.87% -> 1.63% at 60 Hz. These are renderer-only workload reductions, not native whole-app CPU/power savings; desktop capture is not part of this fixture. A separate static screenshot comparison with animations disabled differs by at most 2/255 per channel (mean 0.0146/255), consistent with SVG color rounding. Evidence: `C:/dev/EconomyWarRoom/tmp/liquid-zero-benchmark.json` and `liquid-zero-pixels.json`.

Latest visual adjustment: Glass notch rims now use Liquid's layered 3px reflective band plus a 0.7px specular edge, with a neutral high-contrast mirror gradient. Liquid's displacement is fixed at 0: SVG refraction and displacement-map generation are bypassed; blur and the colored rim remain. Both renderer smokes and production builds pass after the change; native notch screenshots were inspected (`tmp/final-glass-mirror-notch.png`).

The final cache also keeps a reusable original BGRA buffer: an exact key/source match skips per-pixel conversion as well as IPC and painting. Reserved-alpha changes still use the alpha-insensitive RGBA comparison. Complete suites and coverage were rerun successfully. Fresh Liquid samples after this refinement are in `C:/dev/EconomyWarRoom/tmp/liquid-fast-cache-performance.json`; prior tables are earlier snapshots.

Current verification: EconomyWarRoom 195 tests, TokenUsage 166 tests (one live vendor test ignored), both production frontend builds pass. EconomyWarRoom business coverage is 94.23%. Native first-open settings heights are 434/440 DIP respectively; raw RGBA and legacy BMP work, four-edge card changes keep the canvas fixed, automatic fold restores capture affinity, and Dark theme exit releases it. Native reference-material screenshots were visually inspected.

Eight-second static samples on the existing desktop: Liquid CPU 1.95/3.01/4.41% for EconomyWarRoom rail/detail/settings and 1.82/2.42/3.22% for TokenUsage. Private commit was approximately 265-339 MiB for the live materials. Actual high-rate drag produced about 50-54 changed background paints/sec and 10-11% total-machine CPU in debug builds. rAF p99 was 7.1 ms at 144 Hz; this is not compositor-presented FPS. Earlier CPU/memory runs are not a controlled A/B comparison. Full samples, limitations and transient harness observations: `C:/dev/EconomyWarRoom/tmp/liquid-final-review.md` with `liquid-final-performance.json` / `liquid-final-drag.json`.

Glass now uses the supplied neutral lock-screen reference. Liquid keeps its original material CSS and optics; both share the optimized capture worker. The tray has Show/Quit only; application keyboard handlers (including Escape drag cancellation and custom tab navigation) are removed. Native smoke reset helpers now use mouse controls. Earlier keyboard and painted-Glass evidence is historical.

`npm test` includes raw RGBA/cache regressions. `python scripts/liquid-renderer-smoke.py --port PORT` with Vite on that port checks unchanged-frame suppression, a deliberately delayed detail frame across close/reopen, enabled Glass/Liquid switching, explicit visibility buffer release/resume and destroy. The delayed-frame case failed before detail-generation invalidation and passes after the fix. Both projects pass this real-renderer fixture without page errors. It mocks native pixels and does not prove desktop capture by itself.

Windows validation uses isolated app profiles, existing desktop pixels and no fullscreen test backdrop. Raw RGBA avoids per-frame Blob/BMP/image decode; the dedicated worker reuses two DIB buffers and returns empty packets for unchanged pixels. The legacy BMP command remains available to diagnostics. Capture affinity stays excluded during pending reads and restores on leaving both live materials, reload or stop.

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

**Updated:** 2026-10-05 (v0.3.6)

## Snapshot

| Layer | Location | Purpose |
|-------|----------|---------| 
| Unit | `src-tauri/src/**` `#[cfg(test)]` | quota parsers, snapshot finish, TTL cache / HTTP map |
| Front | `src/ui/format.test.ts` | `formatWindowLabel`, `formatCountdown`, percentage formatting |
| Opacity | `src/ui/opacity.test.ts` | opacity snapping, conversions, meter calc, CSS var styling |
| Grok | `quota/grok.rs` tests | weekly credits, legacy cents, **ignores productUsage**, omitted % + period → **unknown**, explicit zero → **0%** |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Corrupt JSON, AppCore visibility, legacy settings |
| GUI | Manual `npm run tauri dev` / `run:exe` | Glass chrome, Quiet Luxury tracks, auto-fold/reveal, updater, opacity floors |

## Commands

### Codex token details and quota-track regression (2026-10-18)

With Vite running, `python scripts/token-details-smoke.py` checks Today / This month, model metadata, persistent expansion/selection/focus, provider marks,
zero/missing/partial/error data, absence of provenance/status/footnote copy,
escaping, three themes and scaled widths.
It also checks model-share shades in the main quota bars, matching dots/abbreviations/percentages
on one legend line, unchanged vendor width/percentage, stale-reset fallback,
independence from calendar tab selection, legend focus while numeric data changes, tab keyboard navigation
and the two-tab selection indicator width under reduced motion. Tiny 1% quota
fills with three models must retain positive segment widths and the vendor percentage. `python
scripts/notch-ring-smoke.py` checks one continuous ring with no secondary ring,
including unknown/zero/full/overage and running activity.
`python scripts/usage-track-smoke.py` compares empty-track pixels across zero,
partial and running usage: sheen and fill light must stay inside actual usage.
All three accept `TOKENUSAGE_TEST_URL` for an isolated Vite port and use synthetic
metadata, never live transcripts or OS preferences.

Rust token tests cover cumulative increments, copied/forked histories, replacement,
partial writes, Monday/week/month boundaries, oversized lines, resumable discovery and fairness.
Regression cases cover events written just after scan time (also across a month
boundary), activation without replay or double-counting, and record-limit coverage
expiry/cache invalidation without falsely clearing same-period warnings.
Limits per background pass: 512 discovery entries, 8 MiB total reads, 1 MiB per
file, 64 file attempts; at most 4096 files and 100000 retained metadata records.
Recent files, including unread sessions, share a reserved 2 MiB allowance while background paths
keep rotating. Retained incomplete-line allocation is capped at 8 MiB total.
Resource regressions cover deleted-file slot reuse, expired owner cleanup,
discarded-buffer release, aggregate buffer limits, recent-file latency, idle
summary reuse and activation/expiry/date/revision invalidation. A cold-start
regression verifies current usage is admitted before the old archive drains.
Fingerprints
cover replacement detection without retaining raw samples, including appending
to a short file without restarting its scan. No-change workers suppress extra
refresh notifications.
Retention includes the current month and preceding seven days for vendor quota windows.
Excluded or uncertain coverage is partial; ongoing scans are loading. Large logs
may take multiple passes. Preview mode disables real Codex token reads as well
as direct quota reads.

Vertical-edge detail expansion keeps its header anchor while there is room
below and shifts upward only at the bottom work boundary. The notch/canvas must
remain fixed. Drag regression coverage also verifies that rail-local changes
inside a fixed full-height canvas emit frontend layout updates before release;
a plain native translation with unchanged local coordinates needs no repaint.

Controller regressions in `src/ui/notch-surface.test.ts`, `notch-drag.test.ts` and
`update-controller.test.ts` exercise deferred native replies, failed begin/retry,
stale drag notices, capture disposal, updater ordering and asynchronous subscription
cleanup without real timers or vendor requests. Rust `infrastructure::poll::tests`
checks snapshot delivery and worker/event failures. Risk tests exercise real failed
filesystem writes for all preference setters and provider visibility.

Theme risk tests cover dark defaults for old settings (including retired system/light
values), glass/dark persistence across an AppCore restart, and failed-write rollback.
`settings-smoke.py` also covers pending theme controls, failed-save selection rollback
and a successful retry. Compare native dark/glass rendering and pointer reflections
in the isolated preview; neither theme may change notch placement or input regions.

Liquid tests cover theme persistence, bounded capture regions, and the deferred-frame
race when switching away and back before capture finishes. Native verification must
use an external patterned window (a DOM backdrop cannot prove desktop refraction),
check absence of foreground in the captured source, Dark capture rejection,
reload restoration, and actual drag cadence. Do not infer 60 fps from the configured
frame interval alone.

Native smoke scripts share `scripts/notch_test_support.py`: they assert the preview
identifier before setting fixture preferences, enable all four providers and hover,
and reset details using mouse controls. They cover the current provider cards and right-click
toggle behavior. The auto-hide smoke deliberately turns always-show off.
`python scripts/notch-hover-stability.py --port 9223 --fixture` prepares two synthetic
quota cards of different heights in that same isolated preview. Without `--fixture`,
the script retains its read-only preference behavior against the selected CDP port.

With Vite running (`npm run dev`), optional Edge/Playwright browser checks:
`python scripts/settings-smoke.py` verifies fixed settings navigation with body scrolling,
save failure rollback/retry, pending controls, the last-provider lock and updater states.
It applies the production `getContentHeight()` result rather than a fixed fixture height.
At full/compact size and after returning to full size, all three tabs must have the same
height, repeated measurements must remain stable, and unconstrained content must fit
without scrolling. A 200px constrained panel must keep its header fixed while the body
scrolls. This is a user-required regression: do not restore active-tab-only sizing.
Also start from short 78px/108px quota-card heights and assert the very first measured
settings height already matches the shared height. Native checks must include opening
settings after quota details before switching from appearance to services; a warmed-up
tab loop alone misses the initial undersized panel.
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
| Settings | 3-tab layout ('모양', '서비스', '일반'); unified '작업 중 강조 효과'; locked last provider; docked footer ('로그 복사', '종료') |
| Autostart (release only) | Install build; Run key should be install path, not `target\debug` |
| Updater | Prior signed build → Settings > 일반 > 앱 정보 '업데이트 확인' / auto-check to newer `latest.json` |

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
`src/ui/notch-state.test.ts` covers pin/settings/close transitions, provider removal,
fake-clock close cancellation, missing/zero/overage,
and secondary quota warnings.

Hover stability is a required regression when changing notch/detail geometry.
`changing_detail_preserves_native_canvas_and_notch_position` checks all four edges,
100/125/150/200% scales, start/middle/end placements, one through four providers,
first/last targets, settings, and 78/150/234/400/560/900/2400 DIP requested detail heights. The native
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
