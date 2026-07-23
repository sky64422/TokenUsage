import {
  clampPct,
  formatCountdown,
  formatPct,
  formatTokenPair,
  formatWindowReset,
  isOver,
  levelClass,
  sourceLabel,
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
    root.innerHTML = `<div class="provider-list">${snaps.map(cardHtml).join("")}</div>`;
  }

  setInterval(() => {
    root.querySelectorAll<HTMLElement>("[data-resets-at]").forEach((el) => {
      el.textContent = formatWindowReset({
        resetsAt: el.dataset.resetsAt || null,
        idle: el.dataset.idle === "1",
        over: el.dataset.over === "1",
      });
      const soon =
        !el.dataset.idle &&
        formatCountdown(el.dataset.resetsAt || null) === "soon";
      el.classList.toggle("urgent", el.dataset.over === "1" || soon);
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
    (!hasUsage &&
      (s.status === "degraded" || s.status === "unavailable"));

  const over =
    !idle &&
    (isOver(s.primary_used_percent, s.message) ||
      s.windows.some((w) =>
        isOver(w.used_percent, s.message, w.used, w.limit),
      ));

  const pct = idle
    ? 0
    : clampPct(s.primary_used_percent);
  const lvl = levelClass(pct, over, idle);

  const src = sourceLabel(s.source, s.message);
  const metaParts = [src.kind];
  if (src.detail) metaParts.push(src.detail);
  if (over) metaParts.push("over");

  const windows = s.windows.length
    ? `<div class="windows">${s.windows.map((w) => windowBlock(w, s.message, idle)).join("")}</div>`
    : s.status === "unavailable"
      ? `<div class="windows"><div class="window-reset">${escapeHtml(s.message ?? "Unavailable")}</div></div>`
      : "";

  return `
    <div class="provider-card" data-provider="${s.provider_id}">
      <div class="provider-head">
        <div class="provider-head-left">
          <div class="provider-name">${escapeHtml(s.display_name)}</div>
          <div class="provider-meta">${metaParts
            .map((p, i) =>
              i === 0
                ? escapeHtml(p)
                : p === "over"
                  ? `<span class="dot">·</span><span class="over-tag">over</span>`
                  : `<span class="dot">·</span>${escapeHtml(p)}`,
            )
            .join("")}</div>
        </div>
        <div class="provider-pct ${lvl}">${formatPct(pct, over, idle)}</div>
      </div>
      ${windows}
    </div>
  `;
}

function windowBlock(
  w: UsageWindow,
  cardMessage: string | null,
  cardIdle: boolean,
): string {
  const over = isOver(w.used_percent, cardMessage, w.used, w.limit);
  const idle =
    cardIdle ||
    ((w.used_percent ?? 0) <= 0 && w.used <= 0 && !w.resets_at);
  const pct = idle ? 0 : clampPct(w.used_percent);
  const lvl = levelClass(pct, over, idle);
  const width = idle ? 0 : (pct ?? 0);

  let label = (w.label ?? w.kind).replace(/\s*·\s*over$/i, "");
  // Human labels
  if (label === "rolling_5h") label = "5-hour";
  if (label === "weekly") label = "Weekly";

  let values: string;
  if (w.unit === "tokens" && w.limit != null) {
    values = formatTokenPair(w.used, w.limit, over);
  } else if (w.unit === "percent") {
    values = formatPct(pct, over, idle);
  } else {
    values = formatPct(pct, over, idle);
  }

  const reset = formatWindowReset({
    resetsAt: w.resets_at,
    idle,
    over,
  });

  return `
    <div class="window-block">
      <div class="window-top">
        <span class="window-label">${escapeHtml(label)}</span>
        <span class="window-values${over ? " over" : ""}">${escapeHtml(values)}</span>
      </div>
      <div class="track" aria-hidden="true">
        <div class="track-fill ${lvl}" style="width:${width}%"></div>
      </div>
      <div class="window-reset${over || formatCountdown(w.resets_at) === "soon" ? " urgent" : ""}"
           data-resets-at="${escapeAttr(w.resets_at ?? "")}"
           data-idle="${idle ? "1" : "0"}"
           data-over="${over ? "1" : "0"}">${escapeHtml(reset)}</div>
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
