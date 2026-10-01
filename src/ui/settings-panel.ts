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

const ICON_DOWNLOAD = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M8 2.5v7.5M5 7.25 8 10.25 11 7.25" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M3 12.5h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;

export function mountSettingsPanel(
  root: HTMLElement,
  settings: AppSettings,
  handlers: {
    onAutostart: (v: boolean) => void;
    onHoverDetail: (v: boolean) => void;
    onOpacityChange: (o: number) => void;
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
      <div class="settings-section">
        <div class="settings-label-row opacity-label-row">
          <span class="settings-label">Opacity</span>
          <span class="settings-value opacity-value" id="opacity-val">${initialPct}%</span>
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
        <label class="settings-toggle" for="autostart">
          <span class="settings-toggle-title">Start with Windows</span>
          <input type="checkbox" id="autostart" class="settings-switch-input" />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
      </div>

      <div class="settings-section">
        <label class="settings-toggle" for="hover-detail">
          <span class="settings-toggle-title">Open on hover</span>
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
        <div class="settings-update">
          <span class="settings-update-title">Version</span>
          <div class="settings-update-copy">
            <span class="settings-meta">${versionStr || "v0.0.0"}</span>
            <span class="settings-update-status" id="update-status"></span>
            <button type="button" class="icon-btn settings-update-btn" id="btn-check-update" aria-label="Check for updates" title="Check for updates">${ICON_DOWNLOAD}</button>
          </div>
        </div>
        <div class="settings-action-row">
          <button type="button" class="settings-debug" id="btn-diag" title="Copy diagnostic log for troubleshooting">Copy Log</button>
          <button type="button" class="settings-quit" id="btn-quit">Quit</button>
        </div>
      </div>
    </div>
  `;

  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const opacityRange = root.querySelector("#opacity-range") as HTMLInputElement;
  const opacityVal = root.querySelector("#opacity-val") as HTMLElement;
  const opacityMeter = root.querySelector(".opacity-meter") as HTMLElement;
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
    destroy() {
      for (const u of unlisteners) u();
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
