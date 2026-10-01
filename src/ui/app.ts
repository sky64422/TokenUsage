import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { renderHeader } from "./header";
import { applyPanelOpacity } from "./opacity";
import { mountProviders } from "./providers";
import { mountSettingsPanel } from "./settings-panel";
import { mountNotch, position } from "./notch";
import {
  createCloseDelay,
  initialNotchState,
  reduceNotchState,
  nudgeOffset,
} from "./notch-state";
import type { NotchEvent } from "./notch-state";
import { PROVIDER_IDS } from "./types";
import type {
  PersistedState,
  ProviderSnapshot,
  NotchLayout,
  NotchPlacement,
  MonitorArea,
  DiagnosticsSnapshot,
  NotchEdge,
} from "./types";

const DRAG_THRESHOLD = 5; // CSS pixels along the attached edge.
const SURFACE_PADDING = 34; // 16px padding and 1px border on both sides.
export async function mountApp(root: HTMLElement): Promise<void> {
  root.innerHTML = `<div class="notch-shell"><nav class="notch" aria-label="AI usage"></nav><section class="notch-detail" aria-label="Usage detail" hidden><div class="detail-body"><div class="detail-heading"><h2>Usage</h2><button class="icon-btn detail-close" aria-label="Close detail">×</button></div><div class="detail-quota"><div class="quota-root"></div><p class="detail-state"></p><small class="detail-hint">Click a ring to keep open</small></div><div class="detail-settings" hidden><div class="header-root"></div><div class="placement-controls"><label for="edge">Edge</label><select id="edge"><option value="right">Right</option><option value="left">Left</option><option value="top">Top</option><option value="bottom">Bottom</option></select><label for="monitor">Display</label><select id="monitor"></select><span></span><button id="recentre">Recentre</button></div><p class="placement-note"></p><div class="settings-root"></div></div><p class="notch-error" role="status" hidden></p></div></section></div>`;
  const shell = root.querySelector<HTMLElement>(".notch-shell")!;
  const rail = root.querySelector<HTMLElement>(".notch")!;
  const detail = root.querySelector<HTMLElement>(".notch-detail")!;
  const body = root.querySelector<HTMLElement>(".detail-body")!;
  const quota = root.querySelector<HTMLElement>(".detail-quota")!;
  const settingsArea = root.querySelector<HTMLElement>(".detail-settings")!;
  const errorEl = root.querySelector<HTMLElement>(".notch-error")!;
  const heading = root.querySelector("h2")!;
  let persisted = await invoke<PersistedState>("get_state");
  let placement = persisted.settings.notch;
  let snaps: ProviderSnapshot[] = [];
  let interaction = initialNotchState();
  let layout: NotchLayout | undefined;
  let revision = Date.now();
  let lastSurface = "";
  let surfaceChain: Promise<void> = Promise.resolve();
  let placementChain: Promise<void> = Promise.resolve();
  let lastQuota = "";
  let dragging = false;
  let pressedPointer: number | null = null;
  let suppressClick = false;
  let dragStart = 0,
    dragOffset = 0,
    dragSpan = 1;
  let pendingDrag: NotchPlacement | null = null;
  let previewRunning = false;
  const win = getCurrentWindow();
  const enabled = () =>
    PROVIDER_IDS.filter((id) => persisted.settings[id].enabled);
  const fail = (e: unknown) => {
    console.error(e);
    errorEl.textContent = String(e);
    errorEl.hidden = false;
  };
  const closeDelay = createCloseDelay(() => {
    if (!document.activeElement?.matches(":focus-visible"))
      dispatch({ type: "leave" });
  });
  const notch = mountNotch(rail, {
    hover: (id) => {
      if (dragging) return;
      closeDelay.cancel();
      dispatch({ type: "hover", id });
    },
    pin: (id) => dispatch({ type: "pin", id }),
  });
  const providers = mountProviders(
    root.querySelector<HTMLElement>(".quota-root")!,
  );
  providers.setTints({
    claude: persisted.settings.claude.card_tint,
    codex: persisted.settings.codex.card_tint,
    grok: persisted.settings.grok.card_tint,
  });
  applyPanelOpacity(shell, persisted.settings.opacity);
  const settings = mountSettingsPanel(
    root.querySelector<HTMLElement>(".settings-root")!,
    persisted.settings,
    {
      onAutostart: (v) => {
        void invoke("set_autostart", { enabled: v }).catch(fail);
      },
      onProviderEnabled: async (provider, isEnabled) => {
        try {
          snaps = await invoke("set_provider_enabled", {
            provider,
            enabled: isEnabled,
          });
          persisted = await invoke("get_state");
          refreshView();
        } catch (e) {
          fail(e);
          throw e;
        }
      },
      onDiagnostics: async () => {
        const diag = await invoke<DiagnosticsSnapshot>("get_diagnostics");
        await navigator.clipboard.writeText(diag.lines.join("\n"));
      },
      onQuit: () => {
        void invoke("quit_app").catch(fail);
      },
    },
    await getVersion(),
  );
  settings.show();
  const header = root.querySelector<HTMLElement>(".header-root")!;
  renderHeader(header, {
    onSettings: () => dispatch({ type: "escape" }),
    onHide: () => {
      dispatch({ type: "reset" });
      void invoke("hide_widget").catch(fail);
    },
    opacity: persisted.settings.opacity,
    onOpacityChange: (opacity) => {
      applyPanelOpacity(shell, opacity);
      void invoke("set_opacity", { opacity }).catch(fail);
    },
  });
  header
    .querySelector("[data-tauri-drag-region]")
    ?.removeAttribute("data-tauri-drag-region");
  const edgeSelect = root.querySelector<HTMLSelectElement>("#edge")!;
  const monitorSelect = root.querySelector<HTMLSelectElement>("#monitor")!;
  let areas: MonitorArea[] = await invoke("get_notch_monitors");
  function fillMonitors() {
    monitorSelect.replaceChildren(
      ...areas.map((m, i) => {
        const o = new Option(
          `Display ${i + 1} · ${m.bounds.width} × ${m.bounds.height}`,
          m.name,
        );
        return o;
      }),
    );
    monitorSelect.value =
      layout?.monitor ?? placement.monitor_hint ?? areas[0]?.name ?? "";
  }
  fillMonitors();
  edgeSelect.value = placement.edge;
  async function savePlacement(next: NotchPlacement) {
    placement = next;
    placementChain = placementChain
      .then(async () => {
        paintLayout(
          await invoke<NotchLayout>("set_notch_placement", { placement: next }),
        );
        errorEl.hidden = true;
      })
      .catch(async (e) => {
        fail(e);
        try {
          const restored = await invoke<PersistedState>("get_state");
          placement = restored.settings.notch;
          edgeSelect.value = placement.edge;
          fillMonitors();
        } catch (syncError) {
          fail(syncError);
        }
      });
    await placementChain;
  }
  edgeSelect.addEventListener("change", () => {
    void savePlacement({ ...placement, edge: edgeSelect.value as NotchEdge });
  });
  monitorSelect.addEventListener("change", () => {
    void savePlacement({ ...placement, monitor_hint: monitorSelect.value });
  });
  root.querySelector("#recentre")!.addEventListener("click", () => {
    void savePlacement({ ...placement, offset: 0.5 });
  });

  function paintLayout(next: NotchLayout) {
    layout = next;
    notch.layout(next);
    shell.dataset.edge = next.edge;
    shell.style.setProperty(
      "--detail-radius",
      `${next.metrics.detail_radius}px`,
    );
    if (next.detail) position(detail, next.detail, next);
    root.querySelector(".placement-note")!.textContent =
      next.edge !== placement.edge
        ? `Taskbar occupies ${placement.edge}; using ${next.edge}.`
        : `Attached to ${next.edge} edge`;
    monitorSelect.value = next.monitor;
    if (!detail.hidden) requestSurface();
  }
  function requestSurface() {
    const expanded = interaction.settings || interaction.provider !== null;
    const height = Math.ceil(
      body.getBoundingClientRect().height + SURFACE_PADDING,
    );
    const key = JSON.stringify([expanded, height, enabled()]);
    if (lastSurface === key) return;
    lastSurface = key;
    const requestRevision = ++revision;
    surfaceChain = surfaceChain
      .then(async () => {
        const next = await invoke<NotchLayout>("set_notch_surface", {
          revision: requestRevision,
          expanded,
          height,
        });
        if (requestRevision === revision) paintLayout(next);
      })
      .catch((e) => {
        lastSurface = "";
        fail(e);
      });
  }
  function render() {
    notch.select(interaction.provider, interaction.pinned);
    detail.hidden = !interaction.settings && !interaction.provider;
    settingsArea.hidden = !interaction.settings;
    quota.hidden = interaction.settings;
    heading.textContent = interaction.settings
      ? "Settings"
      : (snaps.find((s) => s.provider_id === interaction.provider)
          ?.display_name ?? "Usage");
    if (interaction.provider && !interaction.settings) {
      const snap = snaps.find((s) => s.provider_id === interaction.provider);
      const key = JSON.stringify(snap);
      if (key !== lastQuota) {
        providers.setSnapshots(snap ? [snap] : []);
        lastQuota = key;
      }
      const status = root.querySelector<HTMLElement>(".detail-state")!;
      status.textContent =
        snap && snap.status !== "ok"
          ? `${snap.status.replaceAll("_", " ")}${snap.message ? ` · ${snap.message}` : ""}`
          : "";
      status.hidden = !status.textContent;
      root.querySelector(".detail-hint")!.textContent = interaction.pinned
        ? "Kept open · click ring or press Esc to release"
        : "Click ring to keep open";
    }
    requestSurface();
  }
  function dispatch(event: NotchEvent) {
    interaction = reduceNotchState(interaction, event);
    render();
  }
  function refreshView() {
    const ids = enabled();
    interaction = reduceNotchState(interaction, { type: "providers", ids });
    notch.update(snaps, ids);
    if (layout) notch.layout(layout);
    render();
  }
  for (const el of [rail, detail]) {
    el.addEventListener("pointerenter", closeDelay.cancel);
    el.addEventListener("pointerleave", () => {
      if (!dragging) closeDelay.schedule();
    });
    el.addEventListener("focusin", closeDelay.cancel);
    el.addEventListener("focusout", closeDelay.schedule);
  }
  root.querySelector(".detail-close")!.addEventListener("click", () => {
    dispatch({ type: "reset" });
  });
  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    const id = interaction.provider;
    dispatch({ type: "escape" });
    // Focus recovery must not reopen a detail via the focus handler.
    notch.focus(id);
  });
  async function previewDrag(next: NotchPlacement) {
    pendingDrag = next;
    if (previewRunning) return;
    previewRunning = true;
    try {
      while (pendingDrag && dragging) {
        const placement = pendingDrag;
        pendingDrag = null;
        paintLayout(
          await invoke<NotchLayout>("preview_notch_placement", { placement }),
        );
      }
    } catch (e) {
      fail(e);
    } finally {
      previewRunning = false;
    }
  }
  rail.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || !layout || pressedPointer !== null) return;
    pressedPointer = e.pointerId;
    suppressClick = false;
    closeDelay.cancel();
    (e.target as Element).setPointerCapture(e.pointerId);
    dragStart =
      layout.edge === "left" || layout.edge === "right" ? e.screenY : e.screenX;
    dragOffset = placement.offset;
    const m = areas.find((m) => m.name === layout!.monitor)!;
    dragSpan =
      (layout.edge === "left" || layout.edge === "right"
        ? m.work.height - layout.notch.height
        : m.work.width - layout.notch.width) / layout.scale;
  });
  const dragPlacement = (e: PointerEvent): NotchPlacement => {
    const end =
      layout!.edge === "left" || layout!.edge === "right"
        ? e.screenY
        : e.screenX;
    return {
      ...placement,
      offset: Math.max(
        0,
        Math.min(1, dragOffset + (end - dragStart) / Math.max(1, dragSpan)),
      ),
    };
  };
  rail.addEventListener("pointermove", (e) => {
    if (e.pointerId !== pressedPointer || !layout) return;
    const end = layout.edge === "left" || layout.edge === "right" ? e.screenY : e.screenX;
    if (!dragging && Math.abs(end - dragStart) < DRAG_THRESHOLD) return;
    dragging = true;
    suppressClick = true;
    rail.classList.add("is-dragging");
    rail.setPointerCapture(e.pointerId);
    void previewDrag(dragPlacement(e));
  });
  rail.addEventListener("pointerup", (e) => {
    if (e.pointerId !== pressedPointer || !layout) return;
    pressedPointer = null;
    const moved = dragging;
    dragging = false;
    rail.classList.remove("is-dragging");
    pendingDrag = null;
    if (moved) void savePlacement(dragPlacement(e));
  });
  rail.addEventListener("pointercancel", (e) => {
    if (e.pointerId !== pressedPointer) return;
    pressedPointer = null;
    dragging = false;
    rail.classList.remove("is-dragging");
    pendingDrag = null;
    void invoke<NotchLayout>("preview_notch_placement", { placement: null })
      .then(paintLayout)
      .catch(fail);
  });
  rail.addEventListener("click", (e) => {
    if (suppressClick && e.detail !== 0) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
    suppressClick = false;
  }, true);
  function openSettings() {
    closeDelay.cancel();
    if (!interaction.settings) dispatch({ type: "settings" });
    void invoke<MonitorArea[]>("get_notch_monitors")
      .then((next) => {
        areas = next;
        fillMonitors();
      })
      .catch(fail);
  }
  rail.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    openSettings();
  });
  rail.addEventListener("keydown", (e) => {
    if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      e.preventDefault();
      openSettings();
      return;
    }
    if (!layout) return;
    const offset = nudgeOffset(
      layout.edge,
      placement.offset,
      e.key,
      e.shiftKey,
    );
    if (offset !== placement.offset) {
      e.preventDefault();
      void savePlacement({ ...placement, offset });
    }
  });
  await listen<NotchLayout>("notch-layout", (ev) => paintLayout(ev.payload));
  await listen<ProviderSnapshot[]>("snapshots-updated", (ev) => {
    snaps = ev.payload;
    refreshView();
  });
  await win.onFocusChanged(({ payload }) => {
    if (!payload) {
      (document.activeElement as HTMLElement | null)?.blur();
      closeDelay.schedule();
    }
  });
  snaps = await invoke("get_snapshots");
  refreshView();
  await surfaceChain;
  await win.show();
  const observer = new ResizeObserver(() => {
    if (!detail.hidden) requestSurface();
  });
  observer.observe(body);
  window.addEventListener("beforeunload", () => observer.disconnect());
  window.addEventListener("beforeunload", closeDelay.cancel);
}
