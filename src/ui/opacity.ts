/** Opacity scrubber uses whole percent steps of 5 (35%…100%). */
export const OPACITY_MIN_PCT = 35;
export const OPACITY_MAX_PCT = 100;
export const OPACITY_STEP_PCT = 5;
/** Intervals between min and max (35→40 … 95→100). */
export const OPACITY_INTERVALS =
  (OPACITY_MAX_PCT - OPACITY_MIN_PCT) / OPACITY_STEP_PCT; // 13

export function snapOpacityPct(pct: number): number {
  const clamped = Math.min(OPACITY_MAX_PCT, Math.max(OPACITY_MIN_PCT, pct));
  return Math.round(clamped / OPACITY_STEP_PCT) * OPACITY_STEP_PCT;
}

export function opacityToPct(o: number): number {
  return snapOpacityPct(Math.round(o * 100));
}

export function pctToOpacity(pct: number): number {
  return snapOpacityPct(pct) / 100;
}

/** How many 5% steps above min (35% → 0, …, 100% → 13). */
export function opacityStepIndex(pct: number): number {
  return (snapOpacityPct(pct) - OPACITY_MIN_PCT) / OPACITY_STEP_PCT;
}

/** Fill width aligned to 5% cells. */
export function meterFillPct(pct: number): number {
  return (opacityStepIndex(pct) / OPACITY_INTERVALS) * 100;
}

/**
 * Glass opacity + matching text/graph alpha.
 * Background uses --panel-opacity; fg/accent/chrome track the slider so bars
 * and labels don't stay fully solid while the panel goes transparent.
 */
export function applyPanelOpacity(panel: HTMLElement, opacity: number): void {
  const o = Math.min(1, Math.max(0.35, opacity));
  const fg = Math.min(1, Math.max(0.4, o * 0.94 + 0.04));
  const accent = Math.min(1, Math.max(0.36, o * 0.96 + 0.02));
  const chrome = Math.min(1, Math.max(0.28, o * 0.9 + 0.04));
  const tint = chrome;

  const root = document.documentElement;
  for (const el of [panel, root]) {
    el.style.setProperty("--panel-opacity", String(o));
    el.style.setProperty("--fg-opacity", String(fg));
    el.style.setProperty("--accent-opacity", String(accent));
    el.style.setProperty("--chrome-opacity", String(chrome));
    el.style.setProperty("--tint-strength", String(tint));
  }
}
