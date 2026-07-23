import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, ProviderId, ThemeMode } from "./types";

export function applyThemeToDocument(theme: ThemeMode): void {
  document.documentElement.dataset.theme = theme;
}

export function applyPanelOpacity(panel: HTMLElement, opacity: number): void {
  panel.style.setProperty("--panel-opacity", String(opacity));
  document.documentElement.style.setProperty("--panel-opacity", String(opacity));
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
    onProviderEnabled: (id: ProviderId, enabled: boolean) => void;
    onLimits: (id: ProviderId, five: number, weekly: number | null) => void;
    onDiagnostics: () => void;
    onQuit: () => void;
  },
): { show: () => void; hide: () => void; isVisible: () => boolean } {
  let visible = false;

  root.innerHTML = `
    <div class="settings" id="settings-sheet">
      <div class="settings-row">
        <div class="settings-label">Theme</div>
        <div class="segmented" id="theme-seg">
          <button type="button" data-theme="system">Auto</button>
          <button type="button" data-theme="light">Light</button>
          <button type="button" data-theme="dark">Dark</button>
        </div>
      </div>
      <div class="settings-row">
        <div>
          <div class="settings-label">Opacity</div>
          <div class="settings-hint" id="opacity-val"></div>
        </div>
        <input type="range" id="opacity-range" min="0.35" max="1" step="0.01" />
      </div>
      <div class="settings-row">
        <div>
          <div class="settings-label">Refresh (sec)</div>
          <div class="settings-hint">Local scan interval</div>
        </div>
        <input type="number" id="refresh-secs" min="10" max="300" step="5" />
      </div>
      <div class="settings-row">
        <div class="settings-label">Launch at login</div>
        <input type="checkbox" id="autostart" />
      </div>
      <div class="settings-row">
        <div>
          <div class="settings-label">Use tokscale</div>
          <div class="settings-hint">Vendor quotas via <code>tokscale usage --json</code></div>
        </div>
        <input type="checkbox" id="use-tokscale" />
      </div>

      <div class="settings-section-title">Providers & local limits (fallback)</div>
      ${providerLimitBlock("claude", "Claude", settings.claude)}
      ${providerLimitBlock("codex", "Codex", settings.codex)}
      ${providerLimitBlock("grok", "Grok", settings.grok)}

      <div class="settings-row" style="margin-top:10px">
        <button type="button" class="btn-text" id="btn-diag">Copy diagnostics</button>
        <button type="button" class="btn-text btn-danger" id="btn-quit">Quit</button>
      </div>
      <div class="settings-hint" style="margin-top:8px">Hotkey ${settings.hotkey} · Double-click title to check app updates</div>
      <div class="settings-row" style="margin-top:4px">
        <button type="button" class="btn-text" id="btn-update-app">Check for updates</button>
      </div>
    </div>
  `;

  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const themeSeg = root.querySelector("#theme-seg") as HTMLElement;
  const opacityRange = root.querySelector("#opacity-range") as HTMLInputElement;
  const opacityVal = root.querySelector("#opacity-val") as HTMLElement;
  const refreshInput = root.querySelector("#refresh-secs") as HTMLInputElement;
  const autostart = root.querySelector("#autostart") as HTMLInputElement;
  const useTokscale = root.querySelector("#use-tokscale") as HTMLInputElement;

  function markTheme(t: ThemeMode): void {
    themeSeg.querySelectorAll("button").forEach((b) => {
      b.classList.toggle("active", (b as HTMLElement).dataset.theme === t);
    });
  }

  markTheme(settings.theme);
  opacityRange.value = String(settings.opacity);
  opacityVal.textContent = `${Math.round(settings.opacity * 100)}%`;
  refreshInput.value = String(settings.refresh_secs);
  autostart.checked = settings.autostart;
  useTokscale.checked = settings.use_tokscale !== false;

  themeSeg.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", () => {
      const t = (btn as HTMLElement).dataset.theme as ThemeMode;
      markTheme(t);
      handlers.onThemeChange(t);
    });
  });

  opacityRange.addEventListener("input", () => {
    const o = Number(opacityRange.value);
    opacityVal.textContent = `${Math.round(o * 100)}%`;
    handlers.onOpacityChange(o);
  });

  refreshInput.addEventListener("change", () => {
    handlers.onRefreshSecs(Number(refreshInput.value) || 30);
  });

  autostart.addEventListener("change", () => {
    handlers.onAutostart(autostart.checked);
  });

  useTokscale.addEventListener("change", () => {
    handlers.onUseTokscale(useTokscale.checked);
  });

  (["claude", "codex", "grok"] as ProviderId[]).forEach((id) => {
    const en = root.querySelector(`#en-${id}`) as HTMLInputElement;
    const five = root.querySelector(`#five-${id}`) as HTMLInputElement;
    const weekly = root.querySelector(`#weekly-${id}`) as HTMLInputElement;
    en?.addEventListener("change", () => handlers.onProviderEnabled(id, en.checked));
    const applyLimits = () => {
      const f = Number(five.value) || 0;
      const wRaw = weekly.value.trim();
      const w = wRaw === "" ? null : Number(wRaw);
      handlers.onLimits(id, f, w != null && Number.isFinite(w) ? w : null);
    };
    five?.addEventListener("change", applyLimits);
    weekly?.addEventListener("change", applyLimits);
  });

  root.querySelector("#btn-diag")?.addEventListener("click", handlers.onDiagnostics);
  root.querySelector("#btn-quit")?.addEventListener("click", handlers.onQuit);
  root.querySelector("#btn-update-app")?.addEventListener("click", () => {
    void invoke<boolean>("check_for_updates").catch(() => undefined);
  });

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
  };
}

function providerLimitBlock(
  id: string,
  label: string,
  cfg: { enabled: boolean; limits: { five_hour_tokens: number; weekly_tokens: number | null } },
): string {
  return `
    <div class="settings-row">
      <label class="provider-toggle">
        <input type="checkbox" id="en-${id}" ${cfg.enabled ? "checked" : ""} />
        ${label}
      </label>
      <div style="display:flex;gap:6px;align-items:center">
        <input type="number" id="five-${id}" title="5h token limit" value="${cfg.limits.five_hour_tokens}" />
        <input type="number" id="weekly-${id}" title="Weekly token limit" value="${cfg.limits.weekly_tokens ?? ""}" placeholder="weekly" />
      </div>
    </div>
  `;
}
