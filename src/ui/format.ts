export function clampPct(pct: number | null | undefined): number | null {
  if (pct == null || Number.isNaN(pct)) return null;
  return Math.min(100, Math.max(0, pct));
}

export function isOver(
  pct: number | null | undefined,
  message?: string | null,
  used?: number,
  limit?: number | null,
): boolean {
  if (message && /over\s*limit/i.test(message)) return true;
  // Local context-token estimates can exceed plan tables without being "quota over"
  if (message && /estimate\s*\(context/i.test(message)) {
    if (used != null && limit != null && limit > 0 && used > limit) return true;
  }
  if (used != null && limit != null && limit > 0 && used > limit) return true;
  if (pct != null && !Number.isNaN(pct) && pct > 100) return true;
  return false;
}

/** "2h 14m" / "4d" / "soon" */
export function formatCountdown(
  iso: string | null | undefined,
  now = Date.now(),
): string {
  if (!iso) return "";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  const ms = t - now;
  if (ms <= 0) return "soon";
  const totalMin = Math.floor(ms / 60000);
  const days = Math.floor(totalMin / (60 * 24));
  const hours = Math.floor((totalMin % (60 * 24)) / 60);
  const mins = totalMin % 60;
  if (days > 0) return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
  if (hours > 0) return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  if (mins <= 0) return "soon";
  return `${mins}m`;
}

/** Hover / long form: "Jul 31, 3:42 PM" */
export function formatResetClock(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Korean weekday short: 일 월 화 수 목 금 토 */
const KO_WEEKDAYS = ["일", "월", "화", "수", "목", "금", "토"] as const;

/**
 * Compact reset stamp: "7/30 (목) 15:42" (M/D + weekday + 24h local).
 * Avoid locale-long forms like "7월 30일" / "Jul 30".
 */
export function formatResetClockCompact(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const md = `${d.getMonth() + 1}/${d.getDate()}`;
  const dow = KO_WEEKDAYS[d.getDay()] ?? "";
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return dow ? `${md} (${dow}) ${hh}:${mm}` : `${md} ${hh}:${mm}`;
}

/**
 * One-line reset: date/time only ("↻ 7/31 3:42 PM").
 * Countdown removed — absolute clock is enough in the widget.
 */
export function formatWindowReset(opts: {
  resetsAt: string | null | undefined;
  idle: boolean;
  over: boolean;
  now?: number;
}): string {
  // Idle: no label — empty track + %/tokens tell the story
  if (opts.idle) return "";
  if (opts.over && !opts.resetsAt) return "Over limit";
  if (!opts.resetsAt) return "";
  const when = formatResetClockCompact(opts.resetsAt);
  if (!when) return "";
  const soon = formatCountdown(opts.resetsAt, opts.now) === "soon";
  const body = soon ? `↻ soon · ${when}` : `↻ ${when}`;
  return opts.over ? `Over · ${body}` : body;
}

export function levelClass(
  pct: number | null | undefined,
  over = false,
  idle = false,
): string {
  if (idle) return "level-idle";
  if (over) return "level-over";
  if (pct == null || Number.isNaN(pct)) return "level-na";
  const c = clampPct(pct) ?? 0;
  if (c >= 90) return "level-critical";
  if (c >= 70) return "level-warn";
  if (c <= 0) return "level-idle";
  return "level-ok";
}

export function formatPct(
  pct: number | null | undefined,
  over = false,
  idle = false,
): string {
  if (idle && (pct == null || pct === 0)) return "—";
  if (pct == null || Number.isNaN(pct)) return "—";
  const c = clampPct(pct) ?? 0;
  if (over) return "100%";
  return `${Math.round(c)}%`;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${Math.round(n)}`;
}

export function formatTokenPair(
  used: number,
  limit: number | null | undefined,
  _over = false,
): string {
  if (limit == null) return formatTokens(used);
  // Compact "used / limit" only — over is shown via color / meta tag
  return `${formatTokens(used)} / ${formatTokens(limit)}`;
}

export function formatResetsIn(iso: string | null | undefined, now = Date.now()): string {
  const c = formatCountdown(iso, now);
  if (!c) return "";
  if (c === "soon") return "resets soon";
  return `resets in ${c}`;
}

export function sourceLabel(
  source: string,
  message?: string | null,
): { kind: string; detail: string | null } {
  if (source === "tokscale") {
    const plan =
      message && !/over|idle|estimate/i.test(message) ? message : null;
    return { kind: "tokscale", detail: plan };
  }
  // Don't surface "idle" in chrome — empty usage is enough
  if (message === "idle") return { kind: "local", detail: null };
  if (message && /estimate\s*\(context/i.test(message)) {
    return { kind: "local", detail: "context est." };
  }
  if (message === "over limit") return { kind: "local", detail: "over" };
  return { kind: "local", detail: null };
}
