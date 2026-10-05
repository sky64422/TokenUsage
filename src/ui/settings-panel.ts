import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  meterFillPct,
  opacityStepIndex,
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

import { PROVIDER_CATALOG } from "./provider-catalog";
import {
  createUpdateController,
  UPDATE_CHECKING,
  UPDATE_DOWNLOADING,
  UPDATE_FAILED,
  UPDATE_READY,
  UPDATE_RESTARTING,
  type UpdateState,
} from "./update-controller";
export type { UpdatePhase, UpdateInfo, DownloadProgress } from "./update-controller";

export const SETTINGS_PANEL_HEIGHT = 362;

const PROVIDER_VISIBLE = "표시";
const PROVIDER_HIDDEN = "숨김";
const PROVIDER_MINIMUM = "최소 1개 서비스를 표시해야 합니다";
const SAVE_FAILED = "저장하지 못했습니다. 다시 시도해 주세요.";
const SAVE_RESTORED = "저장 실패 · 이전 설정으로 복원됨";
const UPDATE_CHECK = "업데이트 확인";
const UPDATE_RESTART = "재시작하여 적용";
const DIAG_IDLE = "로그 복사";
const DIAG_LOADING = "복사 중…";
const DIAG_DONE = "복사됨";
const DIAG_ERROR = "복사 실패";
const DIAG_FEEDBACK_MS = 1500;

export function mountSettingsPanel(
  root: HTMLElement,
  settings: AppSettings,
  handlers: {
    onAutostart: (v: boolean) => void | Promise<void>;
    onHoverDetail: (v: boolean) => void | Promise<void>;
    onAlwaysShowNotch?: (v: boolean) => void | Promise<void>;
    onShowAnimation?: (v: boolean) => void | Promise<void>;
    onOpacityChange: (o: number) => void;
    onProviderEnabled: (id: ProviderId, enabled: boolean) => void | Promise<void>;
    onClose?: () => void;
    onDiagnostics: () => void | Promise<void>;
    onQuit: () => void;
    onTabChange?: () => void;
  },
  appVersion = "",
): {
  show: () => void;
  hide: () => void;
  isVisible: () => boolean;
  syncProviderEnabled: (st: AppSettings) => void;
  destroy: () => void;
  getContentHeight: () => number;
} {
  let visible = false;
  const versionStr = appVersion ? `v${appVersion.replace(/^v/i, "")}` : "";
  const initialPct = opacityToPct(settings.opacity);

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
          aria-selected="true" aria-controls="tab-panel-appearance">
          <svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M2 5h3m3 0h6M2 11h6m3 0h3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            <circle cx="6.5" cy="5" r="1.5" stroke="currentColor" stroke-width="1.5"/>
            <circle cx="9.5" cy="11" r="1.5" stroke="currentColor" stroke-width="1.5"/>
          </svg>
          <span class="tab-label">모양</span>
        </button>
        <button type="button" class="settings-tab-btn" id="tab-btn-models" role="tab"
          aria-selected="false" aria-controls="tab-panel-models">
          <svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M8 2.5L2 6l6 3.5L14 6 8 2.5z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
            <path d="M2 9.5l6 3.5 6-3.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span class="tab-label">서비스</span>
        </button>
        <button type="button" class="settings-tab-btn" id="tab-btn-general" role="tab"
          aria-selected="false" aria-controls="tab-panel-general">
          <svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="2.5" stroke="currentColor" stroke-width="1.5"/>
            <path d="M8 1.5v1.8m0 9.4v1.8m6.5-6.5h-1.8m-9.4 0H1.5m11.1-4.6l-1.3 1.3m-6.6 6.6l-1.3 1.3m9.2 0l-1.3-1.3m-6.6-6.6l-1.3-1.3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
          <span class="tab-label">일반</span>
        </button>
      </div>

      <div class="settings-scroll">
      <div class="settings-tab-panel" id="tab-panel-appearance" role="tabpanel" aria-labelledby="tab-btn-appearance">
        <div class="settings-section">
          <span class="settings-label">불투명도</span>
          <div class="settings-card opacity-card">
            <div class="opacity-meter" style="--opacity-fill: ${meterFillPct(initialPct)}%">
              <div class="opacity-meter-fill" aria-hidden="true"></div>
              <div class="opacity-meter-ticks" aria-hidden="true">${opacityTicksHtml()}</div>
              <input type="range" id="opacity-range" class="opacity-meter-input"
                min="${OPACITY_MIN_PCT}" max="${OPACITY_MAX_PCT}" step="${OPACITY_STEP_PCT}"
                value="${initialPct}" aria-label="불투명도" />
            </div>
            <span class="opacity-value" id="opacity-val">${initialPct}%</span>
          </div>
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
              <span class="settings-group-title">마우스 올릴 때 상세 열기</span>
              <input type="checkbox" id="hover-detail" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>

          </div>
        </div>

        <div class="settings-section">
          <span class="settings-label">애니메이션</span>
          <div class="settings-group">
            <label class="settings-group-row" for="show-animation" title="작업 진행 중 궤도 회전 및 아이콘 발광 효과를 표시합니다">
              <span class="settings-group-title">작업 중 강조 효과</span>
              <input type="checkbox" id="show-animation" class="settings-switch-input" />
              <span class="settings-switch" aria-hidden="true"></span>
            </label>
          </div>
        </div>
      </div>

      <div class="settings-tab-panel" id="tab-panel-models" role="tabpanel" aria-labelledby="tab-btn-models" hidden>
        <div class="settings-section">
          <span class="settings-label">표시할 모델</span>
          <div class="provider-grid" role="group" aria-label="표시할 모델">
            ${providerCard("claude", settings.claude.enabled !== false)}
            ${providerCard("codex", settings.codex.enabled !== false)}
            ${providerCard("grok", settings.grok.enabled !== false)}
            ${providerCard("agy", settings.agy.enabled !== false)}
          </div>
        </div>
        <p class="settings-save-status" id="provider-save-status" role="status" aria-live="polite"></p>
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
          </div>
        </div>

        <div class="settings-section">
          <span class="settings-label">앱 정보</span>
          <div class="settings-group">
            <div class="settings-group-row is-static">
              <span class="settings-group-title">버전</span>
              <div class="settings-version-inline">
                <span class="settings-update-status" id="update-status" role="status"></span>
                <button type="button" class="settings-update-btn" id="btn-check-update" title="${UPDATE_CHECK}" aria-label="${UPDATE_CHECK}">
                  <span class="update-icon-wrap" aria-hidden="true">
                    <svg class="update-icon update-icon-reload" width="13" height="13" viewBox="0 0 16 16" fill="none">
                      <path d="M14 8a6 6 0 1 1-1.76-4.24L14 5.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                      <path d="M14 2v3.5h-3.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    <svg class="update-icon update-icon-check" width="13" height="13" viewBox="0 0 16 16" fill="none">
                      <path d="M3.5 8.5L6.5 11.5L12.5 4.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    <svg class="update-icon update-icon-restart" width="13" height="13" viewBox="0 0 16 16" fill="none">
                      <path d="M8 2.5v7m0 0l-3-3m3 3l3-3M3.5 13.5h9" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  </span>
                  <span class="sr-only update-label">${UPDATE_CHECK}</span>
                </button>
                <span class="settings-version-badge" title="${versionStr || "v0.0.0"}">${versionStr || "v0.0.0"}</span>
              </div>
            </div>
          </div>
        </div>

        <div class="settings-action-row is-footer">
          <button type="button" class="settings-pill-action" id="btn-diag" title="문제 해결을 위한 진단 로그 복사" aria-label="진단 로그 복사">
            <span class="diag-icon-wrap" aria-hidden="true">
              <svg class="diag-copy-icon" width="13" height="13" viewBox="0 0 16 16" fill="none">
                <rect x="5" y="5" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
                <path d="M3.5 10.5H3a1.5 1.5 0 0 1-1.5-1.5V3a1.5 1.5 0 0 1 1.5-1.5h6a1.5 1.5 0 0 1 1.5 1.5v0.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
              </svg>
              <svg class="diag-check-icon" width="13" height="13" viewBox="0 0 16 16" fill="none">
                <path d="M3.5 8.5L6.5 11.5L12.5 4.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </span>
            <span class="diag-label-wrap" aria-hidden="true">
              <span class="diag-text diag-text-idle">로그 복사</span>
              <span class="diag-text diag-text-copied">복사됨</span>
            </span>
          </button>
          <button type="button" class="settings-pill-action is-quit" id="btn-quit" title="TokenUsage 종료" aria-label="TokenUsage 종료">
            <span class="quit-progress" aria-hidden="true"></span>
            <span class="quit-icon-wrap" aria-hidden="true">
              <svg class="quit-icon" width="13.5" height="13.5" viewBox="0 0 16 16" fill="none">
                <path d="M8 2v5.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                <path d="M11.8 4.3a5.2 5.2 0 1 1-7.6 0" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
              </svg>
            </span>
            <span class="quit-label-wrap" aria-hidden="true">
              <span class="quit-text quit-text-idle">종료</span>
              <span class="quit-text quit-text-confirm">정말 종료할까요?</span>
            </span>
          </button>
        </div>
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
      btn.tabIndex = active ? 0 : -1;
      panel.hidden = !active;
    }
    root.querySelector<HTMLElement>(".settings-tabs")!.style.setProperty("--active-tab", String(list.findIndex(([key]) => key === target)));
    root.querySelector<HTMLElement>(".settings-scroll")!.scrollTop = 0;
    handlers.onTabChange?.();
  }

  tabAppearance.addEventListener("click", () => switchTab("appearance"));
  tabModels.addEventListener("click", () => switchTab("models"));
  tabGeneral.addEventListener("click", () => switchTab("general"));
  const tabs = [tabAppearance, tabModels, tabGeneral];
  const tabKeys: TabKey[] = ["appearance", "models", "general"];
  tabs.forEach((tab, index) => tab.addEventListener("keydown", (event) => {
    let next: number;
    if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    else return;
    event.preventDefault();
    switchTab(tabKeys[next]);
    tabs[next].focus();
  }));
  switchTab("appearance");

  const opacityRange = root.querySelector("#opacity-range") as HTMLInputElement;
  const opacityVal = root.querySelector("#opacity-val") as HTMLElement;
  const opacityMeter = root.querySelector(".opacity-meter") as HTMLElement;
  const updateBtn = root.querySelector("#btn-check-update") as HTMLButtonElement;

  const paintOpacity = (pct: number) => {
    const snapped = snapOpacityPct(pct);
    const o = pctToOpacity(snapped);
    const step = opacityStepIndex(snapped);
    opacityRange.value = String(snapped);
    opacityVal.textContent = `${snapped}%`;
    opacityMeter.style.setProperty("--opacity-fill", `${meterFillPct(snapped)}%`);
    opacityMeter.dataset.step = String(step);

    const ticks = opacityMeter.querySelectorAll<HTMLElement>(".opacity-tick");
    ticks.forEach((tick, idx) => {
      const on = idx < step;
      tick.classList.toggle("is-on", on);
      tick.classList.toggle("is-current", idx === step - 1 && step > 0);
    });

    handlers.onOpacityChange(o);
  };

  paintOpacity(initialPct);

  opacityRange.addEventListener("input", () => {
    paintOpacity(Number(opacityRange.value));
  });
  opacityRange.addEventListener("change", () => {
    paintOpacity(Number(opacityRange.value));
  });

  function bindToggle(id: string, initial: boolean, save?: (value: boolean) => void | Promise<void>) {
    const input = root.querySelector<HTMLInputElement>(`#${id}`)!;
    input.checked = initial;
    const row = input.closest<HTMLElement>(".settings-group-row")!;
    const status = document.createElement("p");
    status.className = "settings-save-status";
    status.id = `${id}-status`;
    status.setAttribute("role", "status");
    status.hidden = true;
    row.after(status);
    input.setAttribute("aria-describedby", status.id);
    input.disabled = !save;
    input.addEventListener("change", async () => {
      const previous = !input.checked;
      input.disabled = true;
      row.dataset.saveState = "saving";
      row.setAttribute("aria-busy", "true");
      try {
        await save!(input.checked);
        row.dataset.saveState = "saved";
        status.hidden = true;
        status.textContent = "";
      } catch {
        input.checked = previous;
        row.dataset.saveState = "restored";
        status.hidden = false;
        status.textContent = SAVE_RESTORED;
      } finally {
        input.disabled = false;
        row.setAttribute("aria-busy", "false");
      }
    });
  }

  bindToggle("always-show", Boolean(settings.always_show_notch), handlers.onAlwaysShowNotch);
  bindToggle("autostart", settings.autostart, handlers.onAutostart);
  bindToggle("hover-detail", Boolean(settings.hover_detail), handlers.onHoverDetail);
  const initialAnim = settings.show_orbit !== false || settings.show_icon_glow !== false;
  bindToggle("show-animation", initialAnim, async (enabled) => {
    await handlers.onShowAnimation?.(enabled);
  });

  function paintUpdateUi(state: UpdateState): void {
    const { phase: updatePhase, version: updateVersion, hint: updateHint,
      busy: updateBusy, error: updateError, checked: updateChecked } = state;
    const btn = root.querySelector<HTMLButtonElement>("#btn-check-update");
    const status = root.querySelector<HTMLElement>("#update-status");
    if (!btn || !status) return;

    btn.disabled = updateBusy || updatePhase === "downloading";
    btn.classList.toggle("busy", updateBusy);
    btn.classList.toggle("update-available", updatePhase !== "idle");
    btn.classList.toggle("update-downloading", updatePhase === "downloading");
    btn.classList.toggle("update-ready", updatePhase === "ready");
    const feedback = updateBusy || updatePhase === "downloading" ? "loading"
      : updateError ? "error" : updatePhase === "ready" || updateChecked ? "done" : "idle";
    btn.dataset.feedback = feedback;
    btn.setAttribute("aria-busy", String(feedback === "loading"));

    let title = UPDATE_CHECK;
    let statusText = "";
    status.classList.remove("is-error", "is-ready");

    if (updatePhase === "ready" && updateVersion) {
      title = `${updateVersion} 설치를 위해 재시작`;
      statusText = UPDATE_READY;
      status.classList.add("is-ready");
    } else if (updatePhase === "downloading" && updateVersion) {
      title = `${updateVersion} 다운로드 중…`;
      statusText = updateHint || UPDATE_DOWNLOADING;
    } else if (updateBusy) {
      title = "업데이트 확인 중…";
      statusText = updateHint || UPDATE_CHECKING;
    } else if (updateHint) {
      statusText = updateHint;
    }

    if (updateError) {
      statusText = UPDATE_FAILED;
      status.classList.remove("is-ready");
      status.classList.add("is-error");
      status.title = updateError;
    } else {
      status.title = title;
    }
    status.textContent = statusText;

    const labelEl = btn.querySelector(".update-label");
    if (labelEl) {
      labelEl.textContent = updateBusy ? (updatePhase === "ready" ? UPDATE_RESTARTING : UPDATE_CHECKING) : updatePhase === "ready" ? UPDATE_RESTART
        : updatePhase === "downloading" ? UPDATE_DOWNLOADING : UPDATE_CHECK;
    }
    btn.setAttribute("title", title);
    btn.setAttribute("aria-label", title);
  }

  const updater = createUpdateController({
    check: () => invoke<boolean>("check_for_updates"),
    listen,
    schedule: (callback, delay) => {
      const timer = window.setTimeout(callback, delay);
      return () => window.clearTimeout(timer);
    },
    reportError: (message, error) => console.error(message, error),
  });
  updater.subscribe(paintUpdateUi);
  const runUpdateAction = () => { void updater.action(); };
  updateBtn.addEventListener("click", runUpdateAction);

  let providerSaving = false;

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
    if (badge) badge.textContent = on ? PROVIDER_VISIBLE : PROVIDER_HIDDEN;
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
      btn.disabled = locked || providerSaving;
      const badge = btn.querySelector<HTMLElement>(".provider-status-badge");
      if (locked) {
        btn.title = `${label}: ${PROVIDER_MINIMUM}`;
        if (badge) badge.textContent = PROVIDER_VISIBLE;
      } else {
        btn.title = label;
        if (badge) badge.textContent = isProviderOn(id) ? PROVIDER_VISIBLE : PROVIDER_HIDDEN;
      }
    });
  }

  PROVIDER_IDS.forEach((id) => {
    const btn = providerBtn(id);
    btn?.addEventListener("click", async () => {
      if (providerSaving) return;
      const next = !isProviderOn(id);
      if (!next && countEnabled() <= 1) {
        // Keep at least one provider on
        return;
      }
      providerSaving = true;
      const status = root.querySelector<HTMLElement>("#provider-save-status")!;
      status.textContent = "";
      status.classList.remove("is-visible");
      setProviderOn(id, next);
      try {
        await handlers.onProviderEnabled(id, next);
      } catch {
        setProviderOn(id, !next);
        status.textContent = SAVE_FAILED;
        status.classList.add("is-visible");
      } finally {
        providerSaving = false;
        syncProviderLocks();
      }
    });
  });

  syncProviderLocks();

  const closeBtn = root.querySelector<HTMLButtonElement>("#btn-close-settings");
  if (closeBtn && handlers.onClose) {
    closeBtn.addEventListener("click", handlers.onClose);
  }

  const diagBtn = root.querySelector("#btn-diag") as HTMLButtonElement | null;
  let diagTimer: ReturnType<typeof setTimeout> | undefined;
  let destroyed = false;
  function paintDiagnostics(state: "idle" | "loading" | "done" | "error") {
    if (!diagBtn || destroyed) return;
    const label = { idle: DIAG_IDLE, loading: DIAG_LOADING, done: DIAG_DONE, error: DIAG_ERROR }[state];
    diagBtn.dataset.feedback = state;
    diagBtn.disabled = state === "loading";
    diagBtn.classList.toggle("is-done", state === "done");
    diagBtn.setAttribute("aria-busy", String(state === "loading"));
    diagBtn.setAttribute("aria-label", label);
    diagBtn.querySelector<HTMLElement>(".diag-text-idle")!.textContent = label;
  }
  diagBtn?.addEventListener("click", async () => {
    if (diagBtn.disabled) return;
    clearTimeout(diagTimer);
    paintDiagnostics("loading");
    try {
      await handlers.onDiagnostics();
      paintDiagnostics("done");
      if (!destroyed) diagTimer = setTimeout(() => paintDiagnostics("idle"), DIAG_FEEDBACK_MS);
    } catch (error) {
      console.error("Failed to copy diagnostics", error);
      paintDiagnostics("error");
    }
  });
  const btnQuit = root.querySelector("#btn-quit") as HTMLButtonElement | null;
  let quitTimer: ReturnType<typeof setTimeout> | null = null;
  function resetQuit() {
    if (quitTimer) {
      clearTimeout(quitTimer);
      quitTimer = null;
    }
    if (btnQuit) {
      btnQuit.classList.remove("is-confirming");
      btnQuit.setAttribute("title", "TokenUsage 종료");
      btnQuit.setAttribute("aria-label", "TokenUsage 종료");
    }
  }
  btnQuit?.addEventListener("click", () => {
    if (!btnQuit) return;
    if (btnQuit.classList.contains("is-confirming")) {
      resetQuit();
      handlers.onQuit();
    } else {
      btnQuit.classList.add("is-confirming");
      btnQuit.setAttribute("title", "다시 누르면 종료됩니다 (3초 후 취소)");
      btnQuit.setAttribute("aria-label", "다시 누르면 종료됩니다 (3초 후 취소)");
      quitTimer = setTimeout(resetQuit, 3000);
    }
  });

  return {
    show() {
      visible = true;
      sheet.classList.add("visible");
    },
    hide() {
      visible = false;
      resetQuit();
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
        if (badge) badge.textContent = on ? PROVIDER_VISIBLE : PROVIDER_HIDDEN;
      });
      syncProviderLocks();
    },
    destroy() {
      destroyed = true;
      clearTimeout(diagTimer);
      resetQuit();
      updateBtn.removeEventListener("click", runUpdateAction);
      updater.destroy();
    },
    getContentHeight(): number {
      return SETTINGS_PANEL_HEIGHT;
    },
  };
}

function providerCard(id: ProviderId, enabled: boolean): string {
  const { label, mark: markUrl } = PROVIDER_CATALOG[id];
  const state = enabled ? "on" : "off";
  const badgeText = enabled ? PROVIDER_VISIBLE : PROVIDER_HIDDEN;
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
