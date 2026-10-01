import claude from "../assets/marks/claude.svg";
import codex from "../assets/marks/codex.svg";
import grok from "../assets/marks/grok.svg";
import { headline } from "./notch-state";
import { PROVIDER_IDS } from "./types";
import type { ProviderId, ProviderSnapshot, NotchLayout, Rect } from "./types";

const MARKS = { claude, codex, grok };
export function mountNotch(
  root: HTMLElement,
  callbacks: {
    hover: (id: ProviderId) => void;
    pin: (id: ProviderId) => void;
  },
) {
  root.tabIndex = 0;
  root.setAttribute("aria-label", "AI usage. Drag to move; right-click or Shift+F10 for settings.");
  root.innerHTML = `<svg class="notch-shape" aria-hidden="true"><path/></svg><div class="notch-cells"></div>`;
  const cells = root.querySelector<HTMLElement>(".notch-cells")!;
  const buttons = new Map<ProviderId, HTMLButtonElement>();
  let restoringFocus = false;
  for (const id of PROVIDER_IDS) {
    const b = document.createElement("button");
    b.className = "notch-cell";
    b.dataset.id = id;
    b.innerHTML = `<span class="ring-wrap"><svg viewBox="0 0 44 44" aria-hidden="true"><circle class="ring-track" cx="22" cy="22" r="19"/><circle class="ring-fill" cx="22" cy="22" r="19" pathLength="100"/></svg><img src="${MARKS[id]}" alt="" draggable="false"/></span><span class="ring-pct">—</span><span class="ring-period">No data</span><span class="ring-status" hidden>!</span>`;
    b.addEventListener("pointerenter", () => callbacks.hover(id));
    b.addEventListener("focus", () => {
      if (!restoringFocus) callbacks.hover(id);
    });
    b.addEventListener("click", () => callbacks.pin(id));
    cells.append(b);
    buttons.set(id, b);
  }
  function shape(layout: NotchLayout) {
    const {
      depth: DEPTH,
      shoulder: SHOULDER,
      inner_radius: INNER,
    } = layout.metrics;
    root.style.setProperty("--notch-depth", `${layout.metrics.depth}px`);
    root.style.setProperty("--notch-cell", `${layout.metrics.cell}px`);
    root.style.setProperty("--notch-inset", `${layout.metrics.inset}px`);
    const vertical = layout.edge === "left" || layout.edge === "right";
    const length =
      (vertical ? layout.notch.height : layout.notch.width) / layout.scale;
    const svg = root.querySelector("svg")!;
    svg.setAttribute(
      "viewBox",
      `0 0 ${vertical ? DEPTH : length} ${vertical ? length : DEPTH}`,
    );
    const path = svg.querySelector("path")!;
    path.setAttribute(
      "d",
      `M${DEPTH} 0 A${SHOULDER} ${SHOULDER} 0 0 1 ${INNER} ${SHOULDER} A${INNER} ${INNER} 0 0 0 0 ${SHOULDER + INNER} L0 ${length - SHOULDER - INNER} A${INNER} ${INNER} 0 0 0 ${INNER} ${length - SHOULDER} A${SHOULDER} ${SHOULDER} 0 0 1 ${DEPTH} ${length} Z`,
    );
    path.setAttribute(
      "transform",
      layout.edge === "left"
        ? `translate(${DEPTH} 0) scale(-1 1)`
        : layout.edge === "bottom"
          ? "matrix(0 1 1 0 0 0)"
          : layout.edge === "top"
            ? `matrix(0 -1 1 0 0 ${DEPTH})`
            : "",
    );
  }
  return {
    update(snaps: ProviderSnapshot[], enabled: ProviderId[]) {
      for (const [id, b] of buttons) {
        b.hidden = !enabled.includes(id);
        const s = snaps.find((v) => v.provider_id === id);
        if (!s) {
          b.setAttribute("aria-label", `${id}: waiting for usage`);
          continue;
        }
        const h = headline(s);
        b.className = `notch-cell ${h.level}`;
        b.querySelector(".ring-pct")!.textContent = h.text;
        b.querySelector(".ring-period")!.textContent = h.label;
        b.querySelector<SVGElement>(".ring-fill")!.style.strokeDasharray =
          `${h.fill} 100`;
        b.querySelector<HTMLElement>(".ring-status")!.hidden =
          !h.degraded && !h.secondaryRisk;
        b.querySelector<HTMLElement>(".ring-status")!.title = h.secondaryRisk;
        b.setAttribute(
          "aria-label",
          `${s.display_name}: ${h.label}, ${h.text} used${h.degraded ? `, ${s.status.replaceAll("_", " ")}` : ""}${h.secondaryRisk ? `, other limits: ${h.secondaryRisk}` : ""}. Click to keep open.`,
        );
      }
    },
    layout(l: NotchLayout) {
      root.dataset.edge = l.edge;
      position(root, l.notch, l);
      shape(l);
    },
    select(id: ProviderId | null, pinned: boolean) {
      for (const [key, b] of buttons)
        b.setAttribute("aria-pressed", String(pinned && key === id));
    },
    focus(id: ProviderId | null) {
      restoringFocus = true;
      if (id) buttons.get(id)?.focus();
      else root.focus();
      restoringFocus = false;
    },
  };
}
export function position(el: HTMLElement, r: Rect, l: NotchLayout) {
  Object.assign(el.style, {
    left: `${(r.x - l.window.x) / l.scale}px`,
    top: `${(r.y - l.window.y) / l.scale}px`,
    width: `${r.width / l.scale}px`,
    height: `${r.height / l.scale}px`,
  });
}
