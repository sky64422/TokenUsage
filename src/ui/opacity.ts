/** Opacity scrubber uses whole percent steps of 5 (0%…100%). */
export const OPACITY_MIN_PCT = 0;
export const OPACITY_MAX_PCT = 100;
export const OPACITY_STEP_PCT = 5;
/** Mirrors Rust `OpacityPolicy::MIN` (physical readability floor). */
export const OPACITY_MIN = 0.35;
/** Mirrors Rust `OpacityPolicy::DEFAULT`. */
export const OPACITY_DEFAULT = 0.92;
/** Intervals between min and max (0→5 … 95→100). */
export const OPACITY_INTERVALS =
  (OPACITY_MAX_PCT - OPACITY_MIN_PCT) / OPACITY_STEP_PCT; // 20

export function snapOpacityPct(pct: number): number {
  const clamped = Math.min(OPACITY_MAX_PCT, Math.max(OPACITY_MIN_PCT, pct));
  return Math.round(clamped / OPACITY_STEP_PCT) * OPACITY_STEP_PCT;
}

/** Maps backend physical opacity [0.35, 1.0] to user-facing percentage [0%, 100%]. */
export function opacityToPct(o: number): number {
  const clamped = Math.min(1.0, Math.max(OPACITY_MIN, o));
  const normalized = (clamped - OPACITY_MIN) / (1.0 - OPACITY_MIN);
  return snapOpacityPct(Math.round(normalized * 100));
}

/** Maps user-facing percentage [0%, 100%] to backend physical opacity [0.35, 1.0]. */
export function pctToOpacity(pct: number): number {
  const normalized = snapOpacityPct(pct) / 100;
  return Number((OPACITY_MIN + normalized * (1.0 - OPACITY_MIN)).toFixed(3));
}

/** How many 5% steps above min (0% → 0, …, 100% → 20). */
export function opacityStepIndex(pct: number): number {
  return (snapOpacityPct(pct) - OPACITY_MIN_PCT) / OPACITY_STEP_PCT;
}

/** Fill width aligned to 5% cells (50% is 50%, 100% is 100%). */
export function meterFillPct(pct: number): number {
  return (opacityStepIndex(pct) / OPACITY_INTERVALS) * 100;
}

/** 20 flex cells per 5% interval with major ticks every 10%. */
export function opacityTicksHtml(): string {
  const parts: string[] = [];
  for (let i = 0; i < OPACITY_INTERVALS; i++) {
    const leftPct = OPACITY_MIN_PCT + i * OPACITY_STEP_PCT;
    const rightPct = leftPct + OPACITY_STEP_PCT;
    const major = rightPct % 10 === 0;
    parts.push(`<span class="opacity-tick${major ? " major" : ""}"></span>`);
  }
  return parts.join("");
}

/**
 * Glass opacity + matching text/graph alpha.
 * Background uses --panel-opacity; fg/accent/chrome track the slider so bars
 * and labels don't stay fully solid while the panel goes transparent.
 */
export function applyPanelOpacity(panel: HTMLElement, opacity: number): void {
  const o = Math.min(1, Math.max(OPACITY_MIN, opacity));
  const fg = Math.min(1, Math.max(0.4, o * 0.94 + 0.04));
  const accent = Math.min(1, Math.max(0.36, o * 0.96 + 0.02));
  const chrome = Math.min(1, Math.max(0.28, o * 0.9 + 0.04));
  const tint = chrome;

  const root = typeof document !== "undefined" ? document.documentElement : null;
  const elements = root ? [panel, root] : [panel];
  for (const el of elements) {
    el.style.setProperty("--panel-opacity", String(o));
    el.style.setProperty("--fg-opacity", String(fg));
    el.style.setProperty("--accent-opacity", String(accent));
    el.style.setProperty("--chrome-opacity", String(chrome));
    el.style.setProperty("--tint-strength", String(tint));
  }
}
