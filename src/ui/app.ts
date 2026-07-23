import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { renderHeader, setSettingsButtonActive } from "./header";
import { mountProviders } from "./providers";
import {
  applyPanelOpacity,
  applyThemeToDocument,
  mountSettingsPanel,
} from "./settings-panel";
import type {
  DiagnosticsSnapshot,
  PersistedState,
  PlanLimits,
  ProviderId,
  ProviderSnapshot,
  ThemeMode,
} from "./types";

export async function mountApp(root: HTMLElement): Promise<void> {
  root.innerHTML = `
    <div class="panel" id="glass-panel">
      <div id="header-root"></div>
      <div class="content" id="content-root">
        <div id="settings-root"></div>
        <div id="providers-root"></div>
      </div>
    </div>
  `;

  const panel = root.querySelector("#glass-panel") as HTMLElement;
  const headerRoot = root.querySelector("#header-root") as HTMLElement;
  const providersRoot = root.querySelector("#providers-root") as HTMLElement;
  const settingsRoot = root.querySelector("#settings-root") as HTMLElement;

  let settingsOpen = false;

  const state = await invoke<PersistedState>("get_state");
  const theme: ThemeMode = state.settings.theme ?? "system";
  const opacity = state.settings.opacity ?? 0.92;

  applyThemeToDocument(theme);
  applyPanelOpacity(panel, opacity);

  const providers = mountProviders(providersRoot);

  const settings = mountSettingsPanel(settingsRoot, state.settings, {
    onThemeChange: async (t) => {
      applyThemeToDocument(t);
      await invoke("set_theme", { theme: t });
    },
    onOpacityChange: async (o) => {
      applyPanelOpacity(panel, o);
      await invoke("set_opacity", { opacity: o });
    },
    onRefreshSecs: async (n) => {
      await invoke("set_refresh_secs", { secs: n });
    },
    onAutostart: async (v) => {
      await invoke("set_autostart", { enabled: v });
    },
    onUseTokscale: async (v) => {
      await invoke("set_use_tokscale", { enabled: v });
    },
    onProviderEnabled: async (id, enabled) => {
      await invoke("set_provider_enabled", { provider: id, enabled });
    },
    onLimits: async (id, five, weekly) => {
      const limits: PlanLimits = {
        five_hour_tokens: five,
        weekly_tokens: weekly,
      };
      await invoke("set_provider_limits", { provider: id as ProviderId, limits });
    },
    onDiagnostics: async () => {
      const diag = await invoke<DiagnosticsSnapshot>("get_diagnostics");
      const text = diag.lines.join("\n");
      try {
        await navigator.clipboard.writeText(text);
      } catch {
        console.log(text);
      }
    },
    onQuit: async () => {
      await invoke("quit_app");
    },
  });

  function toggleSettings(): void {
    settingsOpen = !settingsOpen;
    if (settingsOpen) settings.show();
    else settings.hide();
    setSettingsButtonActive(headerRoot, settingsOpen);
    void scheduleContentMin();
  }

  async function doRefresh(): Promise<void> {
    const snaps = await invoke<ProviderSnapshot[]>("refresh_now");
    providers.setSnapshots(snaps);
    void scheduleContentMin();
  }

  renderHeader(headerRoot, {
    onSettings: toggleSettings,
    onRefresh: () => {
      void doRefresh();
    },
    onHide: () => {
      void invoke("hide_widget");
    },
  });

  try {
    const snaps = await invoke<ProviderSnapshot[]>("get_snapshots");
    providers.setSnapshots(snaps);
  } catch {
    /* empty until first refresh */
  }

  await listen<ProviderSnapshot[]>("snapshots-updated", (ev) => {
    providers.setSnapshots(ev.payload);
    void scheduleContentMin();
  });

  // Persist geometry on move/resize (best-effort)
  const win = getCurrentWindow();
  const persistGeometry = async () => {
    try {
      const pos = await win.outerPosition();
      const size = await win.innerSize();
      const factor = await win.scaleFactor();
      await invoke("set_window_geometry", {
        geometry: {
          x: pos.x / factor,
          y: pos.y / factor,
          width: size.width / factor,
          height: size.height / factor,
        },
      });
    } catch {
      /* ignore */
    }
  };
  await win.onMoved(() => {
    void persistGeometry();
  });
  await win.onResized(() => {
    void persistGeometry();
  });

  async function scheduleContentMin(): Promise<void> {
    requestAnimationFrame(async () => {
      const panelEl = document.getElementById("glass-panel");
      if (!panelEl) return;
      const rect = panelEl.getBoundingClientRect();
      const w = Math.ceil(rect.width);
      const h = Math.ceil(rect.height);
      try {
        await invoke("set_content_min_size", { width: w, height: h });
      } catch {
        /* ignore */
      }
    });
  }

  void scheduleContentMin();
  // Kick a refresh so first paint has real data
  void doRefresh();
}
