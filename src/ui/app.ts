import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CHROME_MIN_H,
  measureContentHugHeight,
  POLICY_MIN_W,
} from "./content-size";
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
      scheduleContentMin(true);
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
    onProviderEnabled: async (id, enabled) => {
      try {
        const snaps = await invoke<ProviderSnapshot[]>("set_provider_enabled", {
          provider: id,
          enabled,
        });
        providers.setSnapshots(snaps);
        scheduleContentMin(true);
      } catch (err) {
        console.error("set_provider_enabled failed", err);
        // Re-sync checkboxes from server state if hide-all was rejected
        try {
          const st = await invoke<PersistedState>("get_state");
          settings.syncProviderEnabled?.(st.settings);
        } catch {
          /* ignore */
        }
        throw err;
      }
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
    // Overlay only: providers fade under an opaque settings sheet. Window size
    // stays put (no content-hug grow/shrink for the sheet).
    if (settingsOpen) {
      settings.show();
      panel.classList.add("settings-open");
    } else {
      settings.hide();
      panel.classList.remove("settings-open");
    }
    setSettingsButtonActive(headerRoot, settingsOpen);
  }

  async function doRefresh(): Promise<void> {
    const snaps = await invoke<ProviderSnapshot[]>("refresh_now");
    providers.setSnapshots(snaps);
    scheduleContentMin(true);
  }

  renderHeader(headerRoot, {
    onSettings: toggleSettings,
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
  void doRefresh();

  await listen<ProviderSnapshot[]>("snapshots-updated", (ev) => {
    providers.setSnapshots(ev.payload);
    scheduleContentMin(true);
  });

  // —— Content-hug min size (WarRoom pattern) ——
  let lastContentMinH = 0;
  let contentMinTimer: ReturnType<typeof setTimeout> | null = null;

  const syncContentMinSize = async (opts: { growIfNeeded: boolean }) => {
    try {
      const contentH = measureContentHugHeight(panel);
      const minHeight = Math.max(CHROME_MIN_H, contentH);
      const grew = minHeight > lastContentMinH + 0.5;
      const shrank = minHeight < lastContentMinH - 0.5;
      const changed = Math.abs(minHeight - lastContentMinH) >= 1;
      // Snap when content grew/shrank (provider cards), or explicit fit (boot).
      const fit = opts.growIfNeeded || grew || shrank;
      if (!changed && !opts.growIfNeeded) return;
      lastContentMinH = minHeight;
      await invoke("set_content_min_size", {
        width: POLICY_MIN_W,
        height: minHeight,
        grow_if_needed: fit,
      });
    } catch (err) {
      console.error("set_content_min_size failed", err);
    }
  };

  function scheduleContentMin(growIfNeeded: boolean): void {
    if (contentMinTimer) clearTimeout(contentMinTimer);
    contentMinTimer = setTimeout(() => {
      void syncContentMinSize({ growIfNeeded });
    }, 40);
  }

  // Content mutations only — never remeasure min from window resize
  // (that caused min to track the drag and bounce with setSize).
  const mutObs = new MutationObserver(() => {
    scheduleContentMin(false);
  });
  mutObs.observe(panel, {
    childList: true,
    subtree: true,
    characterData: true,
    attributes: true,
    attributeFilter: ["class", "style", "hidden"],
  });

  // Geometry persistence
  const win = getCurrentWindow();
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
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
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void persistGeometry();
    }, 250);
  });
  // Persist only; OS min + Rust Resized clamp handle the hard wall.
  await win.onResized(() => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void persistGeometry();
    }, 250);
  });

  // Boot: measure after layout, install hard min, grow if restored size too small.
  requestAnimationFrame(() => {
    void syncContentMinSize({ growIfNeeded: true });
    window.setTimeout(() => {
      void syncContentMinSize({ growIfNeeded: true });
    }, 200);
  });

  void doRefresh();
}
