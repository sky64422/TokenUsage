import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cacheReadPercent, formatCachePercent, formatTokens, mountTokenDetails } from "./token-details";
import type { TokenUsageSnapshot } from "./types";
import { tokenTotal, modelSegments, formatTokenPercent } from "./format";

describe("recorded token display", () => {
  it("formats token ratios consistently in details and main legends", () => {
    expect(formatTokenPercent(null)).toBe("—");
    expect(formatTokenPercent(0)).toBe("0%");
    expect(formatTokenPercent(100)).toBe("100%");
    expect(formatTokenPercent(100 / 1.1)).toBe("90.9%");
  });
  const counts = { input: 1000, cache_read: 750, cache_write: 100, output: 200, reasoning: 50 };
  it("does not double-count cached input or reasoning output", () => {
    expect(tokenTotal(counts)).toBe(1200);
    expect(cacheReadPercent(counts)).toBe(75);
  });
  it("distinguishes omitted, explicit zero and inconsistent values", () => {
    expect(formatTokens(null)).toBe("—");
    expect(formatTokens(0)).toBe("0 Tok");
    expect(formatCachePercent(null)).toBe("—");
    expect(formatCachePercent(0)).toBe("0% Hit");
    expect(formatCachePercent(85)).toBe("85% Hit");
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

class MockNode {
  tagName: string;
  className = "";
  hidden = false;
  textContent = "";
  title = "";
  dataset: Record<string, string> = {};
  children: MockNode[] = [];
  attributes: Record<string, string> = {};
  style: Record<string, any> = { setProperty: vi.fn() };

  constructor(tagName = "div") {
    this.tagName = tagName;
  }

  append(...nodes: MockNode[]) {
    this.children.push(...nodes);
  }

  replaceChildren(...nodes: MockNode[]) {
    this.children = [...nodes];
  }

  setAttribute(k: string, v: string) {
    this.attributes[k] = v;
  }

  querySelector<T = MockNode>(sel: string): T | null {
    if (sel === "details") {
      let f = this.children.find(c => c.tagName === "details");
      if (!f) { f = new MockNode("details"); this.children.push(f); }
      return f as unknown as T;
    }
    if (sel === ".token-overview tbody") {
      let f = this.children.find(c => c.tagName === "tbody");
      if (!f) { f = new MockNode("tbody"); this.children.push(f); }
      return f as unknown as T;
    }
    if (sel === ".token-trend") {
      let f = this.children.find(c => c.className === "token-trend");
      if (!f) { f = new MockNode("div"); f.className = "token-trend"; this.children.push(f); }
      return f as unknown as T;
    }
    return new MockNode("div") as unknown as T;
  }

  classList = {
    add: vi.fn(),
  };

  addEventListener = vi.fn();
}

describe("mountTokenDetails component rendering", () => {
  beforeEach(() => {
    vi.stubGlobal("document", {
      createElement: (tag: string) => new MockNode(tag),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders 30-day fixed trend chart and '최근 30일' summary period", () => {
    const root = new MockNode("div");
    const onResize = vi.fn();
    const tokenView = mountTokenDetails(root as unknown as HTMLElement, onResize);

    const snapshot: TokenUsageSnapshot = {
      source: "local_records",
      status: "ready",
      as_of: "2026-10-20T12:00:00Z",
      today: { models: [], totals: { input: 100, output: 20, cache_read: 0, cache_write: null, reasoning: null } },
      yesterday: { models: [], totals: { input: 80, output: 10, cache_read: 0, cache_write: null, reasoning: null } },
      month: { models: [], totals: { input: 5000, output: 400, cache_read: 0, cache_write: null, reasoning: null } },
      week: { models: [], totals: { input: 500, output: 50, cache_read: 0, cache_write: null, reasoning: null } },
      daily: [
        { date: "2026-10-20", totals: { input: 1_200_000, output: 50_000, cache_read: 0, cache_write: null, reasoning: null } },
        { date: "2026-10-19", totals: { input: 24_000, output: 1_000, cache_read: 0, cache_write: null, reasoning: null } },
      ],
      windows: [],
    };

    tokenView.update(snapshot);

    const tbody = root.querySelector(".token-overview tbody")!;
    const rowHeaders = tbody.children.map(tr => tr.children[0]?.textContent);
    expect(rowHeaders).toEqual(["오늘", "어제", "최근 30일"]);

    const firstRowValue = tbody.children[0]?.children[1];
    const tokensSpan = firstRowValue?.children[0];
    expect(tokensSpan?.textContent).toBe("120 Tok");
    const cacheSpan = firstRowValue?.children[2];
    expect(cacheSpan?.textContent).toBe("0% Hit");
    expect(cacheSpan?.textContent).not.toContain("캐시");

    const trend = root.querySelector(".token-trend")!;
    const label = trend.children[0];
    expect(label.textContent).toBe("사용 추이");
    expect(label.title).toBe("최근 30일 일별 토큰 사용량");

    const chart = trend.children[1];
    expect(chart.attributes["aria-label"]).toBe("최근 30일 일별 총 토큰 사용량. 빈 자리는 기록 없음입니다.");
    expect(chart.children).toHaveLength(30);

    const lastBar = chart.children[29];
    expect(lastBar.dataset.tip).toBe("10월 20일 · 1.3M Tok");
    expect(lastBar.attributes["aria-label"]).toBe("10월 20일 · 1.3M Tok");
    expect(lastBar.dataset.state).toBe("value");

    const prevBar = chart.children[28];
    expect(prevBar.dataset.tip).toBe("10월 19일 · 25K Tok");
    expect(prevBar.attributes["aria-label"]).toBe("10월 19일 · 25K Tok");
    expect(prevBar.dataset.state).toBe("value");

    const emptyBar = chart.children[0];
    expect(emptyBar.dataset.tip).toContain("기록 없음");
    expect(emptyBar.attributes["aria-label"]).toContain("기록 없음");
    expect(emptyBar.dataset.state).toBe("unknown");
  });
});