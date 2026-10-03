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
    onShowOrbit?: (v: boolean) => void;
    onShowPeriod?: (v: boolean) => void;
    onShowIconGlow?: (v: boolean) => void;
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
          <span class="settings-title">설정</span>
        </div>
        <button type="button" class="settings-close-btn" id="btn-close-settings" aria-label="설정 닫기" title="설정 닫기">
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        </button>
      </div>

      <div class="settings-tabs" role="tablist" aria-label="설정 탭">
        <button type="button" class="settings-tab-btn is-active" id="tab-btn-appearance" role="tab"
          aria-selected="true" aria-controls="tab-panel-appearance" tabindex="0">
          <svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M2 5h3m3 0h6M2 11h6m3 0h3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            <circle cx="6.5" cy="5" r="1.5" stroke="currentColor" stroke-width="1.5"/>
            <circle cx="9.5" cy="11" r="1.5" stroke="currentColor" stroke-width="1.5"/>
          </svg>
          <span class="tab-label">모양</span>
        </button>
        <button type="button" class="settings-tab-btn" id="tab-btn-models" role="tab"
          aria-selected="false" aria-controls="tab-panel-models" tabindex="-1">
          <svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M8 2.5L2 6l6 3.5L14 6 8 2.5z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
            <path d="M2 9.5l6 3.5 6-3.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span class="tab-label">모델</span>
        </button>
        <button type="button" class="settings-tab-btn" id="tab-btn-general" role="tab"
          aria-selected="false" aria-controls="tab-panel-general" tabindex="-1">
          <svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="2.5" stroke="currentColor" stroke-width="1.5"/>
            <path d="M8 1.5v1.8m0 9.4v1.8m6.5-6.5h-1.8m-9.4 0H1.5m11.1-4.6l-1.3 1.3m-6.6 6.6l-1.3 1.3m9.2 0l-1.3-1.3m-6.6-6.6l-1.3-1.3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
          <span class="tab-label">일반</span>
        </button>
      </div>

      <div class="settings-tab-panel" id="tab-panel-appearance" role="tabpanel" aria-labelledby="tab-btn-appearance">
        <div class="settings-card opacity-card">
          <span class="settings-card-label">투명도</span>
          <div class="opacity-meter" style="--opacity-fill: ${meterFillPct(initialPct)}%">
            <div class="opacity-meter-fill" aria-hidden="true"></div>
            <div class="opacity-meter-ticks" aria-hidden="true">${opacityTicksHtml()}</div>
            <input type="range" id="opacity-range" class="opacity-meter-input"
              min="${OPACITY_MIN_PCT}" max="${OPACITY_MAX_PCT}" step="${OPACITY_STEP_PCT}"
              value="${initialPct}" aria-label="투명도" />
          </div>
          <span class="opacity-value" id="opacity-val">${initialPct}%</span>
        </div>

        <div class="settings-section">
          <span class="settings-label">화면 표시</span>
          <div class="settings-group">
            <label class="settings-group-row" for="always-show" title="마우스가 벗어나도 가장자리에 노치를 유지합니다">
              <span class="settings-group-title">노치 항상 표시</span>
              <input type="checkbox" id="always-show" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>

            <label class="settings-group-row" for="hover-detail" title="클릭하지 않아도 마우스를 올리면 상세 사용량을 표시합니다">
              <span class="settings-group-title">호버 시 상세 열기</span>
              <input type="checkbox" id="hover-detail" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>

            <label class="settings-group-row" for="show-period" title="노치 링 옆에 리셋 주기(5h, week 등)를 표시합니다">
              <span class="settings-group-title">리셋 주기 표시</span>
              <input type="checkbox" id="show-period" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>
          </div>
        </div>

        <div class="settings-section">
          <span class="settings-label">애니메이션</span>
          <div class="settings-group">
            <label class="settings-group-row" for="show-orbit" title="작업 진행 중 링 주변을 회전하는 궤도를 표시합니다">
              <span class="settings-group-title">회전 애니메이션</span>
              <input type="checkbox" id="show-orbit" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>

            <label class="settings-group-row" for="show-icon-glow" title="작업 진행 중 모델 아이콘에 은은한 발광 효과를 부여합니다">
              <span class="settings-group-title">아이콘 발광 효과</span>
              <input type="checkbox" id="show-icon-glow" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>
          </div>
        </div>
      </div>

      <div class="settings-tab-panel" id="tab-panel-models" role="tabpanel" aria-labelledby="tab-btn-models" hidden>
        <div class="provider-grid" role="group" aria-label="모델 목록">
          ${providerCard("claude", "Claude", MARKS.claude, settings.claude.enabled !== false)}
          ${providerCard("codex", "Codex", MARKS.codex, settings.codex.enabled !== false)}
          ${providerCard("grok", "Grok", MARKS.grok, settings.grok.enabled !== false)}
          ${providerCard("agy", "Antigravity", MARKS.agy, settings.agy.enabled !== false)}
        </div>
      </div>

      <div class="settings-tab-panel" id="tab-panel-general" role="tabpanel" aria-labelledby="tab-btn-general" hidden>
        <div class="settings-section">
          <span class="settings-label">시스템</span>
          <div class="settings-group">
            <label class="settings-group-row" for="autostart">
              <span class="settings-group-title">Windows 시작 시 실행</span>
              <input type="checkbox" id="autostart" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>

            <div class="settings-group-row is-static">
              <span class="settings-group-title">버전</span>
              <div class="settings-update-inline">
                <span class="settings-meta" title="${versionStr || "v0.0.0"}">${versionStr || "v0.0.0"}</span>
                <span class="settings-update-status" id="update-status"></span>
                <button type="button" class="icon-btn settings-update-btn" id="btn-check-update" aria-label="업데이트 확인" title="업데이트 확인">${ICON_DOWNLOAD}</button>
              </div>
            </div>
          </div>
        </div>

        <div class="settings-action-row">
          <button type="button" class="settings-pill-action" id="btn-diag" title="문제 해결을 위한 진단 로그 복사" aria-label="진단 로그 복사">
            <span class="diag-icon-wrap" aria-hidden="true">
              <svg class="diag-copy-icon" width="12" height="12" viewBox="0 0 16 16" fill="none">
                <rect x="5" y="5" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
                <path d="M3.5 10.5H3a1.5 1.5 0 0 1-1.5-1.5V3a1.5 1.5 0 0 1 1.5-1.5h6a1.5 1.5 0 0 1 1.5 1.5v0.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
              </svg>
              <svg class="diag-check-icon" width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M3.5 8.5L6.5 11.5L12.5 4.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </span>
            <span class="diag-label-wrap" aria-hidden="true">
              <span class="diag-text diag-text-idle">Copy Log</span>
              <span class="diag-text diag-text-copied">Copied</span>
            </span>
          </button>
          <button type="button" class="settings-pill-action is-quit" id="btn-quit" title="TokenUsage 종료">종료</button>
        </div>
      </div>
    </div>
  `;

  type TabKey = "appearance" | "models" | "general";
  const sheet = root.querySelector("#settings-sheet") as HTMLElement;
  const tabAppearance = root.querySelector("#tab-btn-appearance") as HTMLButtonElement;
  const tabModels = root.querySelector("#tab-btn-models") as HTMLButtonElement;
  const tabGeneral = root.querySelector("#tab-btn-general") as HTMLButtonElement;
  const panelAppearance = root.querySelector("#tab-panel-appearance") as HTMLElement;
  const panelModels = root.querySelector("#tab-panel-models") as HTMLElement;
  const panelGeneral = root.querySelector("#tab-panel-general") as HTMLElement;

  function switchTab(target: TabKey) {
    const list: Array<[TabKey, HTMLButtonElement, HTMLElement]> = [
      ["appearance", tabAppearance, panelAppearance],
      ["models", tabModels, panelModels],
      ["general", tabGeneral, panelGeneral],
    ];
    for (const [key, btn, panel] of list) {
      const active = key === target;
      btn.classList.toggle("is-active", active);
      btn.setAttribute("aria-selected", active ? "true" : "false");
      btn.setAttribute("tabindex", active ? "0" : "-1");
      panel.hidden = !active;
    }
  }

  tabAppearance.addEventListener("click", () => switchTab("appearance"));
  tabModels.addEventListener("click", () => switchTab("models"));
  tabGeneral.addEventListener("click", () => switchTab("general"));

  tabAppearance.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") {
      e.preventDefault();
      tabModels.focus();
      switchTab("models");
    }
  });

  tabModels.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      tabAppearance.focus();
      switchTab("appearance");
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      tabGeneral.focus();
      switchTab("general");
    }
  });

  tabGeneral.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      tabModels.focus();
      switchTab("models");
    }
  });

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

  const showPeriod = root.querySelector("#show-period") as HTMLInputElement | null;
  if (showPeriod) {
    showPeriod.checked = settings.show_period !== false;
    showPeriod.addEventListener("change", () => {
      handlers.onShowPeriod?.(showPeriod.checked);
    });
  }

  const showOrbit = root.querySelector("#show-orbit") as HTMLInputElement | null;
  if (showOrbit) {
    showOrbit.checked = settings.show_orbit !== false;
    showOrbit.addEventListener("change", () => {
      handlers.onShowOrbit?.(showOrbit.checked);
    });
  }

  const showIconGlow = root.querySelector("#show-icon-glow") as HTMLInputElement | null;
  if (showIconGlow) {
    showIconGlow.checked = settings.show_icon_glow !== false;
    showIconGlow.addEventListener("change", () => {
      handlers.onShowIconGlow?.(showIconGlow.checked);
    });
  }

  function paintUpdateUi(): void {
    const btn = root.querySelector<HTMLButtonElement>("#btn-check-update");
    const status = root.querySelector<HTMLElement>("#update-status");
    if (!btn || !status) return;

    btn.disabled = updateBusy;
    btn.classList.toggle("busy", updateBusy);
    btn.classList.toggle("update-available", updatePhase !== "idle");
    btn.classList.toggle("update-downloading", updatePhase === "downloading");
    btn.classList.toggle("update-ready", updatePhase === "ready");

    let title = "업데이트 확인";
    if (updatePhase === "ready" && updateVersion) {
      title = `${updateVersion} 설치를 위해 재시작`;
      status.textContent = updateHint || `${updateVersion} 준비 완료`;
    } else if (updatePhase === "downloading" && updateVersion) {
      title = updateHint || `${updateVersion} 다운로드 중…`;
      status.textContent = title;
    } else if (updateBusy) {
      title = "업데이트 확인 중…";
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
      updateHint = "다운로드 중…";
      paintUpdateUi();
      return;
    }

    updateBusy = true;
    if (phaseAtClick === "ready") {
      updateHint = version ? `${version} 설치를 위해 재시작…` : "재시작 중…";
    } else {
      updateHint = "확인 중…";
    }
    paintUpdateUi();

    try {
      const hasUpdate = await invoke<boolean>("check_for_updates");
      if (hasUpdate) {
        updateBusy = false;
        if (updatePhase === "idle") updatePhase = "downloading";
        updateHint = "새 업데이트 다운로드 중…";
        paintUpdateUi();
        return;
      }
      updatePhase = "idle";
      updateVersion = null;
      updateBusy = false;
      updateHint = "최신 버전입니다";
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
    return "업데이트 확인 실패";
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
    updateHint = `${ev.payload.version} 다운로드 중…`;
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
      updateHint = `${p.version} 다운로드 중…`;
    }
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<UpdateInfo>("update-ready", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "ready";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `${ev.payload.version} 준비 완료`;
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
    const msg = typeof ev.payload === "string" ? ev.payload : "업데이트 실패";
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
        btn.title = `${label} (필수)`;
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
  diagBtn?.addEventListener("click", () => {
    void Promise.resolve(handlers.onDiagnostics()).then(() => {
      if (!diagBtn) return;
      diagBtn.classList.add("is-done");
      diagBtn.setAttribute("aria-label", "진단 로그 복사됨");
      window.setTimeout(() => {
        if (!diagBtn.isConnected) return;
        diagBtn.classList.remove("is-done");
        diagBtn.setAttribute("aria-label", "진단 로그 복사");
      }, 1500);
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
