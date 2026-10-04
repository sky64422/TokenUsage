import { describe, expect, it } from "vitest";
import { createNotchDrag } from "./notch-drag";
import type { DragNotice } from "./notch-drag";

function fixture(begin: (id: number) => Promise<void> = async () => {}) {
  const finished: Array<[number, boolean]> = [];
  const notices: DragNotice[] = [];
  const errors: unknown[] = [];
  const captured = new Set<number>();
  const capture = {
    setPointerCapture: (id: number) => { captured.add(id); },
    hasPointerCapture: (id: number) => captured.has(id),
    releasePointerCapture: (id: number) => { captured.delete(id); },
  };
  const drag = createNotchDrag({
    begin, finish: async (id, cancel) => { finished.push([id, cancel]); },
    onStart: () => {}, onNotice: event => notices.push(event),
    onState: () => {}, fail: error => errors.push(error),
  });
  const start = () => drag.start({ id: 7, x: 100, y: 100, capture });
  const notice = (id: number, active: boolean, done: boolean): DragNotice => ({
    id, active, finished: done, error: null,
    placement: { edge: "right", monitor_hint: null, offset: .5 },
  });
  return { drag, start, captured, finished, notices, errors, notice };
}

describe("notch drag lifecycle", () => {
  it("queues cancellation behind native begin and releases capture on completion", async () => {
    let resolve!: () => void;
    const f = fixture(() => new Promise(done => { resolve = done; }));
    f.start();
    const id = f.drag.sessionId!;
    f.drag.finish(true);
    await Promise.resolve();
    expect(f.finished).toEqual([]);
    resolve();
    await f.drag.settled();
    expect(f.finished).toEqual([[id, true]]);
    expect(f.captured.has(7)).toBe(true);
    f.drag.notice(f.notice(id, true, true));
    expect(f.captured.size).toBe(0);
    expect(f.drag.sessionId).toBeNull();
    expect(f.drag.dragging).toBe(false);
  });

  it("cleans up failed begin and permits another gesture", async () => {
    const f = fixture(async () => { throw new Error("begin failed"); });
    f.start();
    await f.drag.settled();
    expect(f.errors).toHaveLength(1);
    expect(f.captured.size).toBe(0);
    expect(f.drag.sessionId).toBeNull();
    expect(f.start()).toBe(true);
    await f.drag.settled();
  });

  it("ignores old completions without clearing a newer session or capture", async () => {
    const f = fixture();
    f.start();
    const old = f.drag.sessionId!;
    f.drag.notice(f.notice(old, true, true));
    f.start();
    const current = f.drag.sessionId;
    f.drag.notice(f.notice(old, true, true));
    expect(f.drag.sessionId).toBe(current);
    expect(f.captured.has(7)).toBe(true);
    expect(f.notices).toHaveLength(1);
    await f.drag.settled();
  });

  it("preserves short clicks, suppresses threshold travel, and permits keyboard activation", async () => {
    const f = fixture();
    f.start();
    f.drag.move(7, 102, 100, 5);
    expect(f.drag.consumeClick(1)).toBe(false);
    f.drag.move(99, 140, 100, 5);
    expect(f.drag.consumeClick(1)).toBe(false);
    f.drag.move(7, 103, 104, 5);
    expect(f.drag.consumeClick(1)).toBe(true);
    expect(f.drag.consumeClick(1)).toBe(false);
    f.drag.move(7, 108, 100, 5);
    expect(f.drag.consumeClick(0)).toBe(false);
    await f.drag.settled();
  });

  it("cancels a pending drag and releases capture during disposal", async () => {
    const f = fixture();
    f.start();
    const id = f.drag.sessionId!;
    f.drag.destroy();
    expect(f.captured.size).toBe(0);
    expect(f.start()).toBe(false);
    f.drag.notice(f.notice(id, true, false));
    expect(f.notices).toEqual([]);
    await f.drag.settled();
    expect(f.finished).toEqual([[id, true]]);
  });
});
