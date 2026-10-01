import type { AppSettings, ProviderId } from "./types";
import { PROVIDER_IDS } from "./types";

export function mountSettingsPanel(
  root: HTMLElement,
  settings: AppSettings,
  handlers: {
    onAutostart: (v: boolean) => void;
    onHoverDetail: (v: boolean) => void;
    onProviderEnabled: (id: ProviderId, enabled: boolean) => void | Promise<void>;
    onDiagnostics: () => void | Promise<void>;
    onQuit: () => void;
  },
  appVersion = "",
): {
  show: () => void;
  hide: () => void;
  isVisible: () => boolean;
  syncProviderEnabled: (st: AppSettings) => void;
} {
  let visible = false;
  const versionLabel = appVersion ? `v${appVersion.replace(/^v/i, "")}` : "";

  root.innerHTML = `
    <div class="settings" id="settings-sheet">
      <div class="settings-section">
        <label class="settings-toggle" for="autostart">
          <span class="settings-toggle-text">
            <span class="settings-toggle-title">Launch at login</span>
            <span class="settings-toggle-hint">Start with Windows</span>
          </span>
          <input type="checkbox" id="autostart" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
      </div>

      <div class="settings-section">
        <label class="settings-toggle" for="hover-detail">
          <span class="settings-toggle-text">
            <span class="settings-toggle-title">Open on hover</span>
            <span class="settings-toggle-hint">Hover to preview details</span>
          </span>
          <input type="checkbox" id="hover-detail" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
      </div>

      <div class="settings-section">
        <div class="settings-label">Providers</div>
        <div class="provider-chip-row" role="group" aria-label="Providers">
          ${providerChip("claude", "Claude", settings.claude.enabled !== false)}
          ${providerChip("codex", "Codex", settings.codex.enabled !== false)}
          ${providerChip("grok", "Grok", settings.grok.enabled !== false)}
        </div>
      </div>

      <div class="settings-end">
        <span class="settings-meta">${settings.hotkey} · ↻ update</span>
        <div class="settings-action-row">
          <button type="button" class="settings-debug" id="btn-diag" title="Copy diagnostic log for troubleshooting">Copy Log</button>
          <button type="button" class="settings-quit" id="btn-quit">Quit</button>
        </div>
        ${versionLabel ? `<span class="settings-version">${versionLabel}</span>` : ""}
      </div>
    </div>
  `;

  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const autostart = root.querySelector("#autostart") as HTMLInputElement;
  const hoverDetail = root.querySelector("#hover-detail") as HTMLInputElement;

  autostart.checked = settings.autostart;
  hoverDetail.checked = Boolean(settings.hover_detail);

  autostart.addEventListener("change", () => {
    handlers.onAutostart(autostart.checked);
  });
  hoverDetail.addEventListener("change", () => {
    handlers.onHoverDetail(hoverDetail.checked);
  });

  function providerBtn(id: ProviderId): HTMLButtonElement | null {
    return root.querySelector<HTMLButtonElement>(`[data-provider="${id}"]`);
  }

  function setProviderOn(id: ProviderId, on: boolean): void {
    const btn = providerBtn(id);
    if (!btn) return;
    btn.classList.toggle("on", on);
    btn.classList.toggle("off", !on);
    btn.setAttribute("aria-pressed", on ? "true" : "false");
    syncProviderLocks();
  }

  function isProviderOn(id: ProviderId): boolean {
    return providerBtn(id)?.classList.contains("on") ?? false;
  }

  function countEnabled(): number {
    return PROVIDER_IDS.filter(isProviderOn).length;
  }

  /** Last remaining provider cannot be turned off — visual + a11y lock. */
  function syncProviderLocks(): void {
    const onlyOne = countEnabled() <= 1;
    PROVIDER_IDS.forEach((id) => {
      const btn = providerBtn(id);
      if (!btn) return;
      const locked = onlyOne && isProviderOn(id);
      btn.classList.toggle("is-locked", locked);
      btn.disabled = locked;
      if (locked) {
        btn.title = `${btn.textContent?.trim() ?? id} (required)`;
      } else {
        btn.title = btn.textContent?.trim() ?? id;
      }
    });
  }

  PROVIDER_IDS.forEach((id) => {
    const btn = providerBtn(id);
    btn?.addEventListener("click", () => {
      const next = !isProviderOn(id);
      if (!next && countEnabled() <= 1) {
        // Keep at least one provider on
        return;
      }
      setProviderOn(id, next);
      void Promise.resolve(handlers.onProviderEnabled(id, next)).catch(() => {
        setProviderOn(id, !next);
      });
    });
  });

  syncProviderLocks();

  const diagBtn = root.querySelector("#btn-diag") as HTMLButtonElement | null;
  const diagLabel = "Copy Log";
  diagBtn?.addEventListener("click", () => {
    void Promise.resolve(handlers.onDiagnostics()).then(() => {
      if (!diagBtn) return;
      diagBtn.textContent = "Copied";
      diagBtn.classList.add("is-done");
      window.setTimeout(() => {
        if (!diagBtn.isConnected) return;
        diagBtn.textContent = diagLabel;
        diagBtn.classList.remove("is-done");
      }, 1400);
    });
  });
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
      PROVIDER_IDS.forEach((id) => {
        const btn = providerBtn(id);
        const on = st[id]?.enabled !== false;
        if (!btn) return;
        btn.classList.toggle("on", on);
        btn.classList.toggle("off", !on);
        btn.setAttribute("aria-pressed", on ? "true" : "false");
      });
      syncProviderLocks();
    },
  };
}

function providerChip(id: string, label: string, enabled: boolean): string {
  const state = enabled ? "on" : "off";
  return `
    <button type="button"
      class="provider-chip ${state}"
      data-provider="${id}"
      aria-pressed="${enabled ? "true" : "false"}"
      title="${label}">
      ${label}
    </button>
  `;
}
