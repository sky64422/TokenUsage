import claude from "../assets/marks/claude.svg";
import codex from "../assets/marks/codex.svg";
import grok from "../assets/marks/grok.svg";
import { headline } from "./notch-state";
import { PROVIDER_IDS } from "./types";
import type { ProviderId, ProviderSnapshot, ProviderActivity, NotchLayout, Rect } from "./types";

import agy from "../assets/marks/agy.svg";

const MARKS = { claude, codex, grok, agy };
export function mountNotch(
  root: HTMLElement,
  callbacks: {
    hover: (id: ProviderId) => void;
    pin: (id: ProviderId) => void;
  },
) {
  root.tabIndex = 0;
  root.setAttribute("aria-label", "AI usage. Drag to move; right-click or Shift+F10 for settings.");
  root.innerHTML = `<span class="notch-rest" aria-hidden="true"></span><div class="notch-surface"><svg class="notch-shape" aria-hidden="true"><path/></svg><div class="notch-cells"></div></div>`;
  const cells = root.querySelector<HTMLElement>(".notch-cells")!;
  const buttons = new Map<ProviderId, HTMLButtonElement>();
  let restoringFocus = false;
  for (const id of PROVIDER_IDS) {
    const b = document.createElement("button");
    b.className = "notch-cell";
    b.dataset.id = id;
    b.innerHTML = `<span class="ring-wrap"><svg viewBox="0 0 44 44" aria-hidden="true"><circle class="ring-track" cx="22" cy="22" r="19"/><circle class="ring-fill" cx="22" cy="22" r="19" pathLength="100"/></svg><img src="${MARKS[id]}" alt="" draggable="false"/><span class="ring-status" hidden>!</span></span><span class="ring-meta"><span class="ring-pct">—</span><span class="ring-period">No data</span></span>`;
    b.addEventListener("pointerenter", () => callbacks.hover(id));
    const orbit = document.createElement("span");
    orbit.className = "activity-orbit";
    orbit.setAttribute("aria-hidden", "true");
    b.querySelector(".ring-wrap")!.append(orbit);
    b.addEventListener("focus", () => {
      if (!restoringFocus) callbacks.hover(id);
    });
    b.addEventListener("click", () => callbacks.pin(id));
    cells.append(b);
    buttons.set(id, b);
  }
  let lastShapeKey = "";
  function shape(layout: NotchLayout) {
    const vertical = layout.edge === "left" || layout.edge === "right";
    const length =
      (vertical ? layout.notch.height : layout.notch.width) / layout.scale;
    const shapeKey = `${layout.edge}:${length}:${layout.scale}`;
    if (shapeKey === lastShapeKey) return;
    lastShapeKey = shapeKey;

    const {
      depth: DEPTH,
      shoulder: SHOULDER,
      inner_radius: INNER,
    } = layout.metrics;
    root.style.setProperty("--notch-depth", `${layout.metrics.depth}px`);
    root.style.setProperty("--notch-cell", `${layout.metrics.cell}px`);
    root.style.setProperty("--notch-inset", `${layout.metrics.inset}px`);
    root.style.setProperty("--rest-depth", `${layout.metrics.rest_depth}px`);
    root.style.setProperty("--rest-length", `${layout.metrics.rest_length}px`);
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
    reveal(open: boolean) {
      root.classList.toggle("is-folded", !open);
      cells.inert = !open;
      root.setAttribute("aria-expanded", String(open));
    },
    activity(states: ProviderActivity[]) {
      for (const [id, button] of buttons) {
        const state = states.find((s) => s.provider_id === id)?.state ?? "unknown";
        button.dataset.activity = state;
        const label = state === "running" ? "Working in local Codex session" :
          state === "recent" ? "Recent local activity (inferred)" : "";
        button.title = label;
        if (label) button.setAttribute("aria-description", label);
        else button.removeAttribute("aria-description");
      }
    },
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
      if (root.dataset.edge !== l.edge) root.dataset.edge = l.edge;
      positionNotch(root, l.notch, l);
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
export function positionNotch(el: HTMLElement, r: Rect, l: NotchLayout) {
  const width = `${r.width / l.scale}px`;
  const height = `${r.height / l.scale}px`;
  if (el.style.width !== width) el.style.width = width;
  if (el.style.height !== height) el.style.height = height;

  if (l.anchor_x === "right") {
    if (el.style.right !== "0px") el.style.right = "0px";
    if (el.style.left !== "") el.style.left = "";
  } else {
    if (el.style.left !== "0px") el.style.left = "0px";
    if (el.style.right !== "") el.style.right = "";
  }

  if (l.anchor_y === "bottom") {
    if (el.style.bottom !== "0px") el.style.bottom = "0px";
    if (el.style.top !== "") el.style.top = "";
  } else {
    if (el.style.top !== "0px") el.style.top = "0px";
    if (el.style.bottom !== "") el.style.bottom = "";
  }
}
export function position(el: HTMLElement, r: Rect, l: NotchLayout) {
  const left = `${(r.x - l.window.x) / l.scale}px`;
  const top = `${(r.y - l.window.y) / l.scale}px`;
  const width = `${r.width / l.scale}px`;
  const height = `${r.height / l.scale}px`;
  if (el.style.left !== left) el.style.left = left;
  if (el.style.top !== top) el.style.top = top;
  if (el.style.width !== width) el.style.width = width;
  if (el.style.height !== height) el.style.height = height;
}
