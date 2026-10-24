import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mountProviders, tokenModelsForWindow } from "./providers";
import type { ProviderSnapshot, TokenPeriod, TokenUsageSnapshot, UsageWindow } from "./types";

const reset = "2099-10-18T05:00:00Z";
const counts = { input: 100, output: 20, cache_read: 50, cache_write: null, reasoning: null };
const period = (model: string): TokenPeriod => ({ models: [{ model, counts }], totals: counts });
const quota = (overrides: Partial<UsageWindow> = {}): UsageWindow => ({
  kind: "weekly", label: "week", used: 40, limit: 100, unit: "percent",
  used_percent: 40, resets_at: reset, ...overrides,
});
type TokenWindow = NonNullable<TokenUsageSnapshot["windows"]>[number];
const recorded = (model: string, overrides: Partial<TokenWindow> = {}): TokenWindow => ({
  kind: "weekly", resets_at: reset, usage: period(model), ...overrides,
});
function snapshot(windows: UsageWindow[], tokens: TokenWindow[]): ProviderSnapshot {
  return {
    provider_id: "claude", display_name: "Claude", windows, status: "ok", source: "vendor",
    as_of: reset, message: null, primary_resets_at: reset, primary_used_percent: 40,
    token_usage: { source: "local_records", status: "ready", as_of: reset,
      today: period("today"), week: period("week"), month: period("month"), windows: tokens },
  };
}

describe("recorded model attribution to vendor windows", () => {
  it("distinguishes Claude labels even when period and reset coincide", () => {
    const opus = quota({ label: "Opus week" });
    const sonnet = quota({ label: "Sonnet week" });
    const data = snapshot([opus, sonnet], [recorded("sonnet", { label: "Sonnet week" }), recorded("opus", { label: "Opus week" })]);
    expect(tokenModelsForWindow(data, opus)?.models[0].model).toBe("opus");
    expect(tokenModelsForWindow(data, sonnet)?.models[0].model).toBe("sonnet");
  });

  it("keeps AGY family groups separate and does not assign ungrouped metadata", () => {
    const gemini = quota({ group: "Gemini" });
    const claude = quota({ group: "Claude" });
    const data = snapshot([gemini, claude], [recorded("claude", { group: "Claude" }), recorded("gemini", { group: "Gemini" })]);
    expect(tokenModelsForWindow(data, gemini)?.models[0].model).toBe("gemini");
    expect(tokenModelsForWindow(data, claude)?.models[0].model).toBe("claude");
    data.token_usage!.windows = [recorded("unknown", { group: null })];
    expect(tokenModelsForWindow(data, gemini)).toBeUndefined();
    expect(tokenModelsForWindow(data, claude)).toBeUndefined();
  });

  it("normalizes absent groups and equivalent reset instants for unique legacy data", () => {
    const window = quota({ resets_at: "2099-10-18T14:00:00+09:00" });
    const data = snapshot([window], [recorded("legacy", { group: null })]);
    expect(tokenModelsForWindow(data, window)?.models[0].model).toBe("legacy");
  });

  it("rejects legacy data ambiguous on either side", () => {
    const window = quota();
    expect(tokenModelsForWindow(snapshot([window], [recorded("first"), recorded("second")]), window)).toBeUndefined();
    const duplicateQuota = quota({ label: "other week" });
    expect(tokenModelsForWindow(snapshot([window, duplicateQuota], [recorded("legacy")]), window)).toBeUndefined();
  });

  it("requires matching label and reset and leaves unsupported or unavailable windows plain", () => {
    const window = quota();
    expect(tokenModelsForWindow(snapshot([window], [recorded("wrong label", { label: "Opus week" })]), window)).toBeUndefined();
    expect(tokenModelsForWindow(snapshot([window], [recorded("wrong reset", { resets_at: "2099-10-19T05:00:00Z" })]), window)).toBeUndefined();
    const invalid = quota({ resets_at: "invalid" });
    expect(tokenModelsForWindow(snapshot([invalid], [recorded("invalid", { resets_at: "invalid" })]), invalid)).toBeUndefined();
    const monthly = quota({ kind: "monthly" });
    expect(tokenModelsForWindow(snapshot([monthly], [recorded("monthly", { kind: "monthly" })]), monthly)).toBeUndefined();
    const data = snapshot([window], [recorded("unavailable")]);
    data.token_usage!.status = "unavailable";
    expect(tokenModelsForWindow(data, window)).toBeUndefined();
  });

  it("supports rolling5h and rolling_5h window kinds interchangeably", () => {
    const rollingBackend = quota({ kind: "rolling5h", label: "5h" });
    const data = snapshot([rollingBackend], [recorded("gpt-4o", { kind: "rolling5h", label: "5h" })]);
    expect(tokenModelsForWindow(data, rollingBackend)?.models[0].model).toBe("gpt-4o");

    const rollingFrontend = quota({ kind: "rolling_5h", label: "5h" });
    const dataMixed = snapshot([rollingFrontend], [recorded("gpt-4o", { kind: "rolling5h", label: "5h" })]);
    expect(tokenModelsForWindow(dataMixed, rollingFrontend)?.models[0].model).toBe("gpt-4o");
  });
});

class MockElement {
  tagName: string;
  className = "";
  hidden = false;
  _innerHTML = "";
  children: MockElement[] = [];
  eventListeners: Record<string, Function[]> = {};
  dataset: Record<string, string> = {};

  constructor(tagName = "div") {
    this.tagName = tagName;
  }

  get innerHTML(): string {
    return this._innerHTML;
  }

  set innerHTML(val: string) {
    this._innerHTML = val;
    if (val.includes("provider-list") && val.includes("empty-state")) {
      const empty = new MockElement("div");
      empty.className = "empty-state";
      const list = new MockElement("div");
      list.className = "provider-list";
      this.children = [empty, list];
    }
  }

  append(...nodes: MockElement[]) {
    this.children.push(...nodes);
  }

  replaceChildren(...nodes: MockElement[]) {
    this.children = [...nodes];
  }

  classList = {
    add: (_c: string) => {},
    remove: (_c: string) => {},
    toggle: (_c: string) => {},
    contains: (_c: string) => false,
  };

  addEventListener(type: string, listener: Function) {
    (this.eventListeners[type] ??= []).push(listener);
  }

  setAttribute(_k: string, _v: string) {}
  removeAttribute(_k: string) {}

  querySelector<T = MockElement>(sel: string): T | null {
    if (sel === ".provider-list") {
      return (this.children.find(c => c.className === "provider-list") ?? null) as unknown as T;
    }
    if (sel === ".empty-state") {
      return (this.children.find(c => c.className === "empty-state") ?? null) as unknown as T;
    }
    return new MockElement("div") as unknown as T;
  }

  querySelectorAll<T = MockElement>(_sel: string): T[] {
    return [];
  }
}

describe("provider card DOM rendering and model legends", () => {
  beforeEach(() => {
    vi.stubGlobal("document", {
      createElement: (tag: string) => new MockElement(tag),
      activeElement: null,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders model legend under bar graph and triggers onResize for rolling5h", () => {
    const onResize = vi.fn();
    const root = new MockElement("div");
    const mounted = mountProviders(root as unknown as HTMLElement, onResize);

    const rollingWindow = quota({ kind: "rolling5h", label: "5h", used_percent: 50 });
    const snap = snapshot(
      [rollingWindow],
      [recorded("claude-3-5-sonnet-20241022", { kind: "rolling5h", label: "5h" })],
    );

    mounted.setSnapshots([snap]);

    expect(onResize).toHaveBeenCalled();

    const list = root.querySelector<MockElement>(".provider-list")!;
    const entry = list.children[0];
    const quotaHtml = entry.children[0].innerHTML;

    expect(quotaHtml).toContain("token-model-legend");
    expect(quotaHtml).toContain("token-model-dot");
    expect(quotaHtml).toContain("3.5 Sonnet");
  });

  it("renders model legend correctly for rolling_5h window format", () => {
    const onResize = vi.fn();
    const root = new MockElement("div");
    const mounted = mountProviders(root as unknown as HTMLElement, onResize);

    const rollingWindow = quota({ kind: "rolling_5h", label: "5h", used_percent: 60 });
    const snap = snapshot(
      [rollingWindow],
      [recorded("claude-3-5-sonnet-20241022", { kind: "rolling5h", label: "5h" })],
    );

    mounted.setSnapshots([snap]);

    expect(onResize).toHaveBeenCalled();

    const list = root.querySelector<MockElement>(".provider-list")!;
    const entry = list.children[0];
    const quotaHtml = entry.children[0].innerHTML;

    expect(quotaHtml).toContain("token-model-legend");
    expect(quotaHtml).toContain("token-model-dot");
    expect(quotaHtml).toContain("3.5 Sonnet");
  });

  it("renders multi-model legend with correct dot colors and short labels for weekly window", () => {
    const onResize = vi.fn();
    const root = new MockElement("div");
    const mounted = mountProviders(root as unknown as HTMLElement, onResize);

    const weeklyWindow = quota({ kind: "weekly", label: "week", used_percent: 75 });
    const snap = snapshot(
      [weeklyWindow],
      [{
        kind: "weekly",
        label: "week",
        resets_at: reset,
        usage: {
          models: [
            { model: "claude-3-5-sonnet-20241022", counts: { input: 800, output: 200, cache_read: 0, cache_write: null, reasoning: null } },
            { model: "claude-3-opus-20240229", counts: { input: 200, output: 50, cache_read: 0, cache_write: null, reasoning: null } },
          ],
          totals: { input: 1000, output: 250, cache_read: 0, cache_write: null, reasoning: null },
        },
      }],
    );

    mounted.setSnapshots([snap]);

    const list = root.querySelector<MockElement>(".provider-list")!;
    const entry = list.children[0];
    const quotaHtml = entry.children[0].innerHTML;

    expect(quotaHtml).toContain("token-model-legend");
    expect(quotaHtml).toContain("3.5 Sonnet");
    expect(quotaHtml).toContain("3 Opus");
    const dotCount = (quotaHtml.match(/token-model-dot/g) || []).length;
    expect(dotCount).toBe(2);
  });

  it("renders full Gemini 3.8 Flash High model label without truncation in AGY provider legend", () => {
    const onResize = vi.fn();
    const root = new MockElement("div");
    const mounted = mountProviders(root as unknown as HTMLElement, onResize);

    const agyRolling = quota({ kind: "rolling5h", label: "5h", used_percent: 45, group: "Gemini" });
    const snap: ProviderSnapshot = {
      provider_id: "agy",
      display_name: "Antigravity",
      windows: [agyRolling],
      status: "ok",
      source: "vendor",
      as_of: reset,
      message: null,
      primary_resets_at: reset,
      primary_used_percent: 45,
      token_usage: {
        source: "local_records",
        status: "ready",
        as_of: reset,
        today: period("today"),
        week: period("week"),
        month: period("month"),
        windows: [{
          kind: "rolling5h",
          label: "5h",
          group: "Gemini",
          resets_at: reset,
          usage: {
            models: [{
              model: "gemini-3.8-flash-high",
              counts: { input: 12000, output: 400, cache_read: 2000, cache_write: null, reasoning: null },
            }],
            totals: { input: 12000, output: 400, cache_read: 2000, cache_write: null, reasoning: null },
          },
        }],
      },
    };

    mounted.setSnapshots([snap]);

    const list = root.querySelector<MockElement>(".provider-list")!;
    const entry = list.children[0];
    const quotaHtml = entry.children[0].innerHTML;

    expect(quotaHtml).toContain("token-model-legend");
    expect(quotaHtml).toContain("3.8 Flash");
    expect(quotaHtml).not.toContain("3.8 Flash High");
    expect(quotaHtml).not.toContain("3.8 Flash-H…");
  });

  it("omits model legend when token models are unavailable or quota is idle", () => {
    const onResize = vi.fn();
    const root = new MockElement("div");
    const mounted = mountProviders(root as unknown as HTMLElement, onResize);

    const rollingWindow = quota({ kind: "rolling5h", label: "5h", used_percent: 0 });
    const snap = snapshot([rollingWindow], []);

    mounted.setSnapshots([snap]);

    const list = root.querySelector<MockElement>(".provider-list")!;
    const entry = list.children[0];
    const quotaHtml = entry.children[0].innerHTML;

    expect(quotaHtml).not.toContain("token-model-legend");
  });

  it("renders multi-group AGY snapshot including Gemini, Claude, and Argon groups", () => {
    const onResize = vi.fn();
    const root = new MockElement("div");
    const mounted = mountProviders(root as unknown as HTMLElement, onResize);

    const gemini5h = quota({ kind: "rolling5h", label: "5h", used_percent: 30, group: "Gemini" });
    const geminiWeek = quota({ kind: "weekly", label: "Week", used_percent: 25, group: "Gemini" });
    const claude5h = quota({ kind: "rolling5h", label: "5h", used_percent: 0, group: "Claude" });
    const claudeWeek = quota({ kind: "weekly", label: "Week", used_percent: 0, group: "Claude" });
    const argonWeek = quota({ kind: "weekly", label: "Week", used_percent: 100, group: "Argon" });

    const snap: ProviderSnapshot = {
      provider_id: "agy",
      display_name: "Antigravity",
      windows: [gemini5h, geminiWeek, claude5h, claudeWeek, argonWeek],
      status: "ok",
      source: "vendor",
      as_of: reset,
      message: null,
      primary_resets_at: reset,
      primary_used_percent: 30,
    };

    mounted.setSnapshots([snap]);

    const list = root.querySelector<MockElement>(".provider-list")!;
    const entry = list.children[0];
    const quotaHtml = entry.children[0].innerHTML;

    expect(quotaHtml).toContain("Gemini");
    expect(quotaHtml).toContain("Claude");
    expect(quotaHtml).toContain("Argon");
    expect(quotaHtml).toContain("Antigravity");
  });
});