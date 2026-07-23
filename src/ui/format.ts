/** Format remaining time until ISO reset, e.g. "resets in 2h 14m". */
export function formatResetsIn(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return "reset unknown";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "reset unknown";
  let ms = t - now;
  if (ms <= 0) return "resets soon";
  const totalMin = Math.floor(ms / 60000);
  const days = Math.floor(totalMin / (60 * 24));
  const hours = Math.floor((totalMin % (60 * 24)) / 60);
  const mins = totalMin % 60;
  if (days > 0) return `resets in ${days}d ${hours}h`;
  if (hours > 0) return `resets in ${hours}h ${mins}m`;
  return `resets in ${mins}m`;
}

export function formatResetClock(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function levelClass(pct: number | null | undefined): string {
  if (pct == null || Number.isNaN(pct)) return "level-na";
  if (pct >= 90) return "level-critical";
  if (pct >= 70) return "level-warn";
  return "level-ok";
}

export function formatPct(pct: number | null | undefined): string {
  if (pct == null || Number.isNaN(pct)) return "—";
  return `${Math.round(pct)}%`;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${Math.round(n)}`;
}
