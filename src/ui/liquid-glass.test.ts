import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { mountLiquidGlass } from "./liquid-glass";

class MockElement extends EventTarget {
  dataset: Record<string, string> = {};
  hidden = false;
  className = "";
  classes = new Set<string>();
  classList = {
    contains: (name: string) => this.classes.has(name),
    toggle: (name: string, force?: boolean) => {
      if (force ?? !this.classes.has(name)) this.classes.add(name);
      else this.classes.delete(name);
    },
  };
  children: MockElement[] = [];
  style: Record<string, any> = {
    setProperty: (k: string, v: string) => { (this.style as Record<string, any>)[k] = v; },
  };
  width = 0;
  height = 0;
  rect = { x: 10, y: 20, width: 64, height: 300 };
  setAttribute() {}
  append(...nodes: MockElement[]) { this.children.push(...nodes); }
  prepend(...nodes: MockElement[]) { this.children.unshift(...nodes); }
  before(...nodes: MockElement[]) { this.children.push(...nodes); }
  querySelector() {
    return { before: (...nodes: MockElement[]) => this.children.push(...nodes), removeAttribute: () => {} };
  }
  querySelectorAll() { return []; }
  remove() { this.hidden = true; }
  getBoundingClientRect() { return this.rect; }
  getContext() {
    return {
      putImageData: vi.fn(),
      drawImage: vi.fn(),
      getImageData: () => ({ data: new Uint8ClampedArray(16) }),
      createImageData: () => ({ data: new Uint8ClampedArray(16) }),
      setTransform: vi.fn(),
      resetTransform: vi.fn(),
      fill: vi.fn(),
      beginPath: vi.fn(),
      roundRect: vi.fn(),
    };
  }
}

const settle = async () => {
  for (let index = 0; index < 12; index++) await Promise.resolve();
};

function fixture() {
  const shell = new MockElement(), rail = new MockElement(), detail = new MockElement();
  shell.dataset.theme = "liquid";
  rail.dataset.edge = "right";
  return {
    shell,
    rail,
    detail,
    renderer: mountLiquidGlass(
      shell as unknown as HTMLElement,
      rail as unknown as HTMLElement,
      detail as unknown as HTMLElement,
    ),
  };
}

beforeEach(() => {
  const doc = Object.assign(new EventTarget(), {
    hidden: false,
    createElement: () => new MockElement(),
    createElementNS: () => new MockElement(),
  });
  vi.stubGlobal("document", doc);
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("devicePixelRatio", 2);
  vi.stubGlobal("innerWidth", 1920);
  vi.stubGlobal("innerHeight", 1080);
  vi.stubGlobal("requestAnimationFrame", vi.fn(cb => { setTimeout(cb, 1); return 1; }));
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
  vi.stubGlobal("getComputedStyle", () => ({
    display: "block",
    visibility: "visible",
    borderRadius: "16px",
    getPropertyValue: () => "1",
  }));
  const Observer = class { observe() {} disconnect() {} };
  vi.stubGlobal("MutationObserver", Observer);
  vi.stubGlobal("ResizeObserver", Observer);
  invoke.mockReset();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("Liquid desktop capture lifecycle", () => {
  it("mounts canvas layers and invokes release on cleanup", async () => {
    invoke.mockImplementation((command: string) => {
      if (command === "capture_liquid_backdrop") return Promise.resolve(new ArrayBuffer(0));
      return Promise.resolve();
    });

    const { renderer, shell, rail, detail } = fixture();
    expect(shell.children.length).toBeGreaterThan(0);
    expect(rail.children.length).toBeGreaterThan(0);
    expect(detail.children.length).toBeGreaterThan(0);

    renderer.setEnabled(true);
    await settle();

    renderer.destroy();
    await settle();

    const released = invoke.mock.calls.some(([cmd]) => cmd === "release_liquid_backdrop");
    expect(released).toBe(true);
  });

  it("handles theme disable and visibility changes", async () => {
    invoke.mockImplementation(() => Promise.resolve());
    const { renderer, shell } = fixture();

    renderer.setEnabled(false);
    await settle();
    expect(shell.dataset.liquidStatus).toBeUndefined();

    renderer.setVisible(false);
    await settle();

    renderer.destroy();
  });
});
