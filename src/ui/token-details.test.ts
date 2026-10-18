import { describe, expect, it } from "vitest";
import { cacheReadPercent, formatTokens, modelShare } from "./token-details";
import { tokenTotal, modelSegments } from "./format";

describe("recorded token display", () => {
  const counts = { input: 1000, cache_read: 750, cache_write: 100, output: 200, reasoning: 50 };
  it("does not double-count cached input or reasoning output", () => {
    expect(tokenTotal(counts)).toBe(1200);
    expect(cacheReadPercent(counts)).toBe(75);
    expect(modelShare(counts, { ...counts, input: 2000, output: 400 })).toBe(50);
  });
  it("distinguishes omitted, explicit zero and inconsistent values", () => {
    expect(formatTokens(null)).toBe("—");
    expect(formatTokens(0)).toBe("0");
    expect(cacheReadPercent({ ...counts, input: 0, cache_read: 0 })).toBeNull();
    expect(cacheReadPercent({ ...counts, cache_read: null })).toBeNull();
    expect(cacheReadPercent({ ...counts, cache_read: 1001 })).toBeNull();
    expect(tokenTotal({ ...counts, input: null })).toBeNull();
    expect(formatTokens(Number.MAX_SAFE_INTEGER + 1)).toBe("—");
  });
  it("uses the ratio of summed counters for the total", () => {
    expect(cacheReadPercent({ ...counts, input: 1100, cache_read: 750 })).toBeCloseTo(68.1818, 3);
  });

  it("ranks model segments with darker shades for larger amounts and short labels", () => {
    const small = { ...counts, input: 100, output: 20, cache_read: 0 };
    const data = { models: [{ model: "gpt-6-astra", counts: small }, { model: "gpt-6.1-sol", counts }],
      totals: { ...counts, input: 1100, output: 220 } };
    const segments = modelSegments(data);
    expect(segments.map(s => s.label)).toEqual(["6.1 Sol", "6 Astra"]);
    expect(segments[0].share).toBeCloseTo(100 / 1.1);
    expect(segments[0].lightness).toBeLessThan(segments[1].lightness);
    expect(segments[0].color).toContain("var(--model-base-color) 100%");
    expect(modelSegments({ ...data, totals: { ...data.totals, input: null } })).toEqual([]);
  });
});
