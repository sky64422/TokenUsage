import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

class MockElement extends EventTarget {
  dataset: Record<string, string> = {};
  hidden = false;
  className = "";
  children: MockElement[] = [];
  style: Record<string, string> = {};
  attributes: Record<string, string> = {};
  textContent = "";
  disabled = false;

  get innerHTML() {
    return "";
  }
  set innerHTML(html: string) {
    this.children = [];
    if (html.includes("update-version-tag")) {
      const v = new MockElement();
      v.className = "update-version-tag";
      this.children.push(v);
    }
    if (html.includes("update-btn-cancel")) {
      const c = new MockElement();
      c.className = "update-action-btn update-btn-cancel";
      this.children.push(c);
    }
    if (html.includes("update-btn-confirm")) {
      const r = new MockElement();
      r.className = "update-action-btn update-btn-confirm";
      this.children.push(r);
    }
  }

  setAttribute(k: string, v: string) {
    this.attributes[k] = v;
  }
  getAttribute(k: string) {
    return this.attributes[k];
  }
  removeAttribute(k: string) {
    delete this.attributes[k];
  }
  append(...nodes: MockElement[]) {
    this.children.push(...nodes);
  }
  querySelector<T = MockElement>(selector: string): T | null {
    const sel = selector.replace(".", "");
    const find = (el: MockElement): MockElement | null => {
      if (el.className.split(" ").includes(sel)) return el;
      for (const child of el.children) {
        const found = find(child);
        if (found) return found;
      }
      return null;
    };
    return find(this) as unknown as T;
  }
  click() {
    this.dispatchEvent(new Event("click"));
  }
}

import { mountUpdateCard } from "./update-card";

describe("mountUpdateCard", () => {
  let root: MockElement;

  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
    root = new MockElement();
  });

  it("updates version tag when setUpdate is called", () => {
    const card = mountUpdateCard(root as unknown as HTMLElement);
    card.setUpdate({ current_version: "0.5.13", version: "0.5.14" });
    const versionEl = root.querySelector<MockElement>(".update-version-tag");
    expect(versionEl?.textContent).toBe("v0.5.14");
    card.destroy();
  });

  it("clicking cancel invokes onDismiss", () => {
    const onDismiss = vi.fn();
    const card = mountUpdateCard(root as unknown as HTMLElement, { onDismiss });
    card.setUpdate({ current_version: "0.5.13", version: "0.5.14" });

    const cancelBtn = root.querySelector<MockElement>(".update-btn-cancel")!;
    cancelBtn.click();
    expect(onDismiss).toHaveBeenCalled();
    card.destroy();
  });

  it("clicking confirm invokes check_for_updates and calls onConfirm", async () => {
    const onConfirm = vi.fn();
    const card = mountUpdateCard(root as unknown as HTMLElement, { onConfirm });
    card.setUpdate({ current_version: "0.5.13", version: "0.5.14" });

    const confirmBtn = root.querySelector<MockElement>(".update-btn-confirm")!;
    confirmBtn.click();
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith("check_for_updates");
    expect(onConfirm).toHaveBeenCalled();
    card.destroy();
  });
});
