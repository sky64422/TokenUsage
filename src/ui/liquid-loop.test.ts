import { describe, expect, it, vi } from "vitest";
import { createLiquidLoop } from "./liquid-loop";

describe("Liquid capture lifecycle", () => {
  it("preserves fractional timing on a faster display without queuing catch-up frames", async () => {
    vi.useFakeTimers();
    try {
      for (const rate of [30, 60]) {
        let tick: (time: number) => void = () => {};
        const paint = vi.fn();
        const loop = createLiquidLoop({
          request: cb => { tick=cb; return 1; }, cancel: () => {}, interval: () => 1000/rate,
          capture: async () => null, paint, dispose: () => {}, release: async () => {}, fail: vi.fn(),
        });
        loop.setEnabled(true);
        for (let sample=0;sample<=120;sample++) {
          tick(sample*8.3);
          await vi.advanceTimersByTimeAsync(0);
        }
        expect(paint.mock.calls.length).toBeGreaterThanOrEqual(rate);
        expect(paint.mock.calls.length).toBeLessThanOrEqual(rate+1);
        loop.destroy();
      }
    } finally { vi.useRealTimers(); }
  });
  it("discards an in-flight image and releases capture before restarting", async () => {
    let tick: (time: number) => void = () => {};
    let finish!: (frame: string) => void;
    const order: string[] = [];
    const loop = createLiquidLoop({
      request: cb => { tick = cb; return 1; }, cancel: () => {}, interval: () => 0,
      capture: () => new Promise<string>(resolve => { finish = resolve; order.push("capture"); }),
      paint: value => order.push(`paint:${value}`), dispose: value => order.push(`dispose:${value}`),
      release: async () => { order.push("release"); }, fail: vi.fn(),
    });
    loop.setEnabled(true); tick(0); tick(16);
    expect(order).toEqual(["capture"]);
    loop.setEnabled(false); loop.setEnabled(true);
    finish("stale");
    await vi.waitFor(() => expect(order).toEqual(["capture", "dispose:stale", "release"]));
    tick(32);
    expect(order[order.length - 1]).toBe("capture");
    loop.destroy(); finish("last");
    await vi.waitFor(() => expect(order[order.length - 1]).toBe("release"));
    expect(order).not.toContain("paint:stale");
  });
  it("releases on failure and allows a later explicit retry", async () => {
    let tick: (time: number) => void = () => {};
    const release = vi.fn(async () => {}), fail = vi.fn();
    const loop = createLiquidLoop({
      request: cb => { tick = cb; return 1; }, cancel: () => {}, interval: () => 0,
      capture: async () => { throw Error("capture failed"); },
      paint: vi.fn(), dispose: vi.fn(), release, fail,
    });
    loop.setEnabled(true); tick(0);
    await vi.waitFor(() => expect(release).toHaveBeenCalledOnce());
    expect(fail).toHaveBeenCalledOnce();
    loop.setEnabled(false); loop.setEnabled(true); tick(16);
    await vi.waitFor(() => expect(release).toHaveBeenCalledTimes(2));
    await vi.waitFor(() => { tick(32); expect(fail).toHaveBeenCalledTimes(2); });
    loop.destroy();
  });
});
