import {
  clampPct,
  formatCountdown,
  formatPct,
  formatSubline,
  isOver,
  levelClass,
} from "./format";
import type { ProviderSnapshot, UsageWindow } from "./types";

export function mountProviders(root: HTMLElement): {
  setSnapshots: (snaps: ProviderSnapshot[]) => void;
} {
  let snaps: ProviderSnapshot[] = [];

  function render(): void {
    if (snaps.length === 0) {
      root.innerHTML = `<div class="empty-state">Waiting for usage…</div>`;
      return;
    }
    root.innerHTML = `<div class="provider-list">${snaps.map(rowHtml).join("")}</div>`;
  }

  setInterval(() => {
    root.querySelectorAll<HTMLElement>("[data-resets-at]").forEach((el) => {
      const idle = el.dataset.idle === "1";
      const over = el.dataset.over === "1";
      el.textContent = formatSubline({
        resetsAt: el.dataset.resetsAt || null,
        idle,
        over,
      });
      el.classList.toggle("urgent", over || (!idle && formatCountdown(el.dataset.resetsAt) === "soon"));
    });
  }, 15_000);

  return {
    setSnapshots(next) {
      snaps = next;
      render();
    },
  };
}

/** Pick the single bar that matters: highest pressure window. */
function primaryWindow(s: ProviderSnapshot): UsageWindow | null {
  if (!s.windows.length) return null;
  return s.windows.reduce((best, w) => {
    const bp = best.used_percent ?? -1;
    const wp = w.used_percent ?? -1;
    return wp >= bp ? w : best;
  });
}

function rowHtml(s: ProviderSnapshot): string {
  const primary = primaryWindow(s);
  const hasUsage =
    s.windows.some((w) => (w.used_percent ?? 0) > 0 || w.used > 0) ||
    (s.primary_used_percent ?? 0) > 0;

  const idle =
    s.message === "idle" ||
    (!hasUsage && (s.status === "degraded" || s.status === "unavailable"));

  const over =
    !idle &&
    (isOver(s.primary_used_percent, s.message) ||
      s.windows.some((w) => isOver(w.used_percent, s.message, w.used, w.limit)));

  const pct = idle ? 0 : clampPct(primary?.used_percent ?? s.primary_used_percent);
  const lvl = levelClass(pct, over, idle);
  const width = idle ? 0 : (pct ?? 0);

  const resetsAt =
    s.primary_resets_at ??
    primary?.resets_at ??
    s.windows.find((w) => w.resets_at)?.resets_at ??
    null;

  const sub = formatSubline({ resetsAt, idle, over });

  return `
    <div class="provider-row" data-provider="${s.provider_id}">
      <div class="provider-top">
        <span class="provider-name">${escapeHtml(s.display_name)}</span>
        <span class="provider-pct ${lvl}">${formatPct(pct, over, idle)}</span>
      </div>
      <div class="track" aria-hidden="true">
        <div class="track-fill ${lvl}" style="width:${width}%"></div>
      </div>
      <div class="provider-sub${over ? " urgent" : ""}"
           data-resets-at="${escapeAttr(resetsAt ?? "")}"
           data-idle="${idle ? "1" : "0"}"
           data-over="${over ? "1" : "0"}">${escapeHtml(sub)}</div>
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
