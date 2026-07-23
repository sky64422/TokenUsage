import {
  clampPct,
  formatPct,
  formatResetLine,
  formatResetsIn,
  formatTokenMeta,
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
      root.innerHTML = `<div class="empty-state">No provider data yet.<br/>Tap ↻ after login / tokscale setup.</div>`;
      return;
    }

    root.innerHTML = `<div class="provider-list">${snaps.map(cardHtml).join("")}</div>`;
  }

  setInterval(() => {
    root.querySelectorAll<HTMLElement>("[data-resets-at]").forEach((el) => {
      const iso = el.dataset.resetsAt || null;
      const idle = el.dataset.idle === "1";
      const hasUsage = el.dataset.hasUsage === "1";
      el.textContent = formatResetLine({
        resetsAt: iso,
        idle,
        hasUsage,
      });
    });
  }, 15_000);

  return {
    setSnapshots(next) {
      snaps = next;
      render();
    },
  };
}

function cardHtml(s: ProviderSnapshot): string {
  const hasUsage = s.windows.some(
    (w) => (w.used_percent ?? 0) > 0 || w.used > 0,
  );
  const idle =
    s.message === "idle" ||
    (s.status === "degraded" && !hasUsage) ||
    (s.status === "unavailable" && s.windows.length === 0);

  const over =
    isOver(s.primary_used_percent, s.message) ||
    s.windows.some((w) => isOver(w.used_percent, s.message, w.used, w.limit));

  const pct = clampPct(s.primary_used_percent);
  const lvl = levelClass(pct, over);
  const resetPrimary =
    s.primary_resets_at ??
    s.windows.find((w) => w.resets_at)?.resets_at ??
    null;

  const windows = s.windows.length
    ? s.windows.map((w) => windowHtml(w, s.message)).join("")
    : s.status === "unavailable"
      ? `<div class="provider-idle">${escapeHtml(s.message ?? "unavailable")}</div>`
      : "";

  const resetLine = formatResetLine({
    resetsAt: resetPrimary,
    idle: idle && !hasUsage,
    hasUsage,
  });

  return `
    <div class="provider-card" data-provider="${s.provider_id}">
      <div class="provider-head">
        <div class="provider-name-row">
          <span class="provider-name">${escapeHtml(s.display_name)}</span>
          ${sourceChip(s)}
          ${over ? `<span class="badge-over">over</span>` : ""}
        </div>
        <div class="provider-pct ${lvl}">${formatPct(pct, over)}</div>
      </div>
      <div class="provider-reset"
           data-resets-at="${escapeAttr(resetPrimary ?? "")}"
           data-idle="${idle && !hasUsage ? "1" : "0"}"
           data-has-usage="${hasUsage ? "1" : "0"}">${escapeHtml(resetLine)}</div>
      ${windows}
    </div>
  `;
}

function sourceChip(s: ProviderSnapshot): string {
  const kind = s.source === "tokscale" ? "tokscale" : "local";
  const plan =
    s.source === "tokscale" && s.message && !/over|idle/i.test(s.message)
      ? s.message
      : null;
  const title = plan ? `${kind} · ${plan}` : kind;
  return `<span class="source-chip source-${kind}" title="${escapeAttr(title)}">${escapeHtml(kind)}${plan ? ` · ${escapeHtml(plan)}` : ""}</span>`;
}

function windowHtml(w: UsageWindow, cardMessage: string | null): string {
  const over = isOver(w.used_percent, cardMessage, w.used, w.limit);
  const pct = clampPct(w.used_percent);
  const lvl = levelClass(pct, over);
  const width = pct == null ? 0 : pct;
  let label = w.label ?? w.kind;
  label = label.replace(/\s*·\s*over$/i, "");
  let meta: string;
  if (w.unit === "tokens" && w.limit != null) {
    meta = formatTokenMeta(w.used, w.limit, over);
  } else {
    meta = formatPct(pct, over);
  }
  const resetHint = w.resets_at ? formatResetsIn(w.resets_at) : "";
  return `
    <div class="window-row">
      <div class="window-label">${escapeHtml(label)}</div>
      <div class="bar"><div class="bar-fill ${lvl}" style="width:${width}%"></div></div>
      <div class="window-meta" title="${escapeAttr(resetHint)}">${escapeHtml(meta)}</div>
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
