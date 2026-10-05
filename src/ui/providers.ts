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
} from "./format";
import type { ProviderActivity, ProviderId, ProviderSnapshot, UsageWindow } from "./types";

export function mountProviders(root: HTMLElement): {
  setSnapshots: (snaps: ProviderSnapshot[]) => void;
  setActivities: (acts: ProviderActivity[]) => void;
} {
  let snaps: ProviderSnapshot[] = [];
  const activities = new Map<ProviderId, string>();

  function render(): void {
    if (snaps.length === 0) {
      root.innerHTML = `<div class="empty-state">Waiting for usage…</div>`;
      return;
    }
    root.innerHTML = `<div class="provider-list">${snaps.map((s) => cardHtml(s, activities.get(s.provider_id) === "running")).join("")}</div>`;
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
          });
        })
        .join("")
    : emptyUsageRow(s, idle, over);

  const activityAttr = isRunning ? ' data-activity="running"' : "";
  return `
    <div class="provider-card${idle ? " is-idle" : ""}" data-provider="${s.provider_id}"${activityAttr}>
      ${s.windows.some(w => w.group) ? `<div class="quota-service-name">${escapeHtml(s.display_name)}</div>` : ""}
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
        <span class="provider-name">${escapeHtml(s.display_name)}</span>
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

  return `
    <div class="usage-row${idle ? " is-idle" : ""}${groupClass}" title="${escapeAttr(title)}">
      <div class="usage-head">
        <span class="provider-name"${opts.nameHidden ? ' aria-hidden="true"' : ""}>${escapeHtml(opts.name)}</span>
        <span class="window-label">${escapeHtml(label)}</span>
        <span class="window-reset${urgent ? " urgent" : ""}"
              data-resets-at="${escapeAttr(w.resets_at ?? "")}"
              data-idle="${idle ? "1" : "0"}"
              data-over="${over ? "1" : "0"}">${escapeHtml(reset)}</span>
      </div>
      <div class="usage-metrics">
        <div class="track" aria-hidden="true">
          <div class="track-fill ${lvl}${width > 0 && !idle ? " is-active" : ""}${opts.cardRunning ? " is-running" : ""}" style="width:${width}%">
            ${showStop ? `<span class="track-stop"></span>` : ""}
          </div>
        </div>
        <span class="provider-pct ${lvl}">${escapeHtml(pctText)}</span>
      </div>
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

function escapeAttr(s: string): string {
  return escapeHtml(s);
}
