import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { applyPanelOpacity } from "./opacity";
import { formatProviderStatus } from "./format";
import { mountProviders } from "./providers";
import { mountSettingsPanel } from "./settings-panel";
import { mountNotch, position } from "./notch";
import { createNotchSurface } from "./notch-surface";
import { mountNotchDrag } from "./notch-drag";
import type { DragNotice } from "./notch-drag";
import {
  createCloseDelay,
  initialNotchState,
  reduceNotchState,
} from "./notch-state";
import type { NotchEvent } from "./notch-state";
import { PROVIDER_IDS } from "./types";
import type {
  AppSettings,
  PersistedState,
  ProviderSnapshot,
  ProviderActivity,
  NotchLayout,
  DiagnosticsSnapshot,
} from "./types";

const SURFACE_PADDING = 36; // 16px padding and 1px border on both sides + 2px headroom.
export async function mountApp(root: HTMLElement): Promise<void> {
  root.innerHTML = `<div class="notch-shell"><nav class="notch" aria-label="AI usage"></nav><section class="notch-detail" aria-label="Usage detail" hidden><div class="detail-body"><div class="detail-quota"><div class="quota-root"></div><p class="detail-state"></p></div><div class="detail-settings" hidden><div class="settings-root"></div></div><p class="notch-error" role="status" hidden></p></div></section></div>`;
  const shell = root.querySelector<HTMLElement>(".notch-shell")!;
  const rail = root.querySelector<HTMLElement>(".notch")!;
  const detail = root.querySelector<HTMLElement>(".notch-detail")!;
  const body = root.querySelector<HTMLElement>(".detail-body")!;
  const quota = root.querySelector<HTMLElement>(".detail-quota")!;
  const settingsArea = root.querySelector<HTMLElement>(".detail-settings")!;
  const errorEl = root.querySelector<HTMLElement>(".notch-error")!;
  let persisted = await invoke<PersistedState>("get_state");
  let snaps: ProviderSnapshot[] = [];
  let latestActivities: ProviderActivity[] = [];
  let interaction = initialNotchState();
  let layout: NotchLayout | undefined;
  let lastQuota = "";
  let beforeDrag = initialNotchState();
  const win = getCurrentWindow();
  const enabled = () =>
    PROVIDER_IDS.filter((id) => persisted.settings[id].enabled);
  const fail = (e: unknown) => {
    console.error(e);
    errorEl.textContent = String(e);
    errorEl.hidden = false;
  };
  const events = new AbortController();
  const eventOptions = { signal: events.signal };
  const unlisteners: Array<() => void> = [];
  const surface = createNotchSurface({
    send: request => invoke<NotchLayout>("set_notch_surface", { ...request }),
    paint: paintLayout,
    fail,
  });
  const drag = mountNotchDrag(rail, () => layout, {
    begin: id => invoke<void>("begin_notch_drag", { id }),
    finish: (id, cancel) => invoke<void>("finish_notch_drag", { id, cancel }),
    onStart: () => { beforeDrag = { ...interaction }; closeDelay.cancel(); },
    onNotice: event => {
      if (event.active) closeDelay.cancel();
      if (event.finished) {
        if (event.active) {
          interaction = beforeDrag.settings || beforeDrag.pinned ? beforeDrag : initialNotchState();
        }
        if (event.error) { fail(event.error); interaction = { ...interaction, settings: true }; }
      }
      render();
    },
    fail,
  });
  const closeDelay = createCloseDelay(() => {
    if (drag.sessionId === null && !document.activeElement?.matches(":focus-visible"))
      dispatch({ type: "leave" });
  });
  const notch = mountNotch(rail, {
    hover: (id) => {
      if (drag.dragging || !persisted.settings.hover_detail) return;
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
  function applyAppearanceClasses(st: AppSettings) {
    const showAnim = st.show_orbit !== false || st.show_icon_glow !== false;
    rail.classList.toggle("hide-orbit", !showAnim);
    rail.classList.toggle("hide-icon-glow", !showAnim);
  }
  applyAppearanceClasses(persisted.settings);

  applyPanelOpacity(shell, persisted.settings.opacity);
  const settings = mountSettingsPanel(
    root.querySelector<HTMLElement>(".settings-root")!,
    persisted.settings,
    {
      onAutostart: async (v) => {
        await invoke("set_autostart", { enabled: v });
        persisted.settings.autostart = v;
      },
      onHoverDetail: async (v) => {
        await invoke("set_hover_detail", { enabled: v });
        persisted.settings.hover_detail = v;
      },
      onAlwaysShowNotch: async (v) => {
        await invoke("set_always_show_notch", { enabled: v });
        persisted.settings.always_show_notch = v;
      },
      onShowAnimation: async (v) => {
        await invoke("set_show_animation", { enabled: v });
        persisted.settings.show_orbit = v;
        persisted.settings.show_icon_glow = v;
        applyAppearanceClasses(persisted.settings);
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
      onClose: () => {
        dispatch({ type: "reset" });
      },
      onDiagnostics: async () => {
        const diag = await invoke<DiagnosticsSnapshot>("get_diagnostics");
        await navigator.clipboard.writeText(diag.lines.join("\n"));
      },
      onQuit: () => {
        void invoke("quit_app").catch(fail);
      },
      onTabChange: () => {
        if (interaction.settings) requestSurface();
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
    const expanded = !drag.dragging && (interaction.settings || interaction.provider !== null);
    const height = interaction.settings
      ? settings.getContentHeight()
      : Math.ceil(Math.max(body.offsetHeight, body.scrollHeight) + SURFACE_PADDING);
    let target: number | null = null;
    if (interaction.provider && !interaction.settings) {
      const cell = root.querySelector<HTMLElement>(
        `.notch-cell[data-id="${interaction.provider}"]`,
      );
      if (cell && cell.offsetHeight > 0) {
        const vertical = layout ? layout.edge === "left" || layout.edge === "right" : true;
        target = vertical
          ? cell.offsetTop + cell.offsetHeight / 2
          : cell.offsetLeft + cell.offsetWidth / 2;
      } else {
        const ids = enabled();
        const index = ids.indexOf(interaction.provider);
        if (index >= 0) {
          const cellSize = layout?.metrics.cell ?? 70;
          const inset = layout?.metrics.inset ?? 6;
          target = inset + index * cellSize + cellSize / 2;
        }
      }
    }
    surface.request({ expanded, height, target, providers: enabled() });
  }

  function render() {
    notch.select(interaction.settings || drag.dragging ? null : interaction.provider, interaction.pinned);
    detail.hidden = drag.dragging || (!interaction.settings && !interaction.provider);
    settingsArea.hidden = !interaction.settings;
    detail.classList.toggle("is-settings", interaction.settings);
    quota.hidden = interaction.settings;
    if (interaction.provider && !interaction.settings) {
      const snap = snaps.find((s) => s.provider_id === interaction.provider);
      const act = latestActivities.find((a) => a.provider_id === interaction.provider)?.state;
      const key = `${JSON.stringify(snap)}:${act}`;
      if (key !== lastQuota) {
        providers.setSnapshots(snap ? [snap] : []);
        lastQuota = key;
      }
      const status = root.querySelector<HTMLElement>(".detail-state")!;
      status.title = snap?.message ?? "";
      if (snap && snap.status !== "ok") {
        status.textContent = formatProviderStatus(snap.status, snap.message);
        // Empty rows already carry the summary; don't repeat it beneath the card.
        status.hidden = snap.windows.length === 0;
        status.classList.remove("is-running");
      } else if (act === "running") {
        status.innerHTML = `<span class="activity-pulse-dot" aria-hidden="true"></span>Working`;
        status.hidden = false;
        status.classList.add("is-running");
      } else {
        status.textContent = "";
        status.hidden = true;
        status.classList.remove("is-running");
      }
    }
    requestSurface();
  }
  function dispatch(event: NotchEvent) {
    interaction = reduceNotchState(interaction, event);
    render();
  }
  function refreshView() {
    applyAppearanceClasses(persisted.settings);
    const ids = enabled();
    interaction = reduceNotchState(interaction, { type: "providers", ids });
    notch.update(snaps, ids);
    if (layout) notch.layout(layout);
    render();
  }
  for (const el of [rail, detail]) {
    el.addEventListener("pointerenter", closeDelay.cancel, eventOptions);
    el.addEventListener("pointerleave", () => {
      if (!drag.dragging) closeDelay.schedule();
    }, eventOptions);
    el.addEventListener("focusin", closeDelay.cancel, eventOptions);
    el.addEventListener("focusout", closeDelay.schedule, eventOptions);
  }
  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (drag.sessionId !== null) { drag.finish(true); return; }
    const id = interaction.provider;
    dispatch({ type: "escape" });
    // Focus recovery must not reopen a detail via the focus handler.
    notch.focus(id);
  }, eventOptions);
  unlisteners.push(await listen<DragNotice>("notch-drag", ({ payload }) => drag.notice(payload)));
  function openSettings() {
    if (drag.sessionId !== null) return;
    closeDelay.cancel();
    if (interaction.settings) {
      dispatch({ type: "reset" });
    } else {
      dispatch({ type: "settings" });
    }
  }
  rail.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    openSettings();
  }, eventOptions);
  detail.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  }, eventOptions);
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
  }, eventOptions);
  unlisteners.push(await listen<NotchLayout>("notch-layout", (ev) => paintLayout(ev.payload)));
  unlisteners.push(await listen<boolean>("notch-reveal", (ev) => notch.reveal(ev.payload)));
  function updateActivities(acts: ProviderActivity[]) {
    latestActivities = acts;
    notch.activity(acts);
    providers.setActivities(acts);
    if (interaction.provider) render();
  }
  unlisteners.push(await listen<ProviderActivity[]>("provider-activity", (ev) => updateActivities(ev.payload)));
  rail.addEventListener("focusin", () => {
    if (document.activeElement?.matches(":focus-visible"))
      void invoke<boolean>("set_notch_focus", { focused: true }).then(notch.reveal).catch(fail);
  }, eventOptions);
  rail.addEventListener("focusout", () => {
    if (!rail.contains(document.activeElement))
      void invoke<boolean>("set_notch_focus", { focused: false }).then(notch.reveal).catch(fail);
  }, eventOptions);
  unlisteners.push(await listen<ProviderSnapshot[]>("snapshots-updated", (ev) => {
    snaps = ev.payload;
    refreshView();
  }));
  unlisteners.push(await win.onFocusChanged(({ payload }) => {
    if (!payload && drag.sessionId === null) {
      (document.activeElement as HTMLElement | null)?.blur();
      closeDelay.schedule();
    }
  }));
  snaps = await invoke("get_snapshots");
  notch.reveal(await invoke<boolean>("get_notch_reveal"));
  updateActivities(await invoke<ProviderActivity[]>("get_provider_activity"));
  refreshView();
  await surface.settled();
  await win.show();
  const observer = new ResizeObserver(() => {
    if (!detail.hidden && !interaction.settings) requestSurface();
  });
  observer.observe(body);
  window.addEventListener("beforeunload", () => {
    observer.disconnect();
    closeDelay.cancel();
    events.abort();
    surface.destroy();
    drag.destroy();
    settings.destroy();
    for (const unlisten of unlisteners) unlisten();
  }, { once: true });
}
