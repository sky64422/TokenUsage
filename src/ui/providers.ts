import {
  formatPct,
  formatResetsIn,
  formatResetClock,
  formatTokens,
  levelClass,
} from "./format";
import type { ProviderSnapshot, UsageWindow } from "./types";

export function mountProviders(root: HTMLElement): {
  setSnapshots: (snaps: ProviderSnapshot[]) => void;
} {
  let snaps: ProviderSnapshot[] = [];

  function render(): void {
    if (snaps.length === 0) {
      root.innerHTML = `<div class="empty-state">No provider data yet.<br/>Local logs will appear after refresh.</div>`;
      return;
    }

    root.innerHTML = `<div class="provider-list">${snaps.map(cardHtml).join("")}</div>`;
  }

  // live countdown tick
  setInterval(() => {
    root.querySelectorAll<HTMLElement>("[data-resets-at]").forEach((el) => {
      const iso = el.dataset.resetsAt;
      el.textContent = formatResetsIn(iso ?? null);
    });
  }, 30_000);

  return {
    setSnapshots(next) {
      snaps = next;
      render();
    },
  };
}

function cardHtml(s: ProviderSnapshot): string {
  const pct = s.primary_used_percent;
  const lvl = levelClass(pct);
  const resetPrimary = s.primary_resets_at ?? s.windows.find((w) => w.resets_at)?.resets_at ?? null;
  const windows = s.windows.length
    ? s.windows.map(windowHtml).join("")
    : `<div class="provider-msg">${escapeHtml(s.message ?? s.status)}</div>`;

  return `
    <div class="provider-card" data-provider="${s.provider_id}">
      <div class="provider-head">
        <div class="provider-name">${escapeHtml(s.display_name)}</div>
        <div class="provider-pct ${lvl}">${formatPct(pct)}</div>
      </div>
      <div class="provider-reset">
        <strong data-resets-at="${escapeAttr(resetPrimary ?? "")}">${formatResetsIn(resetPrimary)}</strong>
        <span> · ${formatResetClock(resetPrimary)}</span>
      </div>
      ${windows}
      <div class="provider-msg">${escapeHtml(sourceLabel(s.source))}${s.message ? " · " + escapeHtml(s.message) : ""}</div>
    </div>
  `;
}

function sourceLabel(source: ProviderSnapshot["source"]): string {
  switch (source) {
    case "tokscale":
      return "tokscale";
    case "estimate":
      return "local estimate";
    case "local_file":
      return "local file";
    case "cli":
      return "cli";
    default:
      return source;
  }
}

function windowHtml(w: UsageWindow): string {
  const pct = w.used_percent;
  const lvl = levelClass(pct);
  const width = pct == null ? 0 : Math.min(100, Math.max(0, pct));
  const label = w.label ?? w.kind;
  let meta = formatPct(pct);
  if (w.unit === "tokens" && w.limit != null) {
    meta = `${formatTokens(w.used)} / ${formatTokens(w.limit)}`;
  }
  return `
    <div class="window-row">
      <div class="window-label">${escapeHtml(label)}</div>
      <div class="bar"><div class="bar-fill ${lvl}" style="width:${width}%"></div></div>
      <div class="window-meta" title="${escapeAttr(formatResetsIn(w.resets_at))}">${escapeHtml(meta)}</div>
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
