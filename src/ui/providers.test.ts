import { describe, expect, it } from "vitest";
import { tokenModelsForWindow } from "./providers";
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
});
