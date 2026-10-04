import { describe, expect, it } from "vitest";
import { createNotchSurface } from "./notch-surface";
import type { NotchLayout } from "./types";

const layout: NotchLayout = {
  edge: "right", anchor_x: "right", anchor_y: "top", scale: 1, monitor: "test",
  notch: { x: 936, y: 200, width: 64, height: 232 },
  window: { x: 600, y: 0, width: 400, height: 800 }, detail: null,
  metrics: { rest_depth: 64, rest_length: 232, depth: 64, cell: 72,
    shoulder: 32, inner_radius: 32, inset: 44, detail_radius: 16, drag_threshold: 5 },
};
const input = { expanded: true, height: 100, target: 80, providers: ["claude" as const] };

describe("notch surface requests", () => {
  it("serializes requests and only paints the latest requested layout", async () => {
    const resolvers: Array<(value: NotchLayout) => void> = [];
    const heights: number[] = [];
    const painted: NotchLayout[] = [];
    const surface = createNotchSurface({
      send: request => { heights.push(request.height); return new Promise(resolve => resolvers.push(resolve)); },
      paint: value => painted.push(value), fail: error => { throw error; },
    });
    surface.request(input);
    const first = surface.settled();
    await Promise.resolve();
    surface.request({ ...input, height: 234 });
    expect(heights).toEqual([100]);
    resolvers[0](layout);
    await first;
    expect(painted).toEqual([]);
    expect(heights).toEqual([100, 234]);
    const latest = { ...layout, detail: { x: 0, y: 0, width: 260, height: 234 } };
    resolvers[1](latest);
    await surface.settled();
    expect(painted).toEqual([latest]);
  });

  it("deduplicates identical requests but retries after failure", async () => {
    let attempts = 0;
    const errors: unknown[] = [];
    const painted: NotchLayout[] = [];
    const surface = createNotchSurface({
      send: async () => { if (++attempts === 1) throw new Error("native failure"); return layout; },
      paint: value => painted.push(value), fail: error => errors.push(error),
    });
    surface.request(input);
    surface.request(input);
    await surface.settled();
    expect(attempts).toBe(1);
    expect(errors).toHaveLength(1);
    surface.request(input);
    await surface.settled();
    expect(painted).toEqual([layout]);
    surface.request(input);
    await surface.settled();
    expect(attempts).toBe(2);
  });

  it("does not paint a response or submit queued requests after disposal", async () => {
    let resolve!: (value: NotchLayout) => void;
    let sends = 0;
    const painted: NotchLayout[] = [];
    const surface = createNotchSurface({
      send: () => { sends++; return new Promise(done => { resolve = done; }); },
      paint: value => painted.push(value), fail: error => { throw error; },
    });
    surface.request(input);
    await Promise.resolve();
    surface.request({ ...input, height: 234 });
    surface.destroy();
    resolve(layout);
    await surface.settled();
    expect(sends).toBe(1);
    expect(painted).toEqual([]);
  });
});
