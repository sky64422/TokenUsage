/** Serialize capture, discard old-theme images, and release native exclusion on stop. */
export function createLiquidLoop<T>(io: {
  request: (tick: (time: number) => void) => number;
  cancel: (id: number) => void;
  interval: (time: number) => number;
  capture: () => Promise<T>;
  paint: (frame: T, time: number) => void;
  dispose: (frame: T) => void;
  release: () => Promise<void>;
  fail: (error: unknown) => void;
}) {
  let requested = false, enabled = false, destroyed = false, busy = false;
  let releasePending = false, epoch = 0, frame = 0, due = -Infinity, previousInterval = -1;
  let timer: ReturnType<typeof setTimeout> | undefined;
  function cancelScheduled() {
    io.cancel(frame); frame = 0;
    clearTimeout(timer); timer = undefined;
  }
  function schedule() {
    if (enabled && !destroyed && !busy && !frame && timer === undefined && previousInterval !== Infinity) {
      const delay = due - performance.now();
      if (delay > 0) {
        timer = setTimeout(() => { timer = undefined; frame = io.request(tick); }, delay);
      } else frame = io.request(tick);
    }
  }
  async function release() {
    if (!releasePending) return;
    releasePending = false;
    try { await io.release(); } catch (error) { io.fail(error); }
  }
  function tick(now: number) {
    frame = 0;
    clearTimeout(timer); timer = undefined;
    if (!enabled || destroyed) return;
    const interval = io.interval(now);
    if (interval !== previousInterval) { due = now; previousInterval = interval; }
    if (busy) return;
    if (now < due) { schedule(); return; }
    // Preserve fractional frame time; missed refreshes must not accumulate queued reads.
    due = Math.max(due, now - interval) + interval;
    busy = true;
    const generation = epoch;
    void (async () => {
      try {
        const result = await io.capture();
        try { if (enabled && !destroyed && epoch === generation) io.paint(result, now); }
        finally { io.dispose(result); }
      } catch (error) {
        if (generation === epoch) {
          enabled = false; epoch++;
          cancelScheduled();
          io.fail(error);
        }
        releasePending = true;
      } finally {
        await release();
        busy = false;
        if (due !== -Infinity) {
          const nextInterval = io.interval(performance.now());
          if (nextInterval !== previousInterval) due = now + nextInterval;
          previousInterval = nextInterval;
        }
        schedule();
      }
    })();
  }
  function setEnabled(value: boolean) {
    if (destroyed || requested === value) return;
    requested = enabled = value; epoch++;
    if (value) { due = -Infinity; previousInterval = -1; schedule(); return; }
    cancelScheduled(); releasePending = true;
    if (!busy) {
      busy = true;
      void release().finally(() => { busy = false; schedule(); });
    }
  }
  return {
    setEnabled,
    refresh() {
      due = -Infinity;
      previousInterval = -1;
      clearTimeout(timer); timer = undefined;
      schedule();
    },
    destroy() { setEnabled(false); destroyed = true; cancelScheduled(); },
  };
}
