# Liquid motion validation — 2026-11-04

## Implementation

WGC FrameArrived signals the existing bounded worker. Source frames are returned
promptly; dirty rectangles filter renderer hints, with conservative refresh when
the metadata is unsupported. The renderer coalesces hints, has a 60 Hz active
ceiling, and retains its 1 s static recovery heartbeat and 250 ms reduced-motion
interval. It does not queue historical images. Unchanged source/request pairs
skip readback; closed detail surfaces stop contributing refresh hints.

Liquid samples real surrounding desktop pixels for blur. Only actual monitor
edges repeat pixels. Maximum detail support is reserved at the first rail request
so a static desktop can supply the first opened detail. Legacy callers growing
the reserve start a fresh session. The original content limit remains 1,048,576
pixels; padded buffers are bounded at 4,194,304 pixels on both sides of IPC, with
at most 512 physical pixels of support per side. Native window geometry, input
regions, opacity/filter, quota polling and shared settings height are preserved.

## Native results

Baseline: `98a85c5`. Tests used `com.tokenusage.notch-preview`, a four-provider
rail and a fixed Tk background window. Pixel-only changes never move that window.
These are local debug observations, not GPU presentation or release-build results.

| Check | Before | Final |
| --- | --- | --- |
| Six 150 ms pulses after idle | 0/6 observed | 6/6, arrival 23–39 ms |
| Moving bar canvas paints | 14.0/s | 42.7/s |
| Bar outside notch, within blur support | 0 red pixels | 100 real red pixels |
| First detail after idle | — | Captured layer ready |

Other checks:

- Live pointer drag: 232 paints/5.11 s, actual two-color changes, zero same-monitor
  session restarts, unchanged filter, correct final color, release repaint and
  display affinity 0. Browser drag geometry: 72 samples.
- Recovery: off-monitor failure/restoration, 20 concurrent resets, 40 alternating
  ROI/generation requests with padding 0/2/4, two repeated raw BMP snapshots.
- Dynamic OS screenshots: notch visible 100/100, affinity 0. Background-event
  arrival p50/p95 33/46 ms over 20 samples. This is not external conferencing proof.
- Native dark/glass/liquid settings tabs: 440/440/440 DIP, stable window/rail.
- Browser/native image artifacts were visually inspected.

Evidence under `tmp/`: `liquid-motion-{before,final}.json`,
`liquid-live-drag-result.json`, `liquid-events-native-results.json`,
`liquid-wgc-capability/recovery-result.json`, `native-quality-results.json`,
`liquid-sharing-visible.png`, and `liquid-motion-final.png`.

## Resource tradeoff

Approximately six seconds per workload; CPU includes the app and WebView children
as a fraction of all 12 logical CPUs. Different-time short samples are not a
statistically controlled power comparison.

| Workload | Baseline CPU | Final CPU | Baseline/final private memory |
| --- | ---: | ---: | ---: |
| Static | 0.122% | 0.142% | 281.8 / 301.5 MiB |
| Moving background window | 2.353% | 3.643% | 275.7 / 324.0 MiB |
| Fixed window, pixel changes | 1.623% | 3.122% | 283.3 / 357.8 MiB |
| Unrelated pixel changes | 0.123% | 0.185% | 282.3 / 335.3 MiB |

Static/unrelated workloads produced zero canvas paints. Faster, wider capture
costs more than the former low-frequency path; this is not a claim of lower total
resource use. Bulk edge copying reduced the initial implementation's active
cost. Pixel changes produced 59 baseline versus 86 final paints, so work/output
is not equal. Monitor-wide WGC still incurs work outside the notch.

Raw GPU data: `liquid-backend-motion-{before,final}.json`. App engine percentage
sums were 0.010/0.003 (static), 0.364/0.518 (moving), 0.246/1.918 (pixel changes),
0.010/0.000 (unrelated). These are overlapping engine sums, not whole-GPU use or
energy consumption; DWM CPU was unavailable. No GPU/power saving is established.

## Gates and limits

`npm test`: 299 passed, one live-provider test ignored. `npm run build`,
`cargo clippy --all-targets -- -D warnings`, and `npm run test:ui` (9/9) passed.
Regression failures were observed before fixes for padding requests, dirty-region
coordinates, high-DPI buffers, hint timing and monitor-edge drag reuse.

Real mixed-DPI/HDR, denied capture policy, device loss/sleep, long-term resource
stability and release-build power remain unverified. The 200% tall-detail case is
covered by native/TypeScript unit tests, not physical mixed-DPI hardware. Changes
shorter than a capture/display interval may still be skipped.
