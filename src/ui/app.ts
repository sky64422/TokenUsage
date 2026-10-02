import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { applyPanelOpacity } from "./opacity";
import { mountProviders } from "./providers";
import { mountSettingsPanel } from "./settings-panel";
import { mountNotch, position } from "./notch";
import {
  createCloseDelay,
  initialNotchState,
  reduceNotchState,
} from "./notch-state";
import type { NotchEvent } from "./notch-state";
import { PROVIDER_IDS } from "./types";
import type {
  PersistedState,
  ProviderSnapshot,
  ProviderActivity,
  NotchLayout,
  NotchPlacement,
  DiagnosticsSnapshot,
} from "./types";

const SURFACE_PADDING = 36; // 16px padding and 1px border on both sides + 2px headroom.
export async function mountApp(root: HTMLElement): Promise<void> {
  root.innerHTML = `<div class="notch-shell"><nav class="notch" aria-label="AI usage"></nav><section class="notch-detail" aria-label="Usage detail" hidden><div class="detail-body"><div class="detail-quota"><div class="quota-root"></div><p class="detail-state"></p></div><div class="detail-settings" hidden><div class="detail-heading"><h2>Settings</h2><button class="icon-btn detail-close" aria-label="Close settings">×</button></div><div class="settings-root"></div></div><p class="notch-error" role="status" hidden></p></div></section></div>`;
  const shell = root.querySelector<HTMLElement>(".notch-shell")!;
  const rail = root.querySelector<HTMLElement>(".notch")!;
  const detail = root.querySelector<HTMLElement>(".notch-detail")!;
  const body = root.querySelector<HTMLElement>(".detail-body")!;
  const quota = root.querySelector<HTMLElement>(".detail-quota")!;
  const settingsArea = root.querySelector<HTMLElement>(".detail-settings")!;
  const errorEl = root.querySelector<HTMLElement>(".notch-error")!;
  let persisted = await invoke<PersistedState>("get_state");
  let snaps: ProviderSnapshot[] = [];
  let interaction = initialNotchState();
  let layout: NotchLayout | undefined;
  let revision = Date.now();
  let lastSurface = "";
  let surfaceChain: Promise<void> = Promise.resolve();
  let lastQuota = "";
  let dragging = false;
  let pressedPointer: number | null = null;
  let capturedPointer: { element: Element; id: number } | null = null;
  let suppressClick = false;
  let dragId: number | null = null;
  let nextDragId = Date.now();
  let dragChain: Promise<void> = Promise.resolve();
  let beforeDrag = initialNotchState();
  let localStart = { x: 0, y: 0 };
  const win = getCurrentWindow();
  const enabled = () =>
    PROVIDER_IDS.filter((id) => persisted.settings[id].enabled);
  const fail = (e: unknown) => {
    console.error(e);
    errorEl.textContent = String(e);
    errorEl.hidden = false;
  };
  const closeDelay = createCloseDelay(() => {
    if (dragId === null && !document.activeElement?.matches(":focus-visible"))
      dispatch({ type: "leave" });
  });
  const notch = mountNotch(rail, {
    hover: (id) => {
      if (dragging || !persisted.settings.hover_detail) return;
      closeDelay.cancel();
      dispatch({ type: "hover", id });
    },
    pin: (id) => {
      if (interaction.provider === id && interaction.pinned) {
        dispatch({ type: "reset" });
      } else {
        dispatch({ type: "pin", id });
      }
    },
  });
  const providers = mountProviders(
    root.querySelector<HTMLElement>(".quota-root")!,
  );
  providers.setTints({
    claude: persisted.settings.claude.card_tint,
    codex: persisted.settings.codex.card_tint,
    grok: persisted.settings.grok.card_tint,
    agy: persisted.settings.agy.card_tint,
  });
  applyPanelOpacity(shell, persisted.settings.opacity);
  const settings = mountSettingsPanel(
    root.querySelector<HTMLElement>(".settings-root")!,
    persisted.settings,
    {
      onAutostart: (v) => {
        void invoke("set_autostart", { enabled: v }).catch(fail);
      },
      onHoverDetail: (v) => {
        persisted.settings.hover_detail = v;
        void invoke("set_hover_detail", { enabled: v }).catch(fail);
      },
      onOpacityChange: (opacity) => {
        applyPanelOpacity(shell, opacity);
        void invoke("set_opacity", { opacity }).catch(fail);
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

  let lastLayoutKey = "";
  function paintLayout(next: NotchLayout) {
    const key = `${next.edge}:${next.scale}:${next.notch.x},${next.notch.y},${next.notch.width},${next.notch.height}:${next.window.x},${next.window.y},${next.window.width},${next.window.height}:${next.detail ? `${next.detail.x},${next.detail.y},${next.detail.width},${next.detail.height}` : "none"}`;
    if (key === lastLayoutKey) return;
    lastLayoutKey = key;
    layout = next;
    notch.layout(next);
    if (shell.dataset.edge !== next.edge) shell.dataset.edge = next.edge;
    shell.style.setProperty(
      "--detail-radius",
      `${next.metrics.detail_radius}px`,
    );
    if (next.detail) position(detail, next.detail, next);
  }
  function requestSurface() {
    const expanded = !dragging && (interaction.settings || interaction.provider !== null);
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
    detail.hidden = dragging || (!interaction.settings && !interaction.provider);
    settingsArea.hidden = !interaction.settings;
    quota.hidden = interaction.settings;
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
    if (dragId !== null) { finishDrag(true); return; }
    const id = interaction.provider;
    dispatch({ type: "escape" });
    // Focus recovery must not reopen a detail via the focus handler.
    notch.focus(id);
  });
  function releaseDragCapture() {
    const captured = capturedPointer;
    capturedPointer = null;
    if (captured?.element.hasPointerCapture(captured.id)) captured.element.releasePointerCapture(captured.id);
  }
  function finishDrag(cancel: boolean) {
    const id = dragId;
    if (id === null) return;
    dragChain = dragChain.then(() => invoke<void>("finish_notch_drag", { id, cancel })).catch(fail);
  }
  await listen<{id: number; active: boolean; finished: boolean; placement: NotchPlacement; error: string | null}>("notch-drag", ({payload: event}) => {
    if (event.id !== dragId) return;
    if (event.active) {
      suppressClick = true;
      closeDelay.cancel();
    }
    dragging = event.active && !event.finished;
    rail.classList.toggle("is-dragging", dragging);
    if (event.finished) {
      dragId = null;
      pressedPointer = null;
      releaseDragCapture();
      if (event.active) {
        interaction = beforeDrag.settings || beforeDrag.pinned ? beforeDrag : initialNotchState();
      }
      if (event.error) { fail(event.error); interaction = { ...interaction, settings: true }; }
    }
    render();
  });
  rail.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || !layout || dragId !== null) return;
    const id = ++nextDragId;
    dragId = id;
    pressedPointer = e.pointerId;
    suppressClick = false;
    beforeDrag = { ...interaction };
    localStart = { x: e.screenX, y: e.screenY };
    closeDelay.cancel();
    capturedPointer = { element: e.target as Element, id: e.pointerId };
    capturedPointer.element.setPointerCapture(e.pointerId);
    dragChain = dragChain.then(() => invoke<void>("begin_notch_drag", { id })).catch((error) => {
      if (dragId === id) { dragId = null; pressedPointer = null; releaseDragCapture(); }
      fail(error);
    });
  });
  rail.addEventListener("pointermove", (e) => {
    if (e.pointerId !== pressedPointer || !layout) return;
    // Only suppress the browser's click here; native physical coordinates own placement.
    if (Math.hypot(e.screenX - localStart.x, e.screenY - localStart.y) >= layout.metrics.drag_threshold) {
      suppressClick = true;
    }
  });
  rail.addEventListener("pointerup", (e) => {
    if (e.pointerId !== pressedPointer) return;
    pressedPointer = null;
    finishDrag(false);
  });
  rail.addEventListener("pointercancel", (e) => {
    if (e.pointerId === pressedPointer) finishDrag(true);
  });
  rail.addEventListener("lostpointercapture", (e) => {
    if (e.pointerId === pressedPointer) finishDrag(e.buttons !== 0);
  });
  rail.addEventListener("click", (e) => {
    if (suppressClick && e.detail !== 0) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
    suppressClick = false;
  }, true);
  function openSettings() {
    if (dragId !== null) return;
    closeDelay.cancel();
    if (!interaction.settings) dispatch({ type: "settings" });
  }
  rail.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    openSettings();
  });
  rail.addEventListener("keydown", (e) => {
    if (e.key === "Tab" && rail.classList.contains("is-folded")) {
      e.preventDefault();
      void invoke<boolean>("set_notch_focus", { focused: true }).then(notch.reveal).catch(fail);
      return;
    }
    if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      e.preventDefault();
      openSettings();
      return;
    }
  });
  await listen<NotchLayout>("notch-layout", (ev) => paintLayout(ev.payload));
  await listen<boolean>("notch-reveal", (ev) => notch.reveal(ev.payload));
  await listen<ProviderActivity[]>("provider-activity", (ev) => notch.activity(ev.payload));
  rail.addEventListener("focusin", () => {
    if (document.activeElement?.matches(":focus-visible"))
      void invoke<boolean>("set_notch_focus", { focused: true }).then(notch.reveal).catch(fail);
  });
  rail.addEventListener("focusout", () => {
    if (!rail.contains(document.activeElement))
      void invoke<boolean>("set_notch_focus", { focused: false }).then(notch.reveal).catch(fail);
  });
  await listen<ProviderSnapshot[]>("snapshots-updated", (ev) => {
    snaps = ev.payload;
    refreshView();
  });
  await win.onFocusChanged(({ payload }) => {
    if (!payload && dragId === null) {
      (document.activeElement as HTMLElement | null)?.blur();
      closeDelay.schedule();
    }
  });
  snaps = await invoke("get_snapshots");
  notch.reveal(await invoke<boolean>("get_notch_reveal"));
  notch.activity(await invoke<ProviderActivity[]>("get_provider_activity"));
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
