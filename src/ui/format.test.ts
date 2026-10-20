import { describe, expect, it } from "vitest";
import {
  clampPct,
  formatCountdown,
  formatPct,
  formatWindowLabel,
  formatWindowReset,
  isOver,
  levelClass,
  formatProviderStatus,
  formatModelLabel,
} from "./format";

describe("provider status summaries", () => {
  it.each([
    ["unavailable", "Reading Antigravity CLI quota", "Loading…"],
    ["auth_required", "OAuth expired; run login again", "Sign in required"],
    ["unavailable", "agy timed out after 30 seconds with no usage response", "Request timed out"],
    ["degraded", "Usage unavailable — Grok did not report a percentage", "Usage unavailable"],
    ["unavailable", "Unexpected response\nvery long backend detail", "Unable to fetch usage"],
    ["ok", null, ""],
  ])("summarizes %s / %s", (status, message, expected) => {
    expect(formatProviderStatus(status, message)).toBe(expected);
  });
});

describe("clampPct", () => {
  it("returns null for missing values", () => {
    expect(clampPct(null)).toBeNull();
    expect(clampPct(undefined)).toBeNull();
    expect(clampPct(Number.NaN)).toBeNull();
  });

  it("clamps to 0–100", () => {
    expect(clampPct(-4)).toBe(0);
    expect(clampPct(40)).toBe(40);
    expect(clampPct(140)).toBe(100);
  });
});

describe("isOver", () => {
  it("treats over-limit message and used>limit as over", () => {
    expect(isOver(10, "over limit")).toBe(true);
    expect(isOver(10, null, 120, 100)).toBe(true);
    expect(isOver(101)).toBe(true);
  });

  it("does not treat estimate-style messages as over by themselves", () => {
    expect(isOver(40, "estimate (context tokens)")).toBe(false);
  });
});

describe("levelClass", () => {
  it("maps usage colors at the inclusive 50 / 75 / 90 percent boundaries", () => {
    expect(levelClass(0, false, true)).toBe("level-idle");
    expect(levelClass(50, true, false)).toBe("level-over");
    expect(levelClass(null)).toBe("level-na");
    expect(levelClass(0)).toBe("level-ok");
    expect(levelClass(50)).toBe("level-ok");
    expect(levelClass(50.01)).toBe("level-caution");
    expect(levelClass(75)).toBe("level-caution");
    expect(levelClass(75.01)).toBe("level-warn");
    expect(levelClass(90)).toBe("level-warn");
    expect(levelClass(90.01)).toBe("level-critical");
    expect(levelClass(100)).toBe("level-critical");
    expect(levelClass(125)).toBe("level-critical");
  });
});

describe("formatPct", () => {
  it("preserves measured overage while the bar can clamp separately",()=>{
    expect(formatPct(125,true)).toBe("125%");
    expect(formatPct(0)).toBe("0%");
  });
  it("shows em dash when idle or missing", () => {
    expect(formatPct(0, false, true)).toBe("—");
    expect(formatPct(null)).toBe("—");
  });

  it("rounds and forces 100% when over", () => {
    expect(formatPct(41.4)).toBe("41%");
    expect(formatPct(80, true)).toBe("100%");
  });
});

describe("formatWindowReset", () => {
  it("is empty when idle", () => {
    expect(
      formatWindowReset({
        resetsAt: "2026-08-01T12:00:00Z",
        idle: true,
        over: false,
      }),
    ).toBe("");
  });

  it("uses soon label when reset is in the past", () => {
    const text = formatWindowReset({
      resetsAt: "2020-01-01T00:00:00Z",
      idle: false,
      over: false,
      now: Date.parse("2026-08-01T00:00:00Z"),
    });
    expect(text.startsWith("↻ soon")).toBe(true);
  });

  it("prefixes Over when over limit", () => {
    const text = formatWindowReset({
      resetsAt: "2026-08-02T00:00:00Z",
      idle: false,
      over: true,
      now: Date.parse("2026-08-01T00:00:00Z"),
    });
    expect(text.startsWith("Over ·")).toBe(true);
  });
});

describe("formatWindowLabel", () => {
  it("normalizes internal window kind names to glanceable labels", () => {
    expect(formatWindowLabel("rolling_5h")).toBe("5h");
    expect(formatWindowLabel("rolling5h")).toBe("5h");
    expect(formatWindowLabel("weekly")).toBe("Week");
    expect(formatWindowLabel("monthly")).toBe("Month");
    expect(formatWindowLabel("5h · over")).toBe("5h");
    expect(formatWindowLabel("Custom Window")).toBe("Custom Window");
    expect(formatWindowLabel(null)).toBe("");
  });
});

describe("formatModelLabel", () => {
  it("formats OpenAI, Claude and Gemini model names into concise glanceable labels", () => {
    expect(formatModelLabel("gpt-6-astra")).toBe("6 Astra");
    expect(formatModelLabel("gpt-6.1-sol")).toBe("6.1 Sol");
    expect(formatModelLabel("gpt-4o")).toBe("4o");
    expect(formatModelLabel("claude-3-5-sonnet-20241022")).toBe("3.5 Sonnet");
    expect(formatModelLabel("claude-3-opus-20240229")).toBe("3 Opus");
    expect(formatModelLabel("gemini-1.5-pro")).toBe("1.5 Pro");
    expect(formatModelLabel("gemini-2.0-flash")).toBe("2.0 Flash");
    expect(formatModelLabel("gemini-3.8-flash-high")).toBe("3.8 Flash High");
    expect(formatModelLabel("gemini-3.8-flash-low")).toBe("3.8 Flash Low");
    expect(formatModelLabel("gemini-2.0-flash-thinking-exp")).toBe("2.0 Flash Thinking");
    expect(formatModelLabel("gemini-1.5-flash-8b")).toBe("1.5 Flash 8B");
    expect(formatModelLabel("gemini-2.0-flash-lite")).toBe("2.0 Flash Lite");
    expect(formatModelLabel("gpt-4o-mini")).toBe("4o Mini");
    expect(formatModelLabel("super-extra-long-unknown-model-exceeding-limit")).toBe("super extra long unknow…");
  });
});

describe("formatCountdown", () => {
  it("returns soon for past or zero time", () => {
    expect(formatCountdown("2026-08-01T00:00:00Z", Date.parse("2026-08-01T00:00:00Z"))).toBe("soon");
    expect(formatCountdown(null)).toBe("");
  });

  it("formats hours and minutes or days", () => {
    const base = Date.parse("2026-08-01T00:00:00Z");
    const twoHoursTenMin = new Date(base + (2 * 3600 + 10 * 60) * 1000).toISOString();
    expect(formatCountdown(twoHoursTenMin, base)).toBe("2h 10m");

    const twoDays = new Date(base + 48 * 3600 * 1000).toISOString();
    expect(formatCountdown(twoDays, base)).toBe("2d");
  });
});
