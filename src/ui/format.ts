import type { TokenCounts, TokenPeriod } from "./types";

export function clampPct(pct: number | null | undefined): number | null {
  if (pct == null || Number.isNaN(pct)) return null;
  return Math.min(100, Math.max(0, pct));
}

/** Compact card copy; callers retain the vendor detail in the title/diagnostics. */
export function formatProviderStatus(status: string, message?: string | null): string {
  if (status === "ok") return "";
  if (status === "auth_required") return "Sign in required";
  if (/^reading\b/i.test(message ?? "")) return "Loading…";
  if (/timed?\s*out|timeout/i.test(message ?? "")) return "Request timed out";
  if (status === "degraded") return "Usage unavailable";
  return "Unable to fetch usage";
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
  // Idle: no label — empty track + percentage tell the story
  if (opts.idle) return "";
  if (opts.over && !opts.resetsAt) return "Over limit";
  if (!opts.resetsAt) return "";
  const when = formatResetClockCompact(opts.resetsAt);
  if (!when) return "";
  const soon = formatCountdown(opts.resetsAt, opts.now) === "soon";
  const body = soon ? `↻ soon · ${when}` : `↻ ${when}`;
  return opts.over ? `Over · ${body}` : body;
}

const QUOTA_LEVEL_MAX = { ok: 50, caution: 75, warn: 90 };

export function levelClass(
  pct: number | null | undefined,
  over = false,
  idle = false,
): string {
  if (idle) return "level-idle";
  if (over) return "level-over";
  if (pct == null || Number.isNaN(pct)) return "level-na";
  const c = clampPct(pct) ?? 0;
  if (c > QUOTA_LEVEL_MAX.warn) return "level-critical";
  if (c > QUOTA_LEVEL_MAX.caution) return "level-warn";
  if (c > QUOTA_LEVEL_MAX.ok) return "level-caution";
  return "level-ok";
}

export function formatPct(
  pct: number | null | undefined,
  over = false,
  idle = false,
): string {
  if (idle && (pct == null || pct === 0)) return "—";
  if (pct == null || Number.isNaN(pct)) return "—";
  if (pct > 100) return `${Math.round(pct)}%`;
  const c = clampPct(pct) ?? 0;
  if (over) return "100%";
  return `${Math.round(c)}%`;
}

export function formatWindowLabel(labelOrKind: string | null | undefined): string {
  if (!labelOrKind) return "";
  const label = labelOrKind.replace(/\s*·\s*over$/i, "").trim();
  if (label === "rolling_5h" || label === "rolling5h") return "5h";
  if (label === "weekly") return "Week";
  if (label === "monthly") return "Month";
  return label;
}



export const MODEL_MISSING = "모델 미제공";
export function formatTokenPercent(value: number | null): string {
  return value === null ? "—" : `${value.toFixed(1).replace(/\.0$/, "")}%`;
}
const DARKEST_MODEL = 48;
const MODEL_LIGHTNESS_RANGE = 32;
const MODEL_LABEL_LIMIT = 24;

function titleWord(word: string): string {
  if (!word) return "";
  if (/^8b$/i.test(word)) return "8B";
  return `${word[0].toUpperCase()}${word.slice(1).toLowerCase()}`;
}

export function formatModelLabel(model: string): string {
  let s = model.trim();
  if (!s) return MODEL_MISSING;

  // 3.8 Flash is always consolidated into "3.8 Flash" regardless of tier (tiered), reasoning effort (high/low/medium), or other suffixes.
  if (/^(?:gemini-)?3\.8-flash(?:[- ].*)?$/i.test(s) || /^3\.8\s+flash(?:[- ].*)?$/i.test(s)) {
    return "3.8 Flash";
  }

  // Reasoning effort suffixes (low, medium, high, xhigh, max) and tier variants are execution settings, not distinct models.
  s = s.replace(/-(?:low|medium|high|xhigh|max)$/i, "");
  s = s.replace(/-tiered$/i, "");

  if (/^claude-/i.test(s)) {
    s = s.replace(/^claude-/i, "").replace(/-(?:latest|preview|\d{8})$/i, "");
    s = s.replace(/^(\d+)-(\d+)-(sonnet|haiku|opus)/i, (_, v1, v2, family) => `${v1}.${v2} ${titleWord(family)}`);
    s = s.replace(/^(\d+)-(sonnet|haiku|opus)/i, (_, v1, family) => `${v1} ${titleWord(family)}`);
    s = s.replace(/^(sonnet|haiku|opus)-(\d+)-(\d+)/i, (_, family, v1, v2) => `${v1}.${v2} ${titleWord(family)}`);
    s = s.replace(/^(sonnet|haiku|opus)-(\d+)/i, (_, family, v1) => `${v1} ${titleWord(family)}`);
  } else if (/^gemini-/i.test(s)) {
    s = s.replace(/^gemini-/i, "").replace(/-(?:latest|preview|exp(?:erimental)?|\d{8})$/i, "");
    s = s.replace(
      /^(\d+(?:\.\d+)?)-(pro|flash|ultra|argon)(?:-(thinking|lite|8b))?/i,
      (_, ver, tier, variant) => {
        const parts = [ver, titleWord(tier)];
        if (variant) parts.push(titleWord(variant));
        return parts.join(" ");
      },
    );
  } else if (/^gpt-/i.test(s)) {
    s = s.replace(/^gpt-/i, "").replace(/-(?:latest|preview|\d{8})$/i, "");
    s = s.replace(/^(\d+(?:\.\d+)?)-(sol|astra|codex)/i, (_, ver, family) => `${ver} ${titleWord(family)}`);
    s = s.replace(/-mini$/i, " Mini");
  }

  s = s.replace(/-/g, " ");

  return s.length > MODEL_LABEL_LIMIT ? `${s.slice(0, MODEL_LABEL_LIMIT - 1)}…` : s;
}
const isCount = (n: number | null): n is number => n !== null && Number.isSafeInteger(n) && n >= 0;

export function tokenTotal(c: TokenCounts): number | null {
  if (!isCount(c.input) || !isCount(c.output)) return null;
  const total = c.input + c.output;
  return Number.isSafeInteger(total) ? total : null;
}
export function modelSegments(data: TokenPeriod): { model: string; label: string; share: number; color: string; lightness: number }[] {
  const total = tokenTotal(data.totals);
  if (total === null || total <= 0) return [];

  // Group by formatted model label so reasoning effort variants (high/medium/low) merge into their parent model.
  const grouped = new Map<string, { model: string; label: string; total: number }>();
  for (const row of data.models) {
    const rowTotal = tokenTotal(row.counts);
    if (rowTotal === null) return [];
    const model = row.model || MODEL_MISSING;
    const label = formatModelLabel(model);
    const existing = grouped.get(label);
    if (existing) {
      existing.total += rowTotal;
    } else {
      const displayModel = label === "3.8 Flash" ? "3.8 Flash" : model;
      grouped.set(label, { model: displayModel, label, total: rowTotal });
    }
  }

  const models = [...grouped.values()];
  if (models.reduce((sum, row) => sum + row.total, 0) !== total) return [];
  models.sort((a, b) => b.total - a.total || a.label.localeCompare(b.label));
  const maximum = models[0]?.total ?? 0;
  return models.map(row => {
    const lightness = DARKEST_MODEL + (1 - row.total / maximum) * MODEL_LIGHTNESS_RANGE;
    return {
      model: row.model,
      label: row.label,
      share: (row.total / total) * 100,
      lightness,
      color: `color-mix(in srgb, var(--model-base-color) ${100 - ((lightness - DARKEST_MODEL) / MODEL_LIGHTNESS_RANGE) * 60}%, white)`,
    };
  });
}
