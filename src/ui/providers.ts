import {
  clampPct,
  formatCountdown,
  formatPct,
  formatResetClock,
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

  const pct = idle ? 0 : clampPct(s.primary_used_percent);
  const lvl = levelClass(pct, over, idle);

  const src = sourceLabel(s.source, s.message);
  const metaParts = [src.kind];
  if (src.detail) metaParts.push(src.detail);
  if (over) metaParts.push("over");

  const metaHtml = metaParts
    .map((p, i) =>
      i === 0
        ? escapeHtml(p)
        : p === "over"
          ? `<span class="dot">·</span><span class="over-tag">over</span>`
          : `<span class="dot">·</span>${escapeHtml(p)}`,
    )
    .join("");

  const single = s.windows.length === 1;

  const windows = s.windows.length
    ? `<div class="windows ${single ? "windows-single" : "windows-cols"}">${s.windows
        .map((w) => windowCell(w, s.message, idle, single))
        .join("")}</div>`
    : s.status === "unavailable"
      ? `<div class="window-reset">${escapeHtml(s.message ?? "Unavailable")}</div>`
      : "";

  return `
    <div class="provider-card${idle ? " is-idle" : ""}" data-provider="${s.provider_id}">
      <div class="provider-head">
        <div class="provider-head-left">
          <div class="provider-title-line">
            <span class="provider-name">${escapeHtml(s.display_name)}</span>
            <span class="provider-meta">${metaHtml}</span>
          </div>
        </div>
        <div class="provider-pct ${lvl}">${formatPct(pct, over, idle)}</div>
      </div>
      ${windows}
    </div>
  `;
}

function tokenDetail(w: UsageWindow, over: boolean): string | null {
  if (w.limit != null) {
    return formatTokenPair(w.used, w.limit, over);
  }
  return null;
}

function windowCell(
  w: UsageWindow,
  cardMessage: string | null,
  cardIdle: boolean,
  singleWindow: boolean,
): string {
  const over = isOver(w.used_percent, cardMessage, w.used, w.limit);
  const idle =
    cardIdle ||
    ((w.used_percent ?? 0) <= 0 && w.used <= 0 && !w.resets_at);
  const pct = idle ? 0 : clampPct(w.used_percent);
  const lvl = levelClass(pct, over, idle);
  const width = idle ? 0 : (pct ?? 0);
  const showStop = !idle && width > 0 && width < 99;

  let label = (w.label ?? w.kind).replace(/\s*·\s*over$/i, "");
  if (label === "rolling_5h") label = "5h";
  if (label === "weekly") label = "Week";
  if (label === "5-hour") label = "5h";
  if (label === "Weekly") label = "Week";

  const pctText = formatPct(pct, over, idle);
  const detail = tokenDetail(w, over);

  const reset = formatWindowReset({
    resetsAt: w.resets_at,
    idle,
    over,
  });
  const clockLong = formatResetClock(w.resets_at);
  const title = [detail ?? pctText, clockLong || reset]
    .filter(Boolean)
    .join(" · ");
  const urgent =
    over || (!idle && formatCountdown(w.resets_at) === "soon");

  // Dual: always reserve row-% column (even idle "—") so track ends align.
  // Single: % lives in card head only.
  const pctCell = singleWindow
    ? ""
    : `<span class="window-pct ${lvl}">${escapeHtml(pctText)}</span>`;

  // Meta: tokens left, reset right — always render shell for stable height
  const valuesHtml = detail
    ? `<span class="window-values${over ? " over" : ""}">${escapeHtml(detail)}</span>`
    : `<span class="window-values is-empty" aria-hidden="true"></span>`;
  const resetHtml = reset
    ? `<span class="window-reset${urgent ? " urgent" : ""}"
           data-resets-at="${escapeAttr(w.resets_at ?? "")}"
           data-idle="${idle ? "1" : "0"}"
           data-over="${over ? "1" : "0"}">${escapeHtml(reset)}</span>`
    : `<span class="window-reset is-empty" aria-hidden="true"></span>`;

  return `
    <div class="window-cell${idle ? " is-idle" : ""}${singleWindow ? " is-single" : ""}" title="${escapeAttr(title)}">
      <div class="window-row">
        <span class="window-label">${escapeHtml(label)}</span>
        <div class="track" aria-hidden="true">
          <div class="track-fill ${lvl}" style="width:${width}%">
            ${showStop ? `<span class="track-stop"></span>` : ""}
          </div>
        </div>
        ${pctCell}
      </div>
      <div class="window-meta">${valuesHtml}${resetHtml}</div>
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
