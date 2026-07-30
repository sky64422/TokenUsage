import type { AppSettings, ProviderId, ThemeMode } from "./types";

const REFRESH_PRESETS = [5, 10, 15, 30, 60] as const;

export function applyThemeToDocument(theme: ThemeMode): void {
  document.documentElement.dataset.theme = theme;
}

/**
 * Glass opacity + matching text/graph alpha.
 * Background uses --panel-opacity; fg/accent/chrome track the slider so bars
 * and labels don't stay fully solid while the panel goes transparent.
 */
export function applyPanelOpacity(panel: HTMLElement, opacity: number): void {
  const o = Math.min(1, Math.max(0.35, opacity));
  const fg = Math.min(1, Math.max(0.62, o * 1.02));
  const accent = Math.min(1, Math.max(0.55, o * 1.05));
  const chrome = Math.min(1, Math.max(0.4, o));

  const root = document.documentElement;
  for (const el of [panel, root]) {
    el.style.setProperty("--panel-opacity", String(o));
    el.style.setProperty("--fg-opacity", String(fg));
    el.style.setProperty("--accent-opacity", String(accent));
    el.style.setProperty("--chrome-opacity", String(chrome));
  }
}

function formatRefresh(secs: number): string {
  if (secs < 60) return `${secs}s`;
  return `${secs / 60}m`;
}

export function mountSettingsPanel(
  root: HTMLElement,
  settings: AppSettings,
  handlers: {
    onThemeChange: (t: ThemeMode) => void;
    onOpacityChange: (o: number) => void;
    onRefreshSecs: (n: number) => void;
    onAutostart: (v: boolean) => void;
    onUseTokscale: (v: boolean) => void;
    onUseDirectQuota: (v: boolean) => void;
    onProviderEnabled: (id: ProviderId, enabled: boolean) => void | Promise<void>;
    onDiagnostics: () => void;
    onQuit: () => void;
  },
): {
  show: () => void;
  hide: () => void;
  isVisible: () => boolean;
  syncProviderEnabled: (st: AppSettings) => void;
} {
  let visible = false;
  let refreshSecs = settings.refresh_secs ?? 5;
  if (!REFRESH_PRESETS.includes(refreshSecs as (typeof REFRESH_PRESETS)[number])) {
    // Snap odd saved values to nearest preset for chip UI
    refreshSecs = REFRESH_PRESETS.reduce((best, p) =>
      Math.abs(p - refreshSecs) < Math.abs(best - refreshSecs) ? p : best,
    );
  }

  root.innerHTML = `
    <div class="settings" id="settings-sheet">
      <div class="settings-section">
        <div class="settings-label">Theme</div>
        <div class="segmented" id="theme-seg" role="group" aria-label="Theme">
          <button type="button" data-theme="system">Auto</button>
          <button type="button" data-theme="light">Light</button>
          <button type="button" data-theme="dark">Dark</button>
        </div>
      </div>

      <div class="settings-section">
        <div class="settings-label-row">
          <div class="settings-label">Opacity</div>
          <div class="settings-value" id="opacity-val"></div>
        </div>
        <input type="range" id="opacity-range" class="settings-range" min="0.35" max="1" step="0.01" />
      </div>

      <div class="settings-section">
        <div class="settings-label">Refresh</div>
        <div class="segmented refresh-segmented" id="refresh-seg" role="group" aria-label="Refresh interval">
          ${REFRESH_PRESETS.map(
            (s) => `
            <button type="button" data-refresh="${s}" class="${s === refreshSecs ? "active" : ""}">${formatRefresh(s)}</button>
          `,
          ).join("")}
        </div>
      </div>

      <div class="settings-section">
        <label class="settings-toggle" for="autostart">
          <span class="settings-toggle-text">
            <span class="settings-toggle-title">Launch at login</span>
            <span class="settings-toggle-hint">Start with Windows</span>
          </span>
          <input type="checkbox" id="autostart" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
        <label class="settings-toggle" for="use-direct-quota">
          <span class="settings-toggle-text">
            <span class="settings-toggle-title">Direct vendor quota</span>
            <span class="settings-toggle-hint">Local OAuth → API (Claude / Codex / Grok)</span>
          </span>
          <input type="checkbox" id="use-direct-quota" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
        <label class="settings-toggle" for="use-tokscale">
          <span class="settings-toggle-text">
            <span class="settings-toggle-title">Use tokscale</span>
            <span class="settings-toggle-hint">2nd path when direct vendor fails</span>
          </span>
          <input type="checkbox" id="use-tokscale" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
      </div>

      <div class="settings-section">
        <div class="settings-label">Providers</div>
        <p class="settings-lede">Toggle visibility. At least one stays on. Quotas from vendor API, then tokscale.</p>
        <div class="provider-list-settings">
          ${providerToggleBlock("claude", "Claude", settings.claude)}
          ${providerToggleBlock("codex", "Codex", settings.codex)}
          ${providerToggleBlock("grok", "Grok", settings.grok)}
        </div>
      </div>

      <div class="settings-actions">
        <button type="button" class="btn-text" id="btn-diag">Diagnostics</button>
        <button type="button" class="btn-text btn-danger" id="btn-quit">Quit</button>
      </div>
      <p class="settings-footer">Hotkey ${settings.hotkey} · Updates via header ↻</p>
    </div>
  `;

  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const themeSeg = root.querySelector("#theme-seg") as HTMLElement;
  const refreshSeg = root.querySelector("#refresh-seg") as HTMLElement;
  const opacityRange = root.querySelector("#opacity-range") as HTMLInputElement;
  const opacityVal = root.querySelector("#opacity-val") as HTMLElement;
  const autostart = root.querySelector("#autostart") as HTMLInputElement;
  const useTokscale = root.querySelector("#use-tokscale") as HTMLInputElement;
  const useDirectQuota = root.querySelector("#use-direct-quota") as HTMLInputElement;

  function markTheme(t: ThemeMode): void {
    themeSeg.querySelectorAll("button").forEach((b) => {
      b.classList.toggle("active", (b as HTMLElement).dataset.theme === t);
    });
  }

  function markRefresh(secs: number): void {
    refreshSeg.querySelectorAll("button").forEach((b) => {
      b.classList.toggle("active", Number((b as HTMLElement).dataset.refresh) === secs);
    });
  }

  markTheme(settings.theme);
  markRefresh(refreshSecs);
  opacityRange.value = String(settings.opacity);
  opacityVal.textContent = `${Math.round(settings.opacity * 100)}%`;
  autostart.checked = settings.autostart;
  useTokscale.checked = settings.use_tokscale !== false;
  useDirectQuota.checked = settings.use_direct_quota !== false;

  themeSeg.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", () => {
      const t = (btn as HTMLElement).dataset.theme as ThemeMode;
      markTheme(t);
      handlers.onThemeChange(t);
    });
  });

  refreshSeg.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", () => {
      const secs = Number((btn as HTMLElement).dataset.refresh);
      if (!Number.isFinite(secs)) return;
      refreshSecs = secs;
      markRefresh(secs);
      handlers.onRefreshSecs(secs);
    });
  });

  opacityRange.addEventListener("input", () => {
    const o = Number(opacityRange.value);
    opacityVal.textContent = `${Math.round(o * 100)}%`;
    handlers.onOpacityChange(o);
  });

  autostart.addEventListener("change", () => {
    handlers.onAutostart(autostart.checked);
  });

  useTokscale.addEventListener("change", () => {
    handlers.onUseTokscale(useTokscale.checked);
  });

  useDirectQuota.addEventListener("change", () => {
    handlers.onUseDirectQuota(useDirectQuota.checked);
  });

  function updateToggleHint(id: ProviderId, enabled: boolean): void {
    const hint = root.querySelector(`#en-${id}`)?.closest(".provider-card-settings")
      ?.querySelector(".provider-toggle-hint");
    if (hint) hint.textContent = enabled ? "Visible" : "Hidden";
  }

  function countEnabled(): number {
    return (["claude", "codex", "grok"] as ProviderId[]).filter((id) => {
      const en = root.querySelector(`#en-${id}`) as HTMLInputElement | null;
      return en?.checked;
    }).length;
  }

  (["claude", "codex", "grok"] as ProviderId[]).forEach((id) => {
    const en = root.querySelector(`#en-${id}`) as HTMLInputElement;
    en?.addEventListener("change", () => {
      if (!en.checked && countEnabled() === 0) {
        en.checked = true;
        updateToggleHint(id, true);
        return;
      }
      updateToggleHint(id, en.checked);
      void Promise.resolve(handlers.onProviderEnabled(id, en.checked)).catch(() => {
        en.checked = !en.checked;
        updateToggleHint(id, en.checked);
      });
    });
  });

  root.querySelector("#btn-diag")?.addEventListener("click", handlers.onDiagnostics);
  root.querySelector("#btn-quit")?.addEventListener("click", handlers.onQuit);

  return {
    show() {
      visible = true;
      sheet.classList.add("visible");
    },
    hide() {
      visible = false;
      sheet.classList.remove("visible");
    },
    isVisible: () => visible,
    syncProviderEnabled(st: AppSettings) {
      (["claude", "codex", "grok"] as ProviderId[]).forEach((id) => {
        const en = root.querySelector(`#en-${id}`) as HTMLInputElement | null;
        if (!en) return;
        const on = st[id]?.enabled !== false;
        en.checked = on;
        updateToggleHint(id, on);
      });
    },
  };
}

function providerToggleBlock(
  id: string,
  label: string,
  cfg: { enabled: boolean },
): string {
  return `
    <div class="provider-card-settings">
      <label class="settings-toggle provider-vis-toggle" for="en-${id}" title="Show or hide this provider">
        <span class="settings-toggle-text">
          <span class="settings-toggle-title">${label}</span>
          <span class="provider-toggle-hint">${cfg.enabled ? "Visible" : "Hidden"}</span>
        </span>
        <input type="checkbox" id="en-${id}" class="settings-switch-input" ${cfg.enabled ? "checked" : ""} />
        <span class="settings-switch" aria-hidden="true"></span>
      </label>
    </div>
  `;
}
