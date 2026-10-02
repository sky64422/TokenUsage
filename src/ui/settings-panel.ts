import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  meterFillPct,
  opacityTicksHtml,
  opacityToPct,
  OPACITY_MAX_PCT,
  OPACITY_MIN_PCT,
  OPACITY_STEP_PCT,
  pctToOpacity,
  snapOpacityPct,
} from "./opacity";
import type { AppSettings, ProviderId } from "./types";
import { PROVIDER_IDS } from "./types";

import claudeMark from "../assets/marks/claude.svg";
import codexMark from "../assets/marks/codex.svg";
import grokMark from "../assets/marks/grok.svg";
import agyMark from "../assets/marks/agy.svg";

const MARKS: Record<ProviderId, string> = {
  claude: claudeMark,
  codex: codexMark,
  grok: grokMark,
  agy: agyMark,
};

export type UpdatePhase = "idle" | "downloading" | "ready";

export interface UpdateInfo {
  current_version: string;
  version: string;
}

export interface DownloadProgress {
  version: string;
  chunk_len: number;
  content_length: number | null;
  received: number;
}

const ICON_DOWNLOAD = `<svg class="icon-svg" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M8 2.5v7.5M5 7.25 8 10.25 11 7.25" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M3 12.5h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;

export function mountSettingsPanel(
  root: HTMLElement,
  settings: AppSettings,
  handlers: {
    onAutostart: (v: boolean) => void;
    onHoverDetail: (v: boolean) => void;
    onAlwaysShowNotch?: (v: boolean) => void;
    onOpacityChange: (o: number) => void;
    onProviderEnabled: (id: ProviderId, enabled: boolean) => void | Promise<void>;
    onClose?: () => void;
    onDiagnostics: () => void | Promise<void>;
    onQuit: () => void;
  },
  appVersion = "",
): {
  show: () => void;
  hide: () => void;
  isVisible: () => boolean;
  syncProviderEnabled: (st: AppSettings) => void;
  destroy: () => void;
} {
  let visible = false;
  const versionStr = appVersion ? `v${appVersion.replace(/^v/i, "")}` : "";
  const initialPct = opacityToPct(settings.opacity);

  let updatePhase: UpdatePhase = "idle";
  let updateVersion: string | null = null;
  let updateHint = "";
  let updateBusy = false;

  root.innerHTML = `
    <div class="settings" id="settings-sheet">
      <div class="settings-header">
        <div class="settings-title-group">
          <span class="settings-title">Settings</span>
        </div>
        <button type="button" class="settings-close-btn" id="btn-close-settings" aria-label="Close settings" title="Close settings">
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        </button>
      </div>

      <div class="settings-card opacity-card">
        <span class="settings-card-label">Opacity</span>
        <div class="opacity-meter" style="--opacity-fill: ${meterFillPct(initialPct)}%">
          <div class="opacity-meter-fill" aria-hidden="true"></div>
          <div class="opacity-meter-ticks" aria-hidden="true">${opacityTicksHtml()}</div>
          <input type="range" id="opacity-range" class="opacity-meter-input"
            min="${OPACITY_MIN_PCT}" max="${OPACITY_MAX_PCT}" step="${OPACITY_STEP_PCT}"
            value="${initialPct}" aria-label="Opacity" />
        </div>
        <span class="opacity-value" id="opacity-val">${initialPct}%</span>
      </div>

      <div class="settings-group">
        <label class="settings-group-row" for="always-show">
          <span class="settings-group-title">Always show notch</span>
          <input type="checkbox" id="always-show" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>

        <label class="settings-group-row" for="hover-detail">
          <span class="settings-group-title">Open on hover</span>
          <input type="checkbox" id="hover-detail" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>

        <label class="settings-group-row" for="autostart">
          <span class="settings-group-title">Start with Windows</span>
          <input type="checkbox" id="autostart" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
      </div>

      <div class="settings-section">
        <span class="settings-label">Models</span>
        <div class="provider-grid" role="group" aria-label="Models">
          ${providerCard("claude", "Claude", MARKS.claude, settings.claude.enabled !== false)}
          ${providerCard("codex", "Codex", MARKS.codex, settings.codex.enabled !== false)}
          ${providerCard("grok", "Grok", MARKS.grok, settings.grok.enabled !== false)}
          ${providerCard("agy", "Antigravity", MARKS.agy, settings.agy.enabled !== false)}
        </div>
      </div>

      <div class="settings-end">
        <div class="settings-update">
          <span class="settings-meta" title="${versionStr || "v0.0.0"}">${versionStr || "v0.0.0"}</span>
          <span class="settings-update-status" id="update-status"></span>
          <button type="button" class="icon-btn settings-update-btn" id="btn-check-update" aria-label="Check for updates" title="Check for updates">${ICON_DOWNLOAD}</button>
        </div>
        <div class="settings-action-row">
          <button type="button" class="settings-pill-action" id="btn-diag" title="Copy diagnostic log for troubleshooting">
            <svg class="diag-check-icon" width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true">
              <path d="M2.5 6.5l2.5 2.5 4.5-5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <span class="diag-text">Copy Log</span>
          </button>
          <button type="button" class="settings-pill-action is-quit" id="btn-quit">Quit</button>
        </div>
      </div>
    </div>
  `;

  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const opacityRange = root.querySelector("#opacity-range") as HTMLInputElement;
  const opacityVal = root.querySelector("#opacity-val") as HTMLElement;
  const opacityMeter = root.querySelector(".opacity-meter") as HTMLElement;
  const alwaysShow = root.querySelector("#always-show") as HTMLInputElement | null;
  const autostart = root.querySelector("#autostart") as HTMLInputElement;
  const hoverDetail = root.querySelector("#hover-detail") as HTMLInputElement;
  const updateBtn = root.querySelector("#btn-check-update") as HTMLButtonElement;

  const paintOpacity = (pct: number) => {
    const snapped = snapOpacityPct(pct);
    const o = pctToOpacity(snapped);
    opacityRange.value = String(snapped);
    opacityVal.textContent = `${snapped}%`;
    opacityMeter.style.setProperty("--opacity-fill", `${meterFillPct(snapped)}%`);
    handlers.onOpacityChange(o);
  };

  if (Math.abs(settings.opacity - pctToOpacity(initialPct)) > 0.001) {
    paintOpacity(initialPct);
  }

  opacityRange.addEventListener("input", () => {
    paintOpacity(Number(opacityRange.value));
  });
  opacityRange.addEventListener("change", () => {
    paintOpacity(Number(opacityRange.value));
  });

  if (alwaysShow) {
    alwaysShow.checked = Boolean(settings.always_show_notch);
    alwaysShow.addEventListener("change", () => {
      handlers.onAlwaysShowNotch?.(alwaysShow.checked);
    });
  }
  autostart.checked = settings.autostart;
  hoverDetail.checked = Boolean(settings.hover_detail);

  autostart.addEventListener("change", () => {
    handlers.onAutostart(autostart.checked);
  });
  hoverDetail.addEventListener("change", () => {
    handlers.onHoverDetail(hoverDetail.checked);
  });

  function paintUpdateUi(): void {
    const btn = root.querySelector<HTMLButtonElement>("#btn-check-update");
    const status = root.querySelector<HTMLElement>("#update-status");
    if (!btn || !status) return;

    btn.disabled = updateBusy;
    btn.classList.toggle("busy", updateBusy);
    btn.classList.toggle("update-available", updatePhase !== "idle");
    btn.classList.toggle("update-downloading", updatePhase === "downloading");
    btn.classList.toggle("update-ready", updatePhase === "ready");

    let title = "Check for updates";
    if (updatePhase === "ready" && updateVersion) {
      title = `Restart to install ${updateVersion}`;
      status.textContent = updateHint || `Update ${updateVersion} ready`;
    } else if (updatePhase === "downloading" && updateVersion) {
      title = updateHint || `Downloading ${updateVersion}…`;
      status.textContent = title;
    } else if (updateBusy) {
      title = "Checking for updates…";
      status.textContent = updateHint || title;
    } else {
      status.textContent = updateHint;
    }
    btn.setAttribute("title", title);
    btn.setAttribute("aria-label", title);
  }

  async function runUpdateAction(btn: HTMLButtonElement): Promise<void> {
    const phaseAtClick = updatePhase;
    const version = updateVersion;

    if (phaseAtClick === "downloading") {
      updateHint = "Still downloading…";
      paintUpdateUi();
      return;
    }

    updateBusy = true;
    if (phaseAtClick === "ready") {
      updateHint = version ? `Restarting to install ${version}…` : "Restarting…";
    } else {
      updateHint = "Checking…";
    }
    paintUpdateUi();

    try {
      const hasUpdate = await invoke<boolean>("check_for_updates");
      if (hasUpdate) {
        updateBusy = false;
        if (updatePhase === "idle") updatePhase = "downloading";
        updateHint = "Update found — downloading…";
        paintUpdateUi();
        return;
      }
      updatePhase = "idle";
      updateVersion = null;
      updateBusy = false;
      updateHint = "Up to date";
      paintUpdateUi();
      window.setTimeout(() => {
        if (updatePhase !== "idle") return;
        updateHint = "";
        paintUpdateUi();
      }, 2500);
    } catch (err) {
      console.error("check_for_updates failed", err);
      updateBusy = false;
      updateHint = formatUpdateError(err).slice(0, 120);
      if (phaseAtClick === "ready" && version) {
        updatePhase = "ready";
        updateVersion = version;
      }
      paintUpdateUi();
      window.setTimeout(() => {
        if (!btn.isConnected) return;
        updateHint = "";
        paintUpdateUi();
      }, 4000);
    }
  }

  function formatUpdateError(err: unknown): string {
    if (typeof err === "string") return err;
    if (err && typeof err === "object" && "message" in err) {
      return String((err as { message: unknown }).message);
    }
    return "Update check failed";
  }

  updateBtn.addEventListener("click", () => {
    void runUpdateAction(updateBtn);
  });
  paintUpdateUi();

  const unlisteners: Array<() => void> = [];
  void listen<UpdateInfo>("update-available", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "downloading";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `Downloading ${ev.payload.version}…`;
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<DownloadProgress>("update-download-progress", (ev) => {
    const p = ev.payload;
    if (!p?.version || updatePhase === "ready") return;
    updatePhase = "downloading";
    updateVersion = p.version;
    updateBusy = false;
    if (p.content_length && p.content_length > 0) {
      const pct = Math.min(99, Math.round((p.received / p.content_length) * 100));
      updateHint = `${p.version}… ${pct}%`;
    } else {
      updateHint = `Downloading ${p.version}…`;
    }
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<UpdateInfo>("update-ready", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "ready";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `${ev.payload.version} ready`;
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen("update-not-available", () => {
    if (updatePhase === "idle") return;
    updatePhase = "idle";
    updateVersion = null;
    updateBusy = false;
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<string>("update-failed", (ev) => {
    const msg = typeof ev.payload === "string" ? ev.payload : "Update failed";
    updateBusy = false;
    if (updatePhase === "ready") {
      paintUpdateUi();
      return;
    }
    updatePhase = "idle";
    updateHint = msg.slice(0, 120);
    paintUpdateUi();
    window.setTimeout(() => {
      if (updatePhase !== "idle") return;
      updateHint = "";
      paintUpdateUi();
    }, 4000);
  }).then((u) => unlisteners.push(u));

  function providerBtn(id: ProviderId): HTMLButtonElement | null {
    return root.querySelector<HTMLButtonElement>(`[data-provider="${id}"]`);
  }

  function setProviderOn(id: ProviderId, on: boolean): void {
    const btn = providerBtn(id);
    if (!btn) return;
    btn.classList.toggle("on", on);
    btn.classList.toggle("off", !on);
    btn.setAttribute("aria-pressed", on ? "true" : "false");
    const badge = btn.querySelector<HTMLElement>(".provider-status-badge");
    if (badge) badge.textContent = on ? "ON" : "OFF";
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
      const label = btn.querySelector(".provider-card-name")?.textContent?.trim() ?? id;
      const locked = onlyOne && isProviderOn(id);
      btn.classList.toggle("is-locked", locked);
      btn.disabled = locked;
      const badge = btn.querySelector<HTMLElement>(".provider-status-badge");
      if (locked) {
        btn.title = `${label} (required)`;
        if (badge) badge.textContent = "REQ";
      } else {
        btn.title = label;
        if (badge) badge.textContent = isProviderOn(id) ? "ON" : "OFF";
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

  const closeBtn = root.querySelector<HTMLButtonElement>("#btn-close-settings");
  if (closeBtn && handlers.onClose) {
    closeBtn.addEventListener("click", handlers.onClose);
  }

  const diagBtn = root.querySelector("#btn-diag") as HTMLButtonElement | null;
  const diagText = diagBtn?.querySelector<HTMLElement>(".diag-text");
  diagBtn?.addEventListener("click", () => {
    void Promise.resolve(handlers.onDiagnostics()).then(() => {
      if (!diagBtn) return;
      if (diagText) diagText.textContent = "Copied";
      diagBtn.classList.add("is-done");
      window.setTimeout(() => {
        if (!diagBtn.isConnected) return;
        if (diagText) diagText.textContent = "Copy Log";
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
        const badge = btn.querySelector<HTMLElement>(".provider-status-badge");
        if (badge) badge.textContent = on ? "ON" : "OFF";
      });
      syncProviderLocks();
    },
    destroy() {
      for (const u of unlisteners) u();
    },
  };
}

function providerCard(id: ProviderId, label: string, markUrl: string, enabled: boolean): string {
  const state = enabled ? "on" : "off";
  const badgeText = enabled ? "ON" : "OFF";
  return `
    <button type="button"
      class="provider-card-btn ${state}"
      data-provider="${id}"
      aria-pressed="${enabled ? "true" : "false"}"
      title="${label}">
      <div class="provider-card-top">
        <img src="${markUrl}" class="provider-card-icon" alt="" draggable="false" />
        <span class="provider-status-badge">${badgeText}</span>
      </div>
      <span class="provider-card-name">${label}</span>
    </button>
  `;
}
