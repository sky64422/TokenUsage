import { headline } from "./notch-state";
import { PROVIDER_IDS } from "./types";
import type { ProviderId, ProviderSnapshot, ProviderActivity, NotchLayout, Rect } from "./types";
import { PROVIDER_CATALOG } from "./provider-catalog";
import { clampPct } from "./format";

export function mountNotch(
  root: HTMLElement,
  callbacks: {
    hover: (id: ProviderId) => void;
    pin: (id: ProviderId) => void;
    hoverUpdate?: () => void;
    clickUpdate?: () => void;
  },
) {
  root.setAttribute("aria-label", "AI usage. Drag to move; right-click for settings.");
  root.innerHTML = `<span class="notch-rest" aria-hidden="true"></span>
    <div class="notch-surface">
      <svg class="notch-shape" aria-hidden="true">
        <path id="notch-outline"/>
        <defs>
          <clipPath id="notch-finish-clip"><use href="#notch-outline"/></clipPath>
          <linearGradient id="notch-glass-rim" x1="0" y1="0" x2="1" y2="1">
            <stop stop-color="#eef6ff" stop-opacity=".9"/>
            <stop offset=".18" stop-color="#f9fcff" stop-opacity=".22"/>
            <stop offset=".36" stop-color="#212a33" stop-opacity=".55"/>
            <stop offset=".48" stop-color="#f4f8ff" stop-opacity=".88"/>
            <stop offset=".61" stop-color="#8393a2" stop-opacity=".28"/>
            <stop offset=".79" stop-color="#eff8ff" stop-opacity=".72"/>
            <stop offset="1" stop-color="#d2e3eb" stop-opacity=".5"/>
          </linearGradient>
          <linearGradient id="notch-liquid-rim" x1="0" y1="0" x2="1" y2="1">
            <stop stop-color="#ffffff" stop-opacity=".9"/>
            <stop offset=".18" stop-color="#74e8ef" stop-opacity=".62"/>
            <stop offset=".26" stop-color="#84b3fa" stop-opacity=".44"/>
            <stop offset=".36" stop-color="#0f2131" stop-opacity=".35"/>
            <stop offset=".52" stop-color="#ffffff" stop-opacity=".12"/>
            <stop offset=".70" stop-color="#eda3d2" stop-opacity=".52"/>
            <stop offset=".78" stop-color="#f5fbff" stop-opacity=".68"/>
            <stop offset="1" stop-color="#ffffff" stop-opacity=".5"/>
          </linearGradient>
          <linearGradient id="notch-star-gold" x1="0" y1="0" x2="1" y2="1">
            <stop stop-color="#fff2a8"/>
            <stop offset=".4" stop-color="#ffd043"/>
            <stop offset="1" stop-color="#fa8c16"/>
          </linearGradient>
        </defs>
        <use class="notch-liquid-volume" href="#notch-outline"/>
        <use class="notch-glass-rim" href="#notch-outline"/>
        <use class="notch-liquid-edge" href="#notch-outline"/>
      </svg>
      <div class="notch-finish" aria-hidden="true"></div>
      <div class="notch-cells"></div>
      <button type="button" class="notch-update-star" hidden aria-label="새 버전 업데이트">
        <svg class="notch-star-svg" viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
          <defs>
            <linearGradient id="notch-star-gold-twin" x1="0" y1="0" x2="1" y2="1">
              <stop stop-color="#fff4c2"/>
              <stop offset=".45" stop-color="#ffd043"/>
              <stop offset="1" stop-color="#fa8c16"/>
            </linearGradient>
          </defs>
          <!-- Main star -->
          <path d="M 9 4.5 C 9 9.1, 10.9 11, 15.5 11 C 10.9 11, 9 12.9, 9 17.5 C 9 12.9, 7.1 11, 2.5 11 C 7.1 11, 9 9.1, 9 4.5 Z" fill="url(#notch-star-gold-twin)"/>
          <circle cx="9" cy="11" r="1.3" fill="#ffffff" opacity="0.95"/>
          <!-- Companion smaller star -->
          <path d="M 18.5 1 C 18.5 3.5, 19.5 4.5, 22 4.5 C 19.5 4.5, 18.5 5.5, 18.5 8 C 18.5 5.5, 17.5 4.5, 15 4.5 C 17.5 4.5, 18.5 3.5, 18.5 1 Z" fill="url(#notch-star-gold-twin)"/>
          <circle cx="18.5" cy="4.5" r="0.8" fill="#ffffff" opacity="0.9"/>
        </svg>
      </button>
    </div>`;
  const cells = root.querySelector<HTMLElement>(".notch-cells")!;
  const updateStar = root.querySelector<HTMLButtonElement>(".notch-update-star")!;
  updateStar.addEventListener("click", (e) => {
    e.stopPropagation();
    callbacks.clickUpdate?.();
  });
  updateStar.addEventListener("pointerenter", () => {
    callbacks.hoverUpdate?.();
  });
  const buttons = new Map<ProviderId, HTMLButtonElement>();
  for (const id of PROVIDER_IDS) {
    const b = document.createElement("button");
    b.className = "notch-cell";
    b.dataset.id = id;
    b.innerHTML = `<span class="ring-wrap"><svg viewBox="0 0 44 44" aria-hidden="true">
      <circle class="ring-track" cx="22" cy="22" r="19"/>
      <circle class="ring-fill" cx="22" cy="22" r="19" pathLength="100" visibility="hidden"/>
      </svg><img src="${PROVIDER_CATALOG[id].mark}" alt="" draggable="false"/><span class="activity-orbit" aria-hidden="true"></span><span class="ring-status" hidden>!</span></span><span class="ring-meta"><span class="ring-pct">—</span></span>`;
    b.addEventListener("pointerenter", () => callbacks.hover(id));
    b.addEventListener("click", () => callbacks.pin(id));
    cells.append(b);
    buttons.set(id, b);
  }
  let lastShapeKey = "";
  function shape(layout: NotchLayout) {
    const vertical = layout.edge === "left" || layout.edge === "right";
    const length =
      (vertical ? layout.notch.height : layout.notch.width) / layout.scale;
    const shapeKey = `${layout.edge}:${length}:${layout.scale}:${layout.metrics.content_scale ?? 1}`;
    if (shapeKey === lastShapeKey) return;
    lastShapeKey = shapeKey;

    const {
      depth: DEPTH,
      shoulder: SHOULDER,
      inner_radius: INNER,
    } = layout.metrics;
    const currentScale = layout.metrics.content_scale ?? 1;
    root.style.setProperty("--notch-depth", `${layout.metrics.depth}px`);
    root.style.setProperty("--notch-scale", String(currentScale));
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
        button.removeAttribute("title");
        const label = state === "running" ? `${id}: working` : "";
        if (label) button.setAttribute("aria-description", label);
        else button.removeAttribute("aria-description");
      }
    },
    update(snaps: ProviderSnapshot[], enabled: ProviderId[]) {
      for (const [id, b] of buttons) {
        const hidden = !enabled.includes(id);
        if (b.hidden !== hidden) b.hidden = hidden;
        const s = snaps.find((v) => v.provider_id === id);
        if (!s) {
          const label = `${id}: waiting for usage`;
          if (b.getAttribute("aria-label") !== label) b.setAttribute("aria-label", label);
          continue;
        }
        const h = headline(s);
        const className = `notch-cell ${h.level}`;
        if (b.className !== className) b.className = className;
        const percent = b.querySelector(".ring-pct")!;
        if (percent.textContent !== h.text) percent.textContent = h.text;
        const used = clampPct(h.fill) ?? 0;
        const fill = b.querySelector<SVGElement>(".ring-fill")!;
        const dasharray = `${used} 100`;
        if (fill.style.strokeDasharray !== dasharray) fill.style.strokeDasharray = dasharray;
        const visibility = used > 0 ? "visible" : "hidden";
        if (fill.getAttribute("visibility") !== visibility) fill.setAttribute("visibility", visibility);
        const status = b.querySelector<HTMLElement>(".ring-status")!;
        if (status.hidden !== !h.degraded) status.hidden = !h.degraded;
        const title = h.degraded
          ? (s.message || s.status.replaceAll("_", " "))
          : "";
        if (status.title !== title) status.title = title;
        const label = `${s.display_name}: ${h.label}, ${h.text} used${h.degraded ? `, ${s.status.replaceAll("_", " ")}` : ""}${h.secondaryRisk ? `, other limits: ${h.secondaryRisk}` : ""}. Click to keep open.`;
        if (b.getAttribute("aria-label") !== label) b.setAttribute("aria-label", label);
      }
    },
    layout(l: NotchLayout) {
      if (root.dataset.edge !== l.edge) root.dataset.edge = l.edge;
      positionNotch(root, l.notch, l);
      shape(l);
    },
    select(id: ProviderId | null, pinned: boolean) {
      for (const [key, b] of buttons) {
        b.dataset.detailOpen = String(key === id);
        b.setAttribute("aria-pressed", String(pinned && key === id));
      }
    },
    setUpdateReady(ready: boolean, version?: string) {
      updateStar.hidden = !ready;
      if (ready && version) {
        updateStar.setAttribute("aria-label", `v${version} 업데이트`);
        updateStar.removeAttribute("title");
      }
    },
  };
}
export function positionNotch(el: HTMLElement, r: Rect, l: NotchLayout) {
  const width = `${r.width / l.scale}px`;
  const height = `${r.height / l.scale}px`;
  if (el.style.width !== width) el.style.width = width;
  if (el.style.height !== height) el.style.height = height;

  if (l.anchor_x === "right") {
    const right = `${((l.window.x + l.window.width) - (r.x + r.width)) / l.scale}px`;
    if (el.style.right !== right) el.style.right = right;
    if (el.style.left !== "") el.style.left = "";
  } else {
    const left = `${(r.x - l.window.x) / l.scale}px`;
    if (el.style.left !== left) el.style.left = left;
    if (el.style.right !== "") el.style.right = "";
  }

  if (l.anchor_y === "bottom") {
    const bottom = `${((l.window.y + l.window.height) - (r.y + r.height)) / l.scale}px`;
    if (el.style.bottom !== bottom) el.style.bottom = bottom;
    if (el.style.top !== "") el.style.top = "";
  } else {
    const top = `${(r.y - l.window.y) / l.scale}px`;
    if (el.style.top !== top) el.style.top = top;
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
