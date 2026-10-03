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
  it("maps idle / over / warn / critical", () => {
    expect(levelClass(0, false, true)).toBe("level-idle");
    expect(levelClass(50, true, false)).toBe("level-over");
    expect(levelClass(70, false, false)).toBe("level-warn");
    expect(levelClass(90, false, false)).toBe("level-critical");
    expect(levelClass(20, false, false)).toBe("level-ok");
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
    expect(formatWindowLabel("weekly")).toBe("Week");
    expect(formatWindowLabel("monthly")).toBe("Month");
    expect(formatWindowLabel("5h · over")).toBe("5h");
    expect(formatWindowLabel("Custom Window")).toBe("Custom Window");
    expect(formatWindowLabel(null)).toBe("");
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
