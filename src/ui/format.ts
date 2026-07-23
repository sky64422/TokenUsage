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

/** Short countdown only: "2h 14m" / "4d" / "soon" / "" */
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

/** One calm subtitle under the bar. */
export function formatSubline(opts: {
  resetsAt: string | null | undefined;
  idle: boolean;
  over: boolean;
  now?: number;
}): string {
  if (opts.idle) return "Idle";
  if (opts.over) return "Over limit";
  const c = formatCountdown(opts.resetsAt, opts.now);
  if (!c) return "";
  if (c === "soon") return "Resets soon";
  return `Resets in ${c}`;
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

// Kept for settings / legacy call sites
export function formatResetsIn(iso: string | null | undefined, now = Date.now()): string {
  const c = formatCountdown(iso, now);
  if (!c) return "";
  if (c === "soon") return "resets soon";
  return `resets in ${c}`;
}

export function formatResetClock(_iso: string | null | undefined): string {
  return "";
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${Math.round(n)}`;
}
