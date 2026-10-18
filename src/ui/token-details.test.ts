import { describe, expect, it } from "vitest";
import { cacheReadPercent, tokenTotal, formatTokens, modelShare } from "./token-details";

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
});
