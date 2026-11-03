# Liquid background motion

The user approved improving missed fast scroll motion and apparent scrollbar stretching while preserving the Liquid material. Implement and validate in an isolated preview; no release is requested.

## Approach

Use WGC FrameArrived to wake the existing bounded capture worker and retain only current pixels. Use dirty rectangles when supported to avoid waking the renderer for changes outside its sampled areas; absent metadata must conservatively refresh. Source frames must be returned promptly. Keep periodic recovery reads, single-flight renderer requests, generation invalidation, per-session HWND exclusion and failure fallback.

Active rendering is capped at 60 Hz, reduced motion remains 250 ms, static recovery remains 1 s. This is a ceiling, not a guaranteed display rate. Do not queue historical frames or blend old/new images.

Capture actual surrounding desktop pixels for the blur footprint (three times the existing maximum blur radius), preserving the existing blur/opacity effect. Clamp only at real monitor boundaries, where edge replication is necessary. The native window geometry and input region do not change.

Alternatives considered: constant 60 Hz polling is simpler but increases idle readback cost; GPU-native presentation could reduce copying but requires a substantially larger rendering change. Event-driven WGC with bounded readback is the selected direction from the preceding review. Implementation details are agent decisions.

## Evidence required

Reproduce pixel-only changes after idle without moving a background window. Compare response and moving-bar sampling before/after. Check a bar just outside the notch is included in blur support, and inspect native screenshots. Measure idle, inside-motion and outside-motion CPU/private memory and raw GPU counters where available. Run unit/build/UI, settings height, WGC recovery and live drag checks. Record limitations rather than infer presentation latency from IPC arrival.
