import { describe, expect, it } from "vitest";
import {
  applyPanelOpacity,
  meterFillPct,
  opacityStepIndex,
  opacityToPct,
  OPACITY_MAX_PCT,
  OPACITY_MIN_PCT,
  OPACITY_STEP_PCT,
  pctToOpacity,
  snapOpacityPct,
} from "./opacity";

describe("snapOpacityPct", () => {
  it("clamps below min to OPACITY_MIN_PCT", () => {
    expect(snapOpacityPct(10)).toBe(OPACITY_MIN_PCT);
    expect(snapOpacityPct(0)).toBe(OPACITY_MIN_PCT);
  });

  it("clamps above max to OPACITY_MAX_PCT", () => {
    expect(snapOpacityPct(120)).toBe(OPACITY_MAX_PCT);
  });

  it("snaps to nearest 5% step", () => {
    expect(snapOpacityPct(37)).toBe(35);
    expect(snapOpacityPct(38)).toBe(40);
    expect(snapOpacityPct(92)).toBe(90);
    expect(snapOpacityPct(93)).toBe(95);
  });
});

describe("opacityToPct & pctToOpacity", () => {
  it("converts fraction (0.0-1.0) to snapped whole percent", () => {
    expect(opacityToPct(0.92)).toBe(90);
    expect(opacityToPct(0.95)).toBe(95);
    expect(opacityToPct(1.0)).toBe(100);
    expect(opacityToPct(0.35)).toBe(35);
  });

  it("converts percent back to fraction", () => {
    expect(pctToOpacity(90)).toBe(0.9);
    expect(pctToOpacity(100)).toBe(1.0);
    expect(pctToOpacity(35)).toBe(0.35);
  });
});

describe("opacityStepIndex & meterFillPct", () => {
  it("calculates 0-indexed step from min percent", () => {
    expect(opacityStepIndex(OPACITY_MIN_PCT)).toBe(0);
    expect(opacityStepIndex(OPACITY_MIN_PCT + OPACITY_STEP_PCT)).toBe(1);
    expect(opacityStepIndex(OPACITY_MAX_PCT)).toBe(13);
  });

  it("calculates meter fill percentage from 0% to 100%", () => {
    expect(meterFillPct(OPACITY_MIN_PCT)).toBe(0);
    expect(meterFillPct(OPACITY_MAX_PCT)).toBe(100);
    expect(meterFillPct(snapOpacityPct(67.5))).toBeCloseTo((7 / 13) * 100, 2);
  });
});

describe("applyPanelOpacity", () => {
  it("sets css variables on panel element", () => {
    const vars: Record<string, string> = {};
    const mockEl = {
      style: {
        setProperty(name: string, val: string) {
          vars[name] = val;
        },
        getPropertyValue(name: string) {
          return vars[name] ?? "";
        },
      },
    } as unknown as HTMLElement;

    applyPanelOpacity(mockEl, 0.85);

    expect(vars["--panel-opacity"]).toBe("0.85");
    expect(parseFloat(vars["--fg-opacity"])).toBeGreaterThan(0.4);
    expect(parseFloat(vars["--accent-opacity"])).toBeGreaterThan(0.36);
    expect(parseFloat(vars["--chrome-opacity"])).toBeGreaterThan(0.28);
  });
});
