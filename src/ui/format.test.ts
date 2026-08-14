import { describe, expect, it } from "vitest";
import {
  clampPct,
  formatPct,
  formatWindowReset,
  isOver,
  levelClass,
} from "./format";

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
