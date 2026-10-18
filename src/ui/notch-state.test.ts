import { describe, it, expect, vi } from "vitest";
import {
  initialNotchState,
  reduceNotchState,
  createCloseDelay,
  headline,
} from "./notch-state";
import type { ProviderSnapshot } from "./types";

describe("notch interaction", () => {
  it("keeps settings open when the selected provider is disabled", () => {
    const state = { provider: "claude" as const, pinned: true, settings: true };
    expect(
      reduceNotchState(state, { type: "providers", ids: ["grok"] }),
    ).toEqual({ provider: null, pinned: false, settings: true });
  });
  it("keeps pinned detail across leave and closes settings before pinned detail", () => {
    let s = reduceNotchState(initialNotchState(), {
      type: "pin",
      id: "claude",
    });
    s = reduceNotchState(s, { type: "leave" });
    expect(s.provider).toBe("claude");
    s = reduceNotchState(s, { type: "settings" });
    s = reduceNotchState(s, { type: "escape" });
    expect(s.settings).toBe(false);
    expect(s.provider).toBe("claude");
    expect(reduceNotchState(s, { type: "escape" }).provider).toBe(null);
  });
  it("does not let hover replace a pinned provider or a settings sheet", () => {
    let s = reduceNotchState(initialNotchState(), {
      type: "pin",
      id: "claude",
    });
    expect(reduceNotchState(s, { type: "hover", id: "grok" }).provider).toBe(
      "claude",
    );
    s = reduceNotchState(s, { type: "settings" });
    expect(reduceNotchState(s, { type: "leave" }).settings).toBe(true);
  });
  it("cancels pending close on re-entry", () => {
    vi.useFakeTimers();
    const closed = vi.fn();
    const delay = createCloseDelay(closed);
    delay.schedule();
    vi.advanceTimersByTime(100);
    delay.cancel();
    vi.advanceTimersByTime(200);
    expect(closed).not.toHaveBeenCalled();
    delay.schedule();
    vi.advanceTimersByTime(180);
    expect(closed).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});
describe("quota headline", () => {
  it("warns about a secondary critical window without replacing primary", () => {
    const snap = fixture(10);
    snap.windows[0].label = "5h";
    snap.windows[0].kind = "rolling_5h";
    snap.windows.push({
      ...snap.windows[0],
      label: "Week",
      kind: "weekly",
      used_percent: 95,
    });
    expect(headline(snap)).toMatchObject({
      text: "10%",
      secondaryRisk: "Week 95%",
    });
  });
  const fixture = (pct: number | null): ProviderSnapshot => ({
    provider_id: "claude",
    display_name: "Claude",
    status: "ok",
    source: "vendor",
    message: null,
    as_of: "",
    primary_used_percent: pct,
    primary_resets_at: null,
    windows: [
      {
        kind: "weekly",
        label: "Week",
        used: pct ?? 0,
        limit: 100,
        unit: "percent",
        used_percent: pct,
        resets_at: null,
      },
    ],
  });
  it("preserves overage while clamping only the ring", () => {
    expect(headline(fixture(125))).toMatchObject({
      text: "125%",
      fill: 100,
      label: "Week",
    });
  });
  it("distinguishes zero from missing and refuses auth failures", () => {
    expect(headline(fixture(0)).text).toBe("0%");
    expect(headline(fixture(null)).text).toBe("—");
    expect(headline({ ...fixture(33), status: "auth_required" }).text).toBe(
      "—",
    );
  });
});

describe("positionNotch anchoring", () => {
  it("anchors left and bottom when anchor_x=left and anchor_y=bottom", async () => {
    const { positionNotch } = await import("./notch");
    const el = { style: {} as Record<string, string> };
    const layout = {
      edge: "left" as const,
      anchor_x: "left" as const,
      anchor_y: "bottom" as const,
      notch: { x: 0, y: 800, width: 40, height: 188 },
      detail: { x: 48, y: 638, width: 228, height: 350 },
      window: { x: 0, y: 638, width: 276, height: 350 },
      scale: 1,
      monitor: "test",
      metrics: {
        depth: 40,
        cell: 44,
        shoulder: 16,
        inner_radius: 12,
        inset: 50,
        detail_radius: 16,
        drag_threshold: 5,
        rest_depth: 10,
        rest_length: 80,
      },
    };
    positionNotch(el as any, layout.notch, layout);
    expect(el.style.left).toBe("0px");
    expect(el.style.bottom).toBe("0px");
    expect(el.style.right).toBe("");
    expect(el.style.top).toBe("");
  });

  it("anchors right and top when anchor_x=right and anchor_y=top", async () => {
    const { positionNotch } = await import("./notch");
    const el = { style: {} as Record<string, string> };
    const layout = {
      edge: "right" as const,
      anchor_x: "right" as const,
      anchor_y: "top" as const,
      notch: { x: 1880, y: 100, width: 40, height: 188 },
      detail: { x: 1644, y: 100, width: 228, height: 350 },
      window: { x: 1644, y: 100, width: 276, height: 350 },
      scale: 1,
      monitor: "test",
      metrics: {
        depth: 40,
        cell: 44,
        shoulder: 16,
        inner_radius: 12,
        inset: 50,
        detail_radius: 16,
        drag_threshold: 5,
        rest_depth: 10,
        rest_length: 80,
      },
    };
    positionNotch(el as any, layout.notch, layout);
    expect(el.style.right).toBe("0px");
    expect(el.style.top).toBe("0px");
    expect(el.style.left).toBe("");
    expect(el.style.bottom).toBe("");
  });
});
