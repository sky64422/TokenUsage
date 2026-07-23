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

/** Per-window reset line: countdown · clock, or Idle / — */
export function formatWindowReset(opts: {
  resetsAt: string | null | undefined;
  idle: boolean;
  over: boolean;
  now?: number;
}): string {
  if (opts.idle) return "Idle";
  if (opts.over && !opts.resetsAt) return "Over limit";
  if (!opts.resetsAt) return "—";
  const c = formatCountdown(opts.resetsAt, opts.now);
  const clock = formatResetClock(opts.resetsAt);
  if (!c) return clock || "—";
  if (c === "soon") return clock ? `Resets soon · ${clock}` : "Resets soon";
  if (opts.over) return clock ? `Over · resets in ${c}` : `Over · ${c}`;
  return clock ? `Resets in ${c} · ${clock}` : `Resets in ${c}`;
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
  over: boolean,
): string {
  if (limit == null) return formatTokens(used);
  const base = `${formatTokens(used)} / ${formatTokens(limit)}`;
  return over ? `${base} · over` : base;
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
      message && !/over|idle/i.test(message) ? message : null;
    return { kind: "tokscale", detail: plan };
  }
  if (message === "idle") return { kind: "local", detail: null };
  if (message === "over limit") return { kind: "local", detail: null };
  return { kind: "local", detail: null };
}
