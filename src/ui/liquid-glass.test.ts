import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { mountLiquidGlass } from "./liquid-glass";

class Element extends EventTarget {
  dataset: Record<string, string> = {};
  hidden = false;
  className = "";
  classes = new Set<string>();
  classList = { contains: (name: string) => this.classes.has(name) };
  children: Element[] = [];
  inner: Element | null = null;
  offsetWidth = 0;
  offsetHeight = 0;
  rect = { x: 10, y: 20, width: 64, height: 300 };
  setAttribute() {}
  querySelector(selector: string) { return selector === ".notch-surface" ? this.inner : { before: (child: Element) => this.children.push(child) }; }
  prepend(child: Element) { this.children.push(child); }
  remove() { this.hidden = true; }
  getBoundingClientRect() { return this.rect; }
  getAnimations() { return []; }
}
const settle = async () => { for (let index = 0; index < 12; index++) await Promise.resolve(); };
function fixture(inner?: Element) {
  const shell = new Element(), rail = new Element(), detail = new Element();
  shell.dataset.theme = "liquid"; rail.dataset.edge = "right";
  rail.inner = inner ?? null;
  return { shell, rail, detail, renderer: mountLiquidGlass(shell as unknown as HTMLElement,
    rail as unknown as HTMLElement, detail as unknown as HTMLElement) };
}
beforeEach(() => {
  const document = Object.assign(new EventTarget(), { hidden: false, createElement: () => new Element() });
  vi.stubGlobal("document", document);
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("devicePixelRatio", 2);
  vi.stubGlobal("requestAnimationFrame", vi.fn(() => 1));
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  vi.stubGlobal("getComputedStyle", () => ({ display: "block", visibility: "visible", borderRadius: "16px", getPropertyValue: () => "" }));
  const Observer = class { observe() {} disconnect() {} };
  vi.stubGlobal("MutationObserver", Observer); vi.stubGlobal("ResizeObserver", Observer);
  invoke.mockReset();
});
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("native Liquid geometry lifecycle", () => {
  it("measures the transformed inner notch and preserves its elliptical length radius", async () => {
    invoke.mockImplementation((command: string) => Promise.resolve(command === "begin_liquid_backdrop" ? 7 : undefined));
    const inner = new Element();
    inner.rect = { x: 11, y: 19, width: 70.4, height: 318 };
    inner.offsetWidth = 64; inner.offsetHeight = 300;
    const { renderer } = fixture(inner);
    renderer.setEnabled(true); await settle();
    const update = invoke.mock.calls.find(([command]) => command === "sync_liquid_backdrop");
    expect(update?.[1].surfaces[0]).toEqual({ slot: 0, x: 22, y: 38, width: 141, height: 636,
      edge: "right", radius: 67.84, blur: 6 });
    renderer.destroy(); await settle();
  });
  it("coalesces geometry behind a delayed sync and only acknowledges the latest shape", async () => {
    const pending: Array<() => void> = [];
    invoke.mockImplementation((command: string) => command === "begin_liquid_backdrop" ? Promise.resolve(7)
      : command === "sync_liquid_backdrop" ? new Promise<void>(resolve => pending.push(resolve)) : Promise.resolve());
    const { renderer, rail, shell } = fixture();
    renderer.setEnabled(true); await settle();
    rail.rect.x = 11; renderer.setEnabled(true);
    rail.rect.x = 12; renderer.setEnabled(true);
    expect(invoke.mock.calls.filter(([command]) => command === "sync_liquid_backdrop")).toHaveLength(1);
    pending[0](); await settle();
    const updates = invoke.mock.calls.filter(([command]) => command === "sync_liquid_backdrop");
    expect(updates).toHaveLength(2);
    expect(updates[1][1]).toMatchObject({ session: 7, revision: 2, surfaces: [
      { slot: 0, x: 24, y: 40, width: 128, height: 600, edge: "right", radius: 0, blur: 6 },
      { slot: 1, radius: 32, blur: 20 },
    ] });
    expect(shell.dataset.liquidStatus).toBeUndefined();
    pending[1](); await settle();
    expect(shell.dataset.liquidStatus).toBe("ready");
    renderer.destroy(); await settle();
  });

  it("releases a delayed old sync before reopening the same geometry", async () => {
    const pending: Array<() => void> = [];
    invoke.mockImplementation((command: string) => command === "begin_liquid_backdrop" ? Promise.resolve(9)
      : command === "sync_liquid_backdrop" ? new Promise<void>(resolve => pending.push(resolve)) : Promise.resolve());
    const { renderer, shell } = fixture();
    renderer.setEnabled(true); await settle();
    renderer.setEnabled(false); renderer.setEnabled(true);
    pending[0](); await settle();
    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      "begin_liquid_backdrop", "sync_liquid_backdrop", "release_liquid_backdrop", "sync_liquid_backdrop",
    ]);
    expect(invoke.mock.calls.slice(1).map(([, args]) => args.revision)).toEqual([1, 2, 3]);
    expect(shell.dataset.liquidStatus).toBeUndefined();
    pending[1](); await settle();
    expect(shell.dataset.liquidStatus).toBe("ready");
    renderer.destroy(); await settle();
  });

  it("cleans up a late begin after destruction without applying native surfaces", async () => {
    let begin!: (session: number) => void;
    invoke.mockImplementation((command: string) => command === "begin_liquid_backdrop"
      ? new Promise<number>(resolve => { begin = resolve; }) : Promise.resolve());
    const { renderer, shell } = fixture();
    renderer.setEnabled(true); renderer.destroy(); begin(11); await settle();
    expect(invoke.mock.calls).toEqual([
      ["begin_liquid_backdrop"], ["release_liquid_backdrop", { session: 11, revision: 1 }],
    ]);
    expect(shell.dataset.liquidStatus).toBeUndefined();
  });

  it("clears on failure and retries only after an explicit enable", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    let fail = true;
    invoke.mockImplementation((command: string) => {
      if (command === "begin_liquid_backdrop") return Promise.resolve(7);
      if (command === "sync_liquid_backdrop" && fail) return Promise.reject(Error("native sync failed"));
      return Promise.resolve();
    });
    const { renderer, shell, rail } = fixture();
    renderer.setEnabled(true); await settle();
    expect(shell.dataset.liquidStatus).toBe("unavailable");
    expect(rail.children[0].hidden).toBe(true);
    expect(invoke.mock.calls[invoke.mock.calls.length - 1]?.[0]).toBe("release_liquid_backdrop");
    expect(error).toHaveBeenCalledOnce();
    fail = false; renderer.setEnabled(true); await settle();
    expect(shell.dataset.liquidStatus).toBe("ready");
    expect(invoke.mock.calls.filter(([command]) => command === "begin_liquid_backdrop")).toHaveLength(1);
    renderer.destroy(); await settle();
  });
});
