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
  // Header %: single → one value; dual (5h / Week) → "a% / b%" (same slot as Codex/Grok)
  const headPctHtml = headerPctHtml(s, idle, over);

  const windows = s.windows.length
    ? `<div class="windows ${single ? "windows-single" : "windows-cols"}">${s.windows
        .map((w) => windowCell(w, s.message, idle))
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
        ${headPctHtml}
      </div>
      ${windows}
    </div>
  `;
}

/** Card-head percentage(s). Dual windows → "78% / 12%" with per-leg color. */
function headerPctHtml(
  s: ProviderSnapshot,
  cardIdle: boolean,
  cardOver: boolean,
): string {
  if (s.windows.length === 0) {
    const pct = cardIdle ? 0 : clampPct(s.primary_used_percent);
    const lvl = levelClass(pct, cardOver, cardIdle);
    return `<div class="provider-pct ${lvl}">${escapeHtml(formatPct(pct, cardOver, cardIdle))}</div>`;
  }

  const parts = s.windows.map((w) => {
    const wOver = isOver(w.used_percent, s.message, w.used, w.limit);
    const wIdle =
      cardIdle ||
      ((w.used_percent ?? 0) <= 0 && w.used <= 0 && !w.resets_at);
    const pct = wIdle ? 0 : clampPct(w.used_percent);
    const lvl = levelClass(pct, wOver, wIdle);
    return { lvl, text: formatPct(pct, wOver, wIdle) };
  });

  if (parts.length === 1) {
    return `<div class="provider-pct ${parts[0].lvl}">${escapeHtml(parts[0].text)}</div>`;
  }

  // Dual: 5h% / Week% — each colored independently
  const inner = parts
    .map((p, i) => {
      const seg = `<span class="provider-pct-leg ${p.lvl}">${escapeHtml(p.text)}</span>`;
      return i === 0
        ? seg
        : `<span class="provider-pct-sep" aria-hidden="true">/</span>${seg}`;
    })
    .join("");
  return `<div class="provider-pct provider-pct-dual">${inner}</div>`;
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
  // Absolute used/limit is hover-only; % is in the card header (single or dual).
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

  // Meta: reset only (no used/limit pair under the track)
  const metaHtml = reset
    ? `<div class="window-meta">
        <span class="window-reset${urgent ? " urgent" : ""}"
              data-resets-at="${escapeAttr(w.resets_at ?? "")}"
              data-idle="${idle ? "1" : "0"}"
              data-over="${over ? "1" : "0"}">${escapeHtml(reset)}</span>
      </div>`
    : "";

  // Label | track only — % lives in card head (Codex / dual 5h·Week alike)
  return `
    <div class="window-cell${idle ? " is-idle" : ""}" title="${escapeAttr(title)}">
      <div class="window-row">
        <span class="window-label">${escapeHtml(label)}</span>
        <div class="track" aria-hidden="true">
          <div class="track-fill ${lvl}" style="width:${width}%">
            ${showStop ? `<span class="track-stop"></span>` : ""}
          </div>
        </div>
      </div>
      ${metaHtml}
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
