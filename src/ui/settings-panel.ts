import type { AppSettings, ProviderId } from "./types";

const REFRESH_PRESETS = [5, 10, 15, 30, 60] as const;

/** Opacity slider uses whole percent steps of 5 (35%…100%). */
const OPACITY_MIN_PCT = 35;
const OPACITY_MAX_PCT = 100;
const OPACITY_STEP_PCT = 5;

function snapOpacityPct(pct: number): number {
  const clamped = Math.min(OPACITY_MAX_PCT, Math.max(OPACITY_MIN_PCT, pct));
  return Math.round(clamped / OPACITY_STEP_PCT) * OPACITY_STEP_PCT;
}

function opacityToPct(o: number): number {
  return snapOpacityPct(Math.round(o * 100));
}

function pctToOpacity(pct: number): number {
  return snapOpacityPct(pct) / 100;
}

function meterFillPct(pct: number): number {
  const snapped = snapOpacityPct(pct);
  return ((snapped - OPACITY_MIN_PCT) / (OPACITY_MAX_PCT - OPACITY_MIN_PCT)) * 100;
}

function opacityTicksHtml(): string {
  const parts: string[] = [];
  for (let p = OPACITY_MIN_PCT; p <= OPACITY_MAX_PCT; p += OPACITY_STEP_PCT) {
    const major = p % 10 === 0 || p === OPACITY_MIN_PCT || p === OPACITY_MAX_PCT;
    parts.push(`<span class="opacity-tick${major ? " major" : ""}"></span>`);
  }
  return parts.join("");
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
    onOpacityChange: (o: number) => void;
    onRefreshSecs: (n: number) => void;
    onAutostart: (v: boolean) => void;
    onProviderEnabled: (id: ProviderId, enabled: boolean) => void | Promise<void>;
    onDiagnostics: () => void | Promise<void>;
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

  const initialPct = opacityToPct(settings.opacity);
  root.innerHTML = `
    <div class="settings" id="settings-sheet">
      <div class="settings-section">
        <div class="settings-label-row opacity-label-row">
          <div class="settings-label">Opacity</div>
          <div class="settings-value opacity-value" id="opacity-val">${initialPct}%</div>
        </div>
        <div class="opacity-meter" style="--opacity-fill: ${meterFillPct(initialPct)}%">
          <div class="opacity-meter-fill" aria-hidden="true"></div>
          <div class="opacity-meter-ticks" aria-hidden="true">${opacityTicksHtml()}</div>
          <input type="range" id="opacity-range" class="opacity-meter-input"
            min="${OPACITY_MIN_PCT}" max="${OPACITY_MAX_PCT}" step="${OPACITY_STEP_PCT}"
            value="${initialPct}" aria-label="Opacity" />
        </div>
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
        <span class="settings-meta">Hotkey ${settings.hotkey} · header ↻ for updates</span>
        <div class="settings-action-row">
          <button type="button" class="settings-debug" id="btn-diag" title="Copy diagnostic log for troubleshooting">Copy Log</button>
          <button type="button" class="settings-quit" id="btn-quit">Quit</button>
        </div>
      </div>
    </div>
  `;

  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const refreshSeg = root.querySelector("#refresh-seg") as HTMLElement;
  const opacityRange = root.querySelector("#opacity-range") as HTMLInputElement;
  const opacityVal = root.querySelector("#opacity-val") as HTMLElement;
  const autostart = root.querySelector("#autostart") as HTMLInputElement;

  function markRefresh(secs: number): void {
    refreshSeg.querySelectorAll("button").forEach((b) => {
      b.classList.toggle("active", Number((b as HTMLElement).dataset.refresh) === secs);
    });
  }

  markRefresh(refreshSecs);
  autostart.checked = settings.autostart;

  const opacityMeter = root.querySelector(".opacity-meter") as HTMLElement;

  const paintOpacity = (pct: number) => {
    const snapped = snapOpacityPct(pct);
    const o = pctToOpacity(snapped);
    opacityRange.value = String(snapped);
    opacityVal.textContent = `${snapped}%`;
    opacityMeter.style.setProperty("--opacity-fill", `${meterFillPct(snapped)}%`);
    handlers.onOpacityChange(o);
  };

  // Persist snapped value if legacy 1% step was stored
  if (Math.abs(settings.opacity - pctToOpacity(initialPct)) > 0.001) {
    paintOpacity(initialPct);
  }

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
    paintOpacity(Number(opacityRange.value));
  });

  opacityRange.addEventListener("change", () => {
    paintOpacity(Number(opacityRange.value));
  });

  autostart.addEventListener("change", () => {
    handlers.onAutostart(autostart.checked);
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
  }

  function isProviderOn(id: ProviderId): boolean {
    return providerBtn(id)?.classList.contains("on") ?? false;
  }

  function countEnabled(): number {
    return (["claude", "codex", "grok"] as ProviderId[]).filter(isProviderOn).length;
  }

  (["claude", "codex", "grok"] as ProviderId[]).forEach((id) => {
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
      (["claude", "codex", "grok"] as ProviderId[]).forEach((id) => {
        setProviderOn(id, st[id]?.enabled !== false);
      });
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
