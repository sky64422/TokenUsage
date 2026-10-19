import { afterEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { liquidContrast, mountLiquidContrast } from "./liquid-contrast";

const gray = (value: number) => new Uint8ClampedArray([value, value, value, 255]);
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); invoke.mockReset(); });
describe("liquid foreground contrast", () => {
  it("uses dark ink on white surfaces and white ink on dark surfaces", () => {
    expect(liquidContrast(gray(220))).toBe("light");
    expect(liquidContrast(gray(35), "light")).toBe("dark");
  });
  it("holds its previous mode within the hysteresis band", () => {
    expect(liquidContrast(gray(158), "light")).toBe("light");
    expect(liquidContrast(gray(158), "dark")).toBe("dark");
  });
  it("ignores isolated bright or dark pixels in a sampled margin", () => {
    expect(liquidContrast(new Uint8ClampedArray([...gray(220), ...gray(20), ...gray(220)]))).toBe("light");
    expect(liquidContrast(new Uint8ClampedArray([...gray(20), ...gray(220), ...gray(20)]), "light")).toBe("dark");
  });
  it("serializes sampling and discards a pending result after disabling", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("document", { hidden: false });
    vi.stubGlobal("devicePixelRatio", 1);
    const surface = () => ({ dataset: {} as Record<string, string>, hidden: false,
      classList: { contains: () => false },
      getBoundingClientRect: () => ({ x: 10, y: 10, width: 64, height: 100 }) });
    const rail = surface(), detail = surface();
    let resolve!: (bytes: ArrayBuffer) => void;
    invoke.mockImplementation(() => new Promise<ArrayBuffer>(done => { resolve = done; }));
    const controller = mountLiquidContrast(rail as unknown as HTMLElement, rail as unknown as HTMLElement, detail as unknown as HTMLElement);
    controller.setActive(true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(invoke).toHaveBeenCalledTimes(1);
    controller.setActive(false);
    resolve(new ArrayBuffer(0));
    await vi.advanceTimersByTimeAsync(5000);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(rail.dataset.contrast).toBeUndefined();
    expect(detail.dataset.contrast).toBeUndefined();
  });
  it("bounds visible probes to four cycles per second and skips hidden surfaces", async () => {
    vi.useFakeTimers();
    const documentState = { hidden: false };
    vi.stubGlobal("document", documentState);
    vi.stubGlobal("devicePixelRatio", 1);
    const surface = () => ({ dataset: {}, hidden: false,
      classList: { contains: () => false },
      getBoundingClientRect: () => ({ x: 10, y: 10, width: 64, height: 100 }) });
    const rail = surface(), detail = surface();
    invoke.mockResolvedValue(new ArrayBuffer(0));
    const controller = mountLiquidContrast(rail as unknown as HTMLElement, rail as unknown as HTMLElement, detail as unknown as HTMLElement);
    controller.setActive(true);
    await vi.advanceTimersByTimeAsync(1119);
    expect(invoke).toHaveBeenCalledTimes(8);
    detail.hidden = true;
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledTimes(12);
    documentState.hidden = true;
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledTimes(12);
    controller.setActive(false);
  });

});
