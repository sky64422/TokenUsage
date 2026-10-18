import type { TokenCounts, TokenPeriod, TokenUsageSnapshot } from "./types";
import { tokenTotal, MODEL_MISSING } from "./format";

const MISSING = "—";
const STATUS = { loading: "집계 중", ready: "", partial: "일부 기록", unavailable: "읽기 실패" };
const integerFormat = new Intl.NumberFormat("ko-KR", { maximumFractionDigits: 0 });
const compactFormat = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 });
type TokenPeriodId = "today" | "week" | "month";
const isCount = (n: number | null): n is number => n !== null && Number.isSafeInteger(n) && n >= 0;

export function cacheReadPercent(c: TokenCounts): number | null {
  return isCount(c.input) && c.input > 0 && isCount(c.cache_read) && c.cache_read <= c.input
    ? c.cache_read / c.input * 100 : null;
}
export function modelShare(c: TokenCounts, totals: TokenCounts): number | null {
  const value = tokenTotal(c), total = tokenTotal(totals);
  return value !== null && total !== null && total > 0 && value <= total ? value / total * 100 : null;
}
export function formatTokens(value: number | null): string {
  return isCount(value) ? (value < 10_000 ? integerFormat.format(value) : compactFormat.format(value)) : MISSING;
}
function percent(value: number | null): string { return value === null ? MISSING : `${value.toFixed(1).replace(/\.0$/, "")}%`; }

export function mountTokenDetails(root: HTMLElement, onResize: () => void): { update(snapshot: TokenUsageSnapshot | null): void } {
  root.classList.add("token-details-root");
  root.hidden = true;
  root.innerHTML = `<details class="token-details"><summary>토큰 상세<span class="token-chevron" aria-hidden="true"></span></summary>
    <div class="token-content"><div class="token-periods settings-tabs" role="tablist" aria-label="토큰 집계 기간">
      <button type="button" class="settings-tab-btn is-active" id="token-tab-today" role="tab" aria-controls="codex-token-period-panel" data-period="today" aria-selected="true">오늘</button>
      <button type="button" class="settings-tab-btn" id="token-tab-week" role="tab" aria-controls="codex-token-period-panel" data-period="week" aria-selected="false" tabindex="-1">이번 주</button>
      <button type="button" class="settings-tab-btn" id="token-tab-month" role="tab" aria-controls="codex-token-period-panel" data-period="month" aria-selected="false" tabindex="-1">이번 달</button>
    </div><div id="codex-token-period-panel" role="tabpanel" aria-labelledby="token-tab-today"><div class="token-provenance"><span>이 기기의 기록</span><time></time></div>
    <p class="token-status" role="status"></p><div class="token-totals"></div><div class="token-models"></div>
    <p class="token-note">캐시 입력은 입력에, 추론은 출력에 포함됩니다.</p></div></div></details>`;
  const details = root.querySelector<HTMLDetailsElement>("details")!;
  const rows = root.querySelector<HTMLElement>(".token-models")!;
  const totals = root.querySelector<HTMLElement>(".token-totals")!;
  const status = root.querySelector<HTMLElement>(".token-status")!;
  const time = root.querySelector<HTMLTimeElement>("time")!;
  let period: TokenPeriodId = "today";
  let snapshot: TokenUsageSnapshot | null = null;
  let rowKey = "";
  const buttons = [...root.querySelectorAll<HTMLButtonElement>("[data-period]")];
  const tabs = root.querySelector<HTMLElement>(".token-periods")!;
  const panel = root.querySelector<HTMLElement>("[role=tabpanel]")!;
  details.addEventListener("toggle", onResize);
  for (const button of buttons) button.addEventListener("click", () => {
    period = button.dataset.period as TokenPeriodId;
    for (const b of buttons) {
      const selected = b === button;
      b.setAttribute("aria-selected", String(selected));
      b.classList.toggle("is-active", selected); b.tabIndex = selected ? 0 : -1;
    }
    tabs.style.setProperty("--active-tab", String(buttons.indexOf(button)));
    panel.setAttribute("aria-labelledby", button.id);
    render();
  });
  tabs.addEventListener("keydown", event => {
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (index < 0) return;
    const next = event.key === "ArrowRight" ? (index + 1) % buttons.length
      : event.key === "ArrowLeft" ? (index + buttons.length - 1) % buttons.length
      : event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : null;
    if (next === null) return;
    event.preventDefault(); buttons[next].focus(); buttons[next].click();
  });

  function render(): void {
    root.hidden = snapshot === null;
    if (!snapshot) return;
    const data = snapshot[period];
    const date = new Date(snapshot.as_of);
    time.textContent = Number.isNaN(date.getTime()) ? "" : date.toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit", hour12: false });
    time.dateTime = snapshot.as_of;
    status.textContent = STATUS[snapshot.status] || (data.models.length === 0 ? "기록 없음" : "");
    status.hidden = !status.textContent;
    const nextKey = JSON.stringify([data, period, snapshot.status]);
    if (nextKey === rowKey) return;
    rowKey = nextKey;
    rows.replaceChildren(); totals.replaceChildren();
    if (data.models.length > 0) {
      const summary = document.createElement("div"); summary.className = "token-summary";
      summary.append(metric("총 토큰", tokenTotal(data.totals)), metric("CacheRead 비중", cacheReadPercent(data.totals), true));
      totals.append(summary);
      for (const model of data.models) rows.append(modelRow(model.model, model.counts, data));
    }
    onResize();
  }
  return { update(next) { snapshot = next; render(); } };
}

function metric(label: string, value: number | null, ratio = false): HTMLElement {
  const el = document.createElement("div"); el.className = "token-metric";
  const title = document.createElement("span"); title.textContent = label;
  const amount = document.createElement("strong"); amount.textContent = ratio ? percent(value) : formatTokens(value);
  if (!ratio && isCount(value)) amount.title = `${integerFormat.format(value)} tokens`;
  el.append(title, amount); return el;
}

function modelRow(name: string | null, counts: TokenCounts, data: TokenPeriod): HTMLElement {
  const row = document.createElement("section"); row.className = "token-model";
  const heading = document.createElement("div"); heading.className = "token-model-heading";
  const label = document.createElement("span"); label.textContent = name || MODEL_MISSING; label.title = label.textContent;
  const share = document.createElement("span"); share.className = "token-share"; share.textContent = percent(modelShare(counts, data.totals));
  share.setAttribute("aria-label", `모델별 토큰 비중 ${share.textContent}`);
  heading.append(label, share); row.append(heading);
  const grid = document.createElement("div"); grid.className = "token-metrics";
  grid.append(metric("입력", counts.input), metric("출력", counts.output), metric("CacheRead", counts.cache_read),
    metric("CacheWrite", counts.cache_write), metric("캐시 비중", cacheReadPercent(counts), true), metric("추론", counts.reasoning));
  row.append(grid); return row;
}
