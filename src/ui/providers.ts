import {
  clampPct,
  formatCountdown,
  formatPct,
  formatProviderStatus,
  formatResetClock,
  formatWindowLabel,
  formatWindowReset,
  isOver,
  levelClass,
  modelSegments,
} from "./format";
import type { ProviderActivity, ProviderId, ProviderSnapshot, TokenPeriod, UsageWindow } from "./types";
import { PROVIDER_CATALOG } from "./provider-catalog";
import { mountTokenDetails } from "./token-details";

export function mountProviders(root: HTMLElement, onResize: () => void = () => {}): {
  setSnapshots: (snaps: ProviderSnapshot[]) => void;
  setActivities: (acts: ProviderActivity[]) => void;
} {
  let snaps: ProviderSnapshot[] = [];
  const activities = new Map<ProviderId, string>();
  let lastDisplay = "";
  const entries = new Map<ProviderId, { element: HTMLElement; quota: HTMLElement; key: string; tokens: ReturnType<typeof mountTokenDetails> | null }>();
  root.innerHTML = `<div class="empty-state">Waiting for usage…</div><div class="provider-list"></div>`;
  const list = root.querySelector<HTMLElement>(".provider-list")!;
  const empty = root.querySelector<HTMLElement>(".empty-state")!;

  function render(): void {
    // Freshness timestamps are retained in snapshots, but do not alter this card.
    // Countdown text has its own timer and running-state changes remain visible.
    const display = JSON.stringify(snaps.map(({as_of: _asOf, ...snapshot}) =>
      [snapshot, activities.get(snapshot.provider_id) === "running"]));
    if (display === lastDisplay) return;
    lastDisplay = display;
    empty.hidden = snaps.length > 0;
    list.hidden = snaps.length === 0;
    for (const [id, entry] of entries) entry.element.hidden = !snaps.some(s => s.provider_id === id);
    for (const snap of snaps) {
      let entry = entries.get(snap.provider_id);
      if (!entry) {
        const element = document.createElement("div"); element.className = "provider-entry";
        const quota = document.createElement("div"); element.append(quota); list.append(element);
        let tokens: ReturnType<typeof mountTokenDetails> | null = null;
        if (snap.provider_id === "codex") {
          const tokenRoot = document.createElement("div"); element.append(tokenRoot);
          tokens = mountTokenDetails(tokenRoot, onResize);
        }
        entry = { element, quota, key: "", tokens }; entries.set(snap.provider_id, entry);
      }
      entry.element.hidden = false;
      const { token_usage: _tokens, as_of: _asOf, ...quota } = snap;
      const running = activities.get(snap.provider_id) === "running";
      const key = JSON.stringify([quota, running, snap.token_usage?.windows, snap.token_usage?.status]);
      if (entry.key !== key) {
        const legends = [...entry.quota.querySelectorAll<HTMLElement>(".token-model-legend")];
        const focused = legends.indexOf(document.activeElement as HTMLElement);
        const scrolls = legends.map(el => el.scrollLeft);
        entry.quota.innerHTML = cardHtml(snap, running); entry.key = key;
        entry.quota.querySelectorAll<HTMLElement>(".token-model-legend").forEach((el, index) => {
          el.scrollLeft = scrolls[index] ?? 0;
          if (index === focused) el.focus({ preventScroll: true });
        });
      }
      entry.tokens?.update(snap.token_usage ?? null);
    }
  }

  setInterval(() => {
    root.querySelectorAll<HTMLElement>("[data-resets-at]").forEach((el) => {
      const resetsAt = el.dataset.resetsAt || null;
      const idle = el.dataset.idle === "1";
      const over = el.dataset.over === "1";
      el.textContent = formatWindowReset({ resetsAt, idle, over });
      const soon = !idle && formatCountdown(resetsAt) === "soon";
      el.classList.toggle("urgent", over || soon);
    });
  }, 15_000);

  return {
    setSnapshots(next) {
      snaps = next;
      render();
    },
    setActivities(acts) {
      activities.clear();
      for (const a of acts) activities.set(a.provider_id, a.state);
      render();
    },
  };
}

function cardHtml(s: ProviderSnapshot, isRunning: boolean): string {
  const hasUsage = s.windows.some(
    (w) => (w.used_percent ?? 0) > 0 || w.used > 0,
  );
  const idle =
    s.message === "idle" ||
    (!hasUsage &&
      !s.windows.some((w) => w.used_percent == null && w.resets_at) &&
      (s.status === "degraded" || s.status === "unavailable"));

  const over =
    !idle &&
    (isOver(s.primary_used_percent, s.message) ||
      s.windows.some((w) =>
        isOver(w.used_percent, s.message, w.used, w.limit),
      ));

  const formatGroup = (g?: string | null) => (g === "Claude/GPT" || g === "Claude and GPT" ? "Claude" : (g || ""));
  const rows = s.windows.length
    ? s.windows
        .map((w, i) => {
          const curGroup = formatGroup(w.group);
          const prevGroup = formatGroup(s.windows[i - 1]?.group);
          const isGroupStart = i > 0 && Boolean(curGroup) && prevGroup !== curGroup;
          return usageRow({
            name: curGroup && prevGroup !== curGroup ? curGroup : (i === 0 ? s.display_name : ""),
            nameHidden: i > 0 && (!curGroup || prevGroup === curGroup),
            isGroupStart,
            window: w,
            cardMessage: s.message,
            cardIdle: idle,
            cardRunning: isRunning,
            icon: !s.windows.some(w => w.group) && i === 0 ? s.provider_id : undefined,
            models: s.provider_id === "codex" && s.token_usage?.status !== "unavailable"
              ? s.token_usage?.windows?.find(period => period.kind === w.kind && w.resets_at !== null
                && Date.parse(period.resets_at) === Date.parse(w.resets_at))?.usage : undefined,
          });
        })
        .join("")
    : emptyUsageRow(s, idle, over);

  const activityAttr = isRunning ? ' data-activity="running"' : "";
  return `
    <div class="provider-card${idle ? " is-idle" : ""}" data-provider="${s.provider_id}"${activityAttr}>
      ${s.windows.some(w => w.group) ? `<div class="quota-service-name">${providerMark(s.provider_id)}<span>${escapeHtml(s.display_name)}</span></div>` : ""}
      ${rows}
    </div>
  `;
}

function emptyUsageRow(
  s: ProviderSnapshot,
  idle: boolean,
  over: boolean,
): string {
  const pct = idle ? 0 : clampPct(s.primary_used_percent);
  const lvl = levelClass(pct, over, idle);
  const msg = formatProviderStatus(s.status, s.message);
  return `
    <div class="usage-row${idle ? " is-idle" : ""}">
      <div class="usage-head">
        <span class="provider-name">${providerMark(s.provider_id)}<span>${escapeHtml(s.display_name)}</span></span>
        <span class="window-label"></span>
        <span class="window-reset"></span>
      </div>
      <div class="usage-metrics">
        <span class="usage-msg" title="${escapeAttr(s.message ?? msg)}">${escapeHtml(msg)}</span>
        <span class="provider-pct ${lvl}">${escapeHtml(formatPct(pct, over, idle))}</span>
      </div>
    </div>
  `;
}

function usageRow(opts: {
  name: string;
  nameHidden: boolean;
  isGroupStart?: boolean;
  window: UsageWindow;
  cardMessage: string | null;
  cardIdle: boolean;
  cardRunning?: boolean;
  icon?: ProviderId;
  models?: TokenPeriod;
}): string {
  const w = opts.window;
  const over = isOver(w.used_percent, opts.cardMessage, w.used, w.limit);
  const idle =
    opts.cardIdle ||
    (w.used_percent == null && w.used <= 0 && !w.resets_at);
  const pct = idle ? 0 : clampPct(w.used_percent);
  const lvl = levelClass(pct, over, idle);
  const width = idle ? 0 : (pct ?? 0);
  const showStop = !idle && width > 0 && width < 99;

  const label = formatWindowLabel(w.label ?? w.kind);

  const pctText = formatPct(w.used_percent, over, idle);
  const reset = formatWindowReset({
    resetsAt: w.resets_at,
    idle,
    over,
  });
  const clockLong = formatResetClock(w.resets_at);
  const title = [opts.name, pctText, clockLong || reset]
    .filter(Boolean)
    .join(" · ");
  const urgent =
    over || (!idle && formatCountdown(w.resets_at) === "soon");
  const groupClass = opts.isGroupStart ? " is-group-start" : "";
  const segments = width > 0 && pct !== null && !idle && opts.models ? modelSegments(opts.models) : [];
  const modelDescription = `${label} 기간의 이 기기 입력+출력 토큰 비중입니다. 모델별 한도 소모율이 아닙니다.`;

  return `
    <div class="usage-row ${lvl}${idle ? " is-idle" : ""}${groupClass}" title="${escapeAttr(title)}">
      <div class="usage-head">
        <span class="provider-name"${opts.nameHidden ? ' aria-hidden="true"' : ""}>${opts.icon ? providerMark(opts.icon) : ""}<span>${escapeHtml(opts.name)}</span></span>
        <span class="window-label">${escapeHtml(label)}</span>
        <span class="window-reset${urgent ? " urgent" : ""}"
              data-resets-at="${escapeAttr(w.resets_at ?? "")}"
              data-idle="${idle ? "1" : "0"}"
              data-over="${over ? "1" : "0"}">${escapeHtml(reset)}</span>
      </div>
      <div class="usage-metrics">
        <div class="track" aria-hidden="true">
          <div class="track-fill ${lvl}${width > 0 && !idle ? " is-active" : ""}${opts.cardRunning ? " is-running" : ""}" style="width:${width}%">
            ${segments.length ? `<div class="track-models">${segments.filter(s => s.share > 0).map(s => `<span style="flex-grow:${s.share};background:${s.color}"></span>`).join("")}</div>` : ""}
            ${showStop && !segments.length ? `<span class="track-stop"></span>` : ""}
          </div>
        </div>
        <span class="provider-pct ${lvl}">${escapeHtml(pctText)}</span>
      </div>
      ${segments.length ? `<div class="token-model-legend" tabindex="0" aria-label="${escapeAttr(modelDescription)}" title="${escapeAttr(modelDescription)}">
        ${segments.map(s => `<span class="token-legend-item" title="${escapeAttr(s.model)} · 기기 토큰 ${s.share.toFixed(1)}%"><i class="token-model-dot" style="background:${s.color}" aria-hidden="true"></i><span>${escapeHtml(s.label)}</span></span>`).join("")}</div>` : ""}
    </div>
  `;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function providerMark(id: ProviderId): string {
  return `<img class="provider-detail-mark" src="${escapeAttr(PROVIDER_CATALOG[id].mark)}" alt="" aria-hidden="true" width="14" height="14">`;
}

function escapeAttr(s: string): string {
  return escapeHtml(s);
}
