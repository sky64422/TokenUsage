import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, ProviderId, ThemeMode } from "./types";

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
  // Keep type slightly stronger than glass so low opacity stays readable
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
      <div class="settings-hint" id="update-status" style="margin-top:4px" aria-live="polite"></div>
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

  const updateBtn = root.querySelector("#btn-update-app") as HTMLButtonElement | null;
  const updateStatus = root.querySelector("#update-status") as HTMLElement | null;
  updateBtn?.addEventListener("click", () => {
    void (async () => {
      if (!updateBtn) return;
      updateBtn.disabled = true;
      if (updateStatus) updateStatus.textContent = "Checking for updates…";
      try {
        const installed = await invoke<boolean>("check_for_updates");
        if (updateStatus) {
          updateStatus.textContent = installed
            ? "Update installed. Restart the app if it does not reopen."
            : "You're on the latest version.";
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (updateStatus) {
          updateStatus.textContent = formatUpdateError(msg);
        }
      } finally {
        updateBtn.disabled = false;
      }
    })();
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

/** Human-readable updater failures (private repo / network / signature). */
function formatUpdateError(raw: string): string {
  const s = raw.replace(/^Error:\s*/i, "").trim();
  if (/404|not found|failed to fetch|error sending request|dns|timed out|cannot reach/i.test(s)) {
    return "Update check failed: cannot download release (private GitHub repo or network). Make the repo public, or install from Releases manually.";
  }
  if (/signature|minisign|key/i.test(s)) {
    return `Update check failed (signature): ${s}`;
  }
  return s ? `Update check failed: ${s}` : "Update check failed.";
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
