import { invoke } from "@tauri-apps/api/core";
import {
  clampPct,
  formatCountdown,
  formatPct,
  formatResetClock,
  formatWindowLabel,
  formatWindowReset,
  isOver,
  levelClass,
} from "./format";
import type { CardTint, ProviderId, ProviderSnapshot, UsageWindow } from "./types";
import { CARD_TINTS, PROVIDER_IDS } from "./types";

function normalizeTint(raw: string | null | undefined): CardTint {
  return CARD_TINTS.some((t) => t.value === raw) ? (raw as CardTint) : "none";
}

export function mountProviders(root: HTMLElement): {
  setSnapshots: (snaps: ProviderSnapshot[]) => void;
  setTints: (tints: Partial<Record<ProviderId, CardTint>>) => void;
} {
  let snaps: ProviderSnapshot[] = [];
  const tints: Record<ProviderId, CardTint> = {
    claude: "none",
    codex: "none",
    grok: "none",
    agy: "none",
  };
  let tintMenuEl: HTMLElement | null = null;

  function closeTintMenu(): void {
    if (tintMenuEl) {
      tintMenuEl.remove();
      tintMenuEl = null;
    }
  }

  function openTintMenu(id: ProviderId, clientX: number, clientY: number): void {
    closeTintMenu();
    const current = tints[id];
    const menu = document.createElement("div");
    menu.className = "tint-menu";
    menu.setAttribute("role", "menu");
    menu.innerHTML = `
      <div class="tint-menu-label">Card color</div>
      <div class="tint-swatches">
        ${CARD_TINTS.map(
          (t) => `
          <button type="button" class="tint-swatch tint-${t.value}${t.value === current ? " active" : ""}"
            data-tint="${t.value}" title="${t.label}" aria-label="${t.label}"></button>`,
        ).join("")}
      </div>
    `;
    document.body.appendChild(menu);
    const pad = 8;
    const rect = menu.getBoundingClientRect();
    let left = clientX;
    let top = clientY;
    if (left + rect.width > window.innerWidth - pad) {
      left = window.innerWidth - rect.width - pad;
    }
    if (top + rect.height > window.innerHeight - pad) {
      top = window.innerHeight - rect.height - pad;
    }
    menu.style.left = `${Math.max(pad, left)}px`;
    menu.style.top = `${Math.max(pad, top)}px`;
    tintMenuEl = menu;

    menu.querySelectorAll<HTMLButtonElement>("[data-tint]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        const tint = normalizeTint(btn.dataset.tint);
        closeTintMenu();
        tints[id] = tint;
        applyTintClass(id, tint);
        void invoke("set_provider_tint", { provider: id, tint }).catch((err) => {
          console.error("set_provider_tint failed", err);
        });
      });
    });
  }

  function applyTintClass(id: ProviderId, tint: CardTint): void {
    const el = root.querySelector<HTMLElement>(`[data-provider="${id}"]`);
    if (!el) return;
    for (const t of CARD_TINTS) {
      if (t.value === "none") continue;
      el.classList.toggle(`tint-${t.value}`, t.value === tint);
    }
  }

  function bindTintMenus(): void {
    root.querySelectorAll<HTMLElement>("[data-provider]").forEach((card) => {
      card.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        e.stopPropagation();
        const id = card.dataset.provider as ProviderId | undefined;
        if (!id) return;
        openTintMenu(id, e.clientX, e.clientY);
      });
    });
  }

  function render(): void {
    closeTintMenu();
    if (snaps.length === 0) {
      root.innerHTML = `<div class="empty-state">Waiting for usage…</div>`;
      return;
    }
    root.innerHTML = `<div class="provider-list">${snaps.map((s) => cardHtml(s, tints[s.provider_id])).join("")}</div>`;
    bindTintMenus();
  }

  document.addEventListener("pointerdown", (e) => {
    if (tintMenuEl && !tintMenuEl.contains(e.target as Node)) {
      closeTintMenu();
    }
  });

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
    setTints(next) {
      for (const id of PROVIDER_IDS) {
        if (next[id] != null) tints[id] = normalizeTint(next[id]);
      }
      render();
    },
  };
}

function cardHtml(s: ProviderSnapshot, tint: CardTint): string {
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

  const rows = s.windows.length
    ? s.windows
        .map((w, i) =>
          usageRow({
            name: w.group && s.windows[i-1]?.group !== w.group ? w.group : (i === 0 ? s.display_name : ""),
            nameHidden: i > 0 && (!w.group || s.windows[i-1]?.group === w.group),
            window: w,
            cardMessage: s.message,
            cardIdle: idle,
          }),
        )
        .join("")
    : emptyUsageRow(s, idle, over);

  const tintClass = tint !== "none" ? ` tint-${tint}` : "";
  return `
    <div class="provider-card${idle ? " is-idle" : ""}${tintClass}" data-provider="${s.provider_id}">
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
  const msg = s.message ?? (s.status === "unavailable" ? "Unavailable" : "");
  return `
    <div class="usage-row${idle ? " is-idle" : ""}">
      <div class="usage-head">
        <span class="provider-name">${escapeHtml(s.display_name)}</span>
        <span class="window-label"></span>
        <span class="window-reset"></span>
      </div>
      <div class="usage-metrics">
        <span class="usage-msg">${escapeHtml(msg)}</span>
        <span class="provider-pct ${lvl}">${escapeHtml(formatPct(pct, over, idle))}</span>
      </div>
    </div>
  `;
}

function usageRow(opts: {
  name: string;
  nameHidden: boolean;
  window: UsageWindow;
  cardMessage: string | null;
  cardIdle: boolean;
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

  return `
    <div class="usage-row${idle ? " is-idle" : ""}" title="${escapeAttr(title)}">
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
          <div class="track-fill ${lvl}${width > 0 && !idle ? " is-active" : ""}" style="width:${width}%">
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
