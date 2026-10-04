import { afterEach, describe, expect, it, vi } from "vitest";
import { createUpdateController, type UpdateDependencies, type UpdateState } from "./update-controller";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function setup(pendingSubscriptions = false) {
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const subscriptions: Array<ReturnType<typeof deferred<() => void>>> = [];
  const unlisten = vi.fn();
  const check = vi.fn<() => Promise<boolean>>().mockResolvedValue(false);
  const reportError = vi.fn();
  const deps: UpdateDependencies = {
    check,
    listen: (event, handler) => {
      handlers.set(event, handler as (event: { payload: unknown }) => void);
      if (!pendingSubscriptions) return Promise.resolve(unlisten);
      const subscription = deferred<() => void>();
      subscriptions.push(subscription);
      return subscription.promise;
    },
    schedule: (callback, delay) => {
      const timer = setTimeout(callback, delay);
      return () => clearTimeout(timer);
    },
    reportError,
  };
  const controller = createUpdateController(deps);
  const states: UpdateState[] = [];
  controller.subscribe(state => states.push(state));
  return {
    controller, check, subscriptions, unlisten, reportError, states,
    state: () => states[states.length - 1],
    emit: (event: string, payload: unknown = null) => handlers.get(event)!({ payload }),
  };
}

afterEach(() => vi.useRealTimers());

describe("update controller", () => {
  it("keeps ready state when download progress and the check result arrive late", async () => {
    const h = setup();
    const check = deferred<boolean>();
    h.check.mockReturnValue(check.promise);
    const action = h.controller.action();
    h.emit("update-available", { current_version: "1.0", version: "2.0" });
    h.emit("update-ready", { current_version: "1.0", version: "2.0" });
    h.emit("update-download-progress", { version: "2.0", received: 50, content_length: 100, chunk_len: 50 });
    check.resolve(true);
    await action;
    expect(h.state()).toMatchObject({ phase: "ready", version: "2.0", hint: "준비 완료", busy: false });
    h.controller.destroy();
  });

  it("caps progress below completion and suppresses actions while downloading", async () => {
    const h = setup();
    h.emit("update-download-progress", { version: "2.0", received: 100, content_length: 100, chunk_len: 100 });
    expect(h.state()).toMatchObject({ phase: "downloading", hint: "99% 다운로드" });
    await h.controller.action();
    expect(h.check).not.toHaveBeenCalled();
    h.controller.destroy();
  });

  it("retains ready version after restart failure and permits retry", async () => {
    const h = setup();
    h.emit("update-ready", { current_version: "1.0", version: "2.0" });
    h.check.mockRejectedValueOnce(new Error("restart failed")).mockResolvedValueOnce(true);
    await h.controller.action();
    expect(h.state()).toMatchObject({ phase: "ready", version: "2.0", busy: false, error: "restart failed" });
    await h.controller.action();
    expect(h.check).toHaveBeenCalledTimes(2);
    expect(h.state()).toMatchObject({ phase: "ready", error: "" });
    h.controller.destroy();
  });

  it("does not send duplicate IPC while a check is pending even if events clear busy", async () => {
    const h = setup();
    const check = deferred<boolean>();
    h.check.mockReturnValue(check.promise);
    const action = h.controller.action();
    h.emit("update-ready", { current_version: "1.0", version: "2.0" });
    await h.controller.action();
    expect(h.check).toHaveBeenCalledTimes(1);
    check.resolve(true);
    await action;
    h.controller.destroy();
  });

  it("returns to idle on failure and clears the error when checking again", async () => {
    vi.useFakeTimers();
    const h = setup();
    h.emit("update-available", { current_version: "1.0", version: "2.0" });
    h.emit("update-failed", "network unavailable");
    expect(h.state()).toMatchObject({ phase: "idle", error: "network unavailable" });
    await h.controller.action();
    expect(h.state()).toMatchObject({ phase: "idle", version: null, checked: true, error: "", hint: "최신 버전" });
    vi.advanceTimersByTime(2500);
    expect(h.state().checked).toBe(false);
    h.controller.destroy();
  });

  it("disposes subscriptions that resolve after destruction and ignores pending results", async () => {
    const h = setup(true);
    const check = deferred<boolean>();
    h.check.mockReturnValue(check.promise);
    const action = h.controller.action();
    h.controller.destroy();
    const count = h.states.length;
    h.emit("update-ready", { current_version: "1.0", version: "2.0" });
    for (const subscription of h.subscriptions) subscription.resolve(h.unlisten);
    check.resolve(false);
    await action;
    expect(h.unlisten).toHaveBeenCalledTimes(5);
    expect(h.states).toHaveLength(count);
    await h.controller.action();
    expect(h.check).toHaveBeenCalledTimes(1);
  });

  it("removes live listeners and cancels feedback timers on destruction", async () => {
    vi.useFakeTimers();
    const h = setup();
    await h.controller.action();
    expect(vi.getTimerCount()).toBe(1);
    h.controller.destroy();
    h.controller.destroy();
    expect(vi.getTimerCount()).toBe(0);
    expect(h.unlisten).toHaveBeenCalledTimes(5);
  });

  it("reports event registration failures without unhandled rejections", async () => {
    const h = setup(true);
    h.subscriptions[0].reject(new Error("registration failed"));
    await Promise.resolve();
    await Promise.resolve();
    expect(h.reportError).toHaveBeenCalledWith("update-available subscription failed", expect.any(Error));
    h.controller.destroy();
    for (const subscription of h.subscriptions.slice(1)) subscription.resolve(h.unlisten);
  });
});
