import type { TokenCounts, TokenPeriod, TokenUsageSnapshot } from "./types";
import { tokenTotal, formatTokenPercent } from "./format";

const MISSING = "—";
const integerFormat = new Intl.NumberFormat("ko-KR", { maximumFractionDigits: 0 });
const compactFormat = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 });
const TOKEN_PERIODS = [{ id: "today", label: "오늘" }, { id: "yesterday", label: "어제" }, { id: "month", label: "최근 30일" }] as const;
const EMPTY_PERIOD: TokenPeriod = { models: [], totals: { input: null, output: null, cache_read: null, cache_write: null, reasoning: null } };
const isCount = (n: number | null): n is number => n !== null && Number.isSafeInteger(n) && n >= 0;

export function cacheReadPercent(c: TokenCounts): number | null {
  return isCount(c.input) && c.input > 0 && isCount(c.cache_read) && c.cache_read <= c.input
    ? c.cache_read / c.input * 100 : null;
}
export function formatCachePercent(value: number | null): string {
  const pct = formatTokenPercent(value);
  return pct === MISSING ? pct : `${pct} Hit`;
}

export function formatTokens(value: number | null): string {
  if (!isCount(value)) return MISSING;
  const num = value < 10_000 ? integerFormat.format(value) : compactFormat.format(value);
  return `${num} Tok`;
}

export function mountTokenDetails(root: HTMLElement, onResize: () => void): { update(snapshot: TokenUsageSnapshot | null): void } {
  root.classList.add("token-details-root");
  root.hidden = true;
  root.innerHTML = `<details class="token-details"><summary aria-label="토큰 상세"><span class="token-chevron" aria-hidden="true"></span></summary>
    <div class="token-content">
      <table class="token-overview" aria-label="기간별 토큰 사용량과 캐시 비중"><tbody></tbody></table>
      <div class="token-trend"></div>
    </div></details>`;
  const details = root.querySelector<HTMLDetailsElement>("details")!;
  let snapshot: TokenUsageSnapshot | null = null;
  let overviewKey = "";
  details.addEventListener("toggle", onResize);

  function render(): void {
    const hidden = snapshot === null;
    const visibilityChanged = root.hidden !== hidden;
    root.hidden = hidden;
    if (!snapshot) { if (visibilityChanged) onResize(); return; }
    const nextOverviewKey = JSON.stringify([snapshot.today, snapshot.yesterday, snapshot.month, snapshot.daily, snapshot.as_of.slice(0, 10)]);
    if (nextOverviewKey !== overviewKey) {
      overviewKey = nextOverviewKey;
      const body = root.querySelector(".token-overview tbody")!;
      body.replaceChildren(...TOKEN_PERIODS.map(({id, label}) => {
        const data = snapshot![id] ?? EMPTY_PERIOD;
        const row = document.createElement("tr");
        const heading = document.createElement("th"); heading.scope = "row"; heading.textContent = label;
        const total = tokenTotal(data.totals);
        const value = document.createElement("td");
        const tokens = document.createElement("span"); tokens.className = "token-overview-total"; tokens.textContent = formatTokens(total);
        tokens.setAttribute("aria-label", `총 토큰 ${formatTokens(total)}`);
        if (isCount(total)) tokens.title = `${compactFormat.format(total)} Tok`;
        const separator = document.createElement("span"); separator.className = "token-overview-separator"; separator.textContent = "·"; separator.setAttribute("aria-hidden", "true");
        const cache = document.createElement("span"); cache.className = "token-overview-cache"; cache.textContent = formatCachePercent(cacheReadPercent(data.totals));
        cache.title = "전체 입력 토큰 중 캐시에서 읽은 토큰의 비율";
        value.append(tokens, separator, cache);
        row.append(heading, value); return row;
      }));
      renderTrend(root.querySelector<HTMLElement>(".token-trend")!, snapshot);
      onResize();
    } else if (visibilityChanged) {
      onResize();
    }
  }
  return { update(next) { snapshot = next; render(); } };
}

function renderTrend(root: HTMLElement, snapshot: TokenUsageSnapshot): void {
  root.replaceChildren();
  const label = document.createElement("span"); label.className = "token-trend-label"; label.textContent = "사용 추이"; label.title = "최근 30일 일별 토큰 사용량";
  root.append(label);
  const date = /^(\d{4})-(\d{2})-(\d{2})/.exec(snapshot.as_of);
  if (!date) { const missing = document.createElement("span"); missing.textContent = MISSING; root.append(missing); return; }
  const [, year, month, day] = date;
  const baseDate = new Date(Number(year), Number(month) - 1, Number(day));
  const totals = new Map((snapshot.daily ?? []).map(item => [item.date, tokenTotal(item.totals)]));
  const maximum = Math.max(1, ...[...totals.values()].filter(isCount));
  const chart = document.createElement("div"); chart.className = "token-trend-chart";
  chart.setAttribute("role", "img"); chart.setAttribute("aria-label", "최근 30일 일별 총 토큰 사용량. 빈 자리는 기록 없음입니다.");
  for (let offset = 29; offset >= 0; offset--) {
    const cur = new Date(baseDate.getFullYear(), baseDate.getMonth(), baseDate.getDate() - offset);
    const y = cur.getFullYear();
    const m = String(cur.getMonth() + 1).padStart(2, "0");
    const d = String(cur.getDate()).padStart(2, "0");
    const key = `${y}-${m}-${d}`;

    const total = totals.get(key) ?? null;

    const bar = document.createElement("span"); bar.className = "token-trend-day";
    bar.dataset.state = total === null ? "unknown" : total === 0 ? "zero" : "value";
    const tipText = `${cur.getMonth() + 1}월 ${cur.getDate()}일 · ${total === null ? "기록 없음" : `${compactFormat.format(total)} Tok`}`;
    bar.dataset.tip = tipText;
    bar.setAttribute("aria-label", tipText);
    if (isCount(total) && total > 0) bar.style.setProperty("--day-height", `${total / maximum * 100}%`);
    chart.append(bar);
  }
  root.append(chart);
}
