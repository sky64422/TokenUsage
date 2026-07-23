/** Clamp display percent to 0–100. */
export function clampPct(pct: number | null | undefined): number | null {
  if (pct == null || Number.isNaN(pct)) return null;
  return Math.min(100, Math.max(0, pct));
}

/** True when usage exceeds configured/vendor limit (raw > 100 or message). */
export function isOver(
  pct: number | null | undefined,
  message?: string | null,
  used?: number,
  limit?: number | null,
): boolean {
  if (message && /over\s*limit/i.test(message)) return true;
  if (used != null && limit != null && limit > 0 && used > limit) return true;
  if (pct != null && !Number.isNaN(pct) && pct > 100) return true;
  return false;
}

/**
 * Format remaining time until ISO reset.
 * Prefer countdown; past/near-zero → "resets soon" only when meaningful.
 */
export function formatResetsIn(
  iso: string | null | undefined,
  now = Date.now(),
): string {
  if (!iso) return "";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  const ms = t - now;
  if (ms <= 0) return "resets soon";
  const totalMin = Math.floor(ms / 60000);
  const days = Math.floor(totalMin / (60 * 24));
  const hours = Math.floor((totalMin % (60 * 24)) / 60);
  const mins = totalMin % 60;
  if (days > 0) return `resets in ${days}d ${hours}h`;
  if (hours > 0) return `resets in ${hours}h ${mins}m`;
  if (mins <= 0) return "resets soon";
  return `resets in ${mins}m`;
}

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

/** Primary reset line for a card; idle when no reset / no usage. */
export function formatResetLine(opts: {
  resetsAt: string | null | undefined;
  idle?: boolean;
  hasUsage?: boolean;
  now?: number;
}): string {
  if (opts.idle || (!opts.resetsAt && !opts.hasUsage)) {
    return "idle · no recent usage";
  }
  if (!opts.resetsAt) {
    return opts.hasUsage ? "reset unknown" : "idle · no recent usage";
  }
  const countdown = formatResetsIn(opts.resetsAt, opts.now);
  const clock = formatResetClock(opts.resetsAt);
  if (!countdown) return clock || "reset unknown";
  if (!clock) return countdown;
  return `${countdown} · ${clock}`;
}

export function levelClass(
  pct: number | null | undefined,
  over = false,
): string {
  if (over) return "level-critical level-over";
  if (pct == null || Number.isNaN(pct)) return "level-na";
  const c = clampPct(pct) ?? 0;
  if (c >= 90) return "level-critical";
  if (c >= 70) return "level-warn";
  return "level-ok";
}

export function formatPct(
  pct: number | null | undefined,
  over = false,
): string {
  if (pct == null || Number.isNaN(pct)) return "—";
  const c = clampPct(pct) ?? 0;
  if (over) return "100%+";
  return `${Math.round(c)}%`;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${Math.round(n)}`;
}

/** Compact used/limit; when over, show used / limit with over hint. */
export function formatTokenMeta(
  used: number,
  limit: number | null | undefined,
  over: boolean,
): string {
  if (limit == null) return formatTokens(used);
  const base = `${formatTokens(used)} / ${formatTokens(limit)}`;
  return over ? `${base} over` : base;
}
