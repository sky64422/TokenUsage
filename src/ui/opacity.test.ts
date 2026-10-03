import { describe, expect, it } from "vitest";
import {
  applyPanelOpacity,
  meterFillPct,
  opacityStepIndex,
  opacityTicksHtml,
  opacityToPct,
  OPACITY_DEFAULT,
  OPACITY_MAX_PCT,
  OPACITY_MIN,
  OPACITY_MIN_PCT,
  OPACITY_STEP_PCT,
  pctToOpacity,
  snapOpacityPct,
} from "./opacity";

describe("snapOpacityPct", () => {
  it("clamps below min to OPACITY_MIN_PCT", () => {
    expect(snapOpacityPct(-10)).toBe(OPACITY_MIN_PCT);
    expect(snapOpacityPct(0)).toBe(OPACITY_MIN_PCT);
  });

  it("clamps above max to OPACITY_MAX_PCT", () => {
    expect(snapOpacityPct(120)).toBe(OPACITY_MAX_PCT);
  });

  it("snaps to nearest 5% step", () => {
    expect(snapOpacityPct(37)).toBe(35);
    expect(snapOpacityPct(38)).toBe(40);
    expect(snapOpacityPct(48)).toBe(50);
    expect(snapOpacityPct(52)).toBe(50);
    expect(snapOpacityPct(92)).toBe(90);
    expect(snapOpacityPct(93)).toBe(95);
  });
});

describe("opacityToPct & pctToOpacity", () => {
  it("converts fraction (0.35-1.0) to user percent (0-100)", () => {
    expect(opacityToPct(OPACITY_DEFAULT)).toBe(90);
    expect(opacityToPct(1.0)).toBe(100);
    expect(opacityToPct(OPACITY_MIN)).toBe(0);
    expect(opacityToPct(0.675)).toBe(50);
  });

  it("converts percent back to fraction", () => {
    expect(pctToOpacity(0)).toBe(0.35);
    expect(pctToOpacity(50)).toBe(0.675);
    expect(pctToOpacity(90)).toBe(0.935);
    expect(pctToOpacity(100)).toBe(1.0);
  });
});

describe("opacityStepIndex & meterFillPct", () => {
  it("calculates 0-indexed step from min percent", () => {
    expect(opacityStepIndex(OPACITY_MIN_PCT)).toBe(0);
    expect(opacityStepIndex(OPACITY_MIN_PCT + OPACITY_STEP_PCT)).toBe(1);
    expect(opacityStepIndex(50)).toBe(10);
    expect(opacityStepIndex(OPACITY_MAX_PCT)).toBe(20);
  });

  it("calculates meter fill percentage from 0% to 100% matching label 1:1", () => {
    expect(meterFillPct(0)).toBe(0);
    expect(meterFillPct(50)).toBe(50);
    expect(meterFillPct(100)).toBe(100);
    expect(meterFillPct(snapOpacityPct(67.5))).toBe(70);
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

describe("opacityTicksHtml", () => {
  it("generates 20 tick spans with major ticks every 10%", () => {
    const html = opacityTicksHtml();
    const count = (html.match(/<span class="opacity-tick/g) || []).length;
    expect(count).toBe(20);
    const majorCount = (html.match(/opacity-tick major/g) || []).length;
    // 10, 20, 30, 40, 50, 60, 70, 80, 90, 100 are multiples of 10%
    expect(majorCount).toBe(10);
  });
});
