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
    getPropertyValue: (k: string) => (this.style as Record<string, any>)[k] ?? "",
  };
  width = 0;
  height = 0;
  rect = { x: 10, y: 20, width: 64, height: 300 };
  offsetWidth = 0;
  offsetHeight = 0;
  offsetLeft = 0;
  offsetTop = 0;
  offsetParent: MockElement | null = null;
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

  it("releases capture lease when static and pixels are unchanged", async () => {
    let now = 1000;
    vi.stubGlobal("performance", { now: () => now });
    let frameCb: ((time: number) => void) | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn(cb => {
      frameCb = cb;
      return 1;
    }));

    invoke.mockImplementation((command: string) => {
      if (command === "capture_liquid_backdrop") {
        return Promise.resolve(new ArrayBuffer(0));
      }
      return Promise.resolve();
    });

    const { renderer } = fixture();
    renderer.setEnabled(true);

    expect(frameCb).toBeTruthy();
    frameCb!(now);
    await settle();

    // Advance beyond STATIC_AFTER_MS (2000ms)
    now += 2500;
    frameCb!(now);
    await settle();

    const released = invoke.mock.calls.some(([cmd]) => cmd === "release_liquid_backdrop");
    expect(released).toBe(true);

    renderer.destroy();
  });

  it("refreshes capture immediately on pointer interaction", async () => {
    let now = 1000;
    vi.stubGlobal("performance", { now: () => now });
    let frameCb: ((time: number) => void) | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn(cb => {
      frameCb = cb;
      return 1;
    }));

    invoke.mockImplementation((command: string) => {
      if (command === "capture_liquid_backdrop") {
        return Promise.resolve(new ArrayBuffer(0));
      }
      return Promise.resolve();
    });

    const { renderer, rail } = fixture();
    renderer.setEnabled(true);
    frameCb!(now);
    await settle();

    now += 2500;
    frameCb!(now);
    await settle();

    invoke.mockClear();

    // User moves pointer over rail
    rail.dispatchEvent(new Event("pointerenter"));
    expect(frameCb).toBeTruthy();
    frameCb!(now);
    await settle();

    let capturedAgain = invoke.mock.calls.some(([cmd]) => cmd === "capture_liquid_backdrop");
    expect(capturedAgain).toBe(true);

    // Let it settle again and test pointerdown on window
    now += 2500;
    frameCb!(now);
    await settle();

    invoke.mockClear();
    window.dispatchEvent(new Event("pointerdown"));
    expect(frameCb).toBeTruthy();
    frameCb!(now);
    await settle();

    capturedAgain = invoke.mock.calls.some(([cmd]) => cmd === "capture_liquid_backdrop");
    expect(capturedAgain).toBe(true);

    renderer.destroy();
  });

  it("uses layout dimensions instead of transformed rect to prevent canvas flash during drop animation", async () => {
    let now = 1000;
    vi.stubGlobal("performance", { now: () => now });
    let frameCb: ((time: number) => void) | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn(cb => {
      frameCb = cb;
      return 1;
    }));

    invoke.mockImplementation((command: string) => {
      if (command === "capture_liquid_backdrop") {
        return Promise.resolve(new ArrayBuffer(0));
      }
      return Promise.resolve();
    });

    const { renderer, detail } = fixture();
    detail.offsetWidth = 260;
    detail.offsetHeight = 400;
    detail.offsetLeft = 1600;
    detail.offsetTop = 100;
    detail.offsetParent = {} as any;
    detail.rect = { x: 1614, y: 100, width: 213, height: 400 };

    renderer.setEnabled(true);
    expect(frameCb).toBeTruthy();
    frameCb!(now);
    await settle();

    const firstCall = invoke.mock.calls.find(([cmd, args]) => cmd === "capture_liquid_backdrop" && args?.surface === 1);
    expect(firstCall).toBeTruthy();
    expect(firstCall![1].width).toBe(520);
    expect(firstCall![1].height).toBe(800);
    expect(firstCall![1].generation).toBe(2);

    invoke.mockClear();

    now += 50;
    detail.rect = { x: 1608, y: 100, width: 240, height: 400 };
    frameCb!(now);
    await settle();

    const secondCall = invoke.mock.calls.find(([cmd, args]) => cmd === "capture_liquid_backdrop" && args?.surface === 1);
    expect(secondCall).toBeTruthy();
    expect(secondCall![1].width).toBe(520);
    expect(secondCall![1].height).toBe(800);
    expect(secondCall![1].generation).toBe(2);

    renderer.destroy();
  });

  it("releases liquid backdrop when window blurs to allow screen capture", async () => {
    const { renderer } = fixture();
    renderer.setEnabled(true);
    invoke.mockClear();

    window.dispatchEvent(new Event("blur"));
    expect(invoke).toHaveBeenCalledWith("release_liquid_backdrop");

    renderer.destroy();
  });

  it("releases liquid backdrop on 0-byte settling cache-hit frame when not dragging", async () => {
    let now = 1000;
    vi.stubGlobal("performance", { now: () => now });
    let frameCb: ((time: number) => void) | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn(cb => {
      frameCb = cb;
      return 1;
    }));

    invoke.mockImplementation((command: string) => {
      if (command === "capture_liquid_backdrop") {
        return Promise.resolve(new ArrayBuffer(0));
      }
      return Promise.resolve();
    });

    const { renderer, rail } = fixture();
    expect(rail.classList.contains("is-dragging")).toBe(false);
    renderer.setEnabled(true);
    invoke.mockClear();

    frameCb!(now);
    await settle();

    expect(invoke).toHaveBeenCalledWith(
      "capture_liquid_backdrop",
      expect.objectContaining({ dragging: false }),
    );
    expect(invoke).toHaveBeenCalledWith("release_liquid_backdrop");

    renderer.destroy();
  });

  it("passes dragging: true to capture_liquid_backdrop while actively dragging", async () => {
    let now = 1000;
    vi.stubGlobal("performance", { now: () => now });
    let frameCb: ((time: number) => void) | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn(cb => {
      frameCb = cb;
      return 1;
    }));

    invoke.mockImplementation((command: string) => {
      if (command === "capture_liquid_backdrop") {
        return Promise.resolve(new ArrayBuffer(0));
      }
      return Promise.resolve();
    });

    const { renderer, rail } = fixture();
    rail.classList.toggle("is-dragging", true);
    renderer.setEnabled(true);
    invoke.mockClear();

    frameCb!(now);
    await settle();

    expect(invoke).toHaveBeenCalledWith(
      "capture_liquid_backdrop",
      expect.objectContaining({ dragging: true }),
    );
    // While dragging, release_liquid_backdrop is NOT called per-frame
    expect(invoke).not.toHaveBeenCalledWith("release_liquid_backdrop");

    renderer.destroy();
  });
});
