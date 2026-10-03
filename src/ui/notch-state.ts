import { clampPct, formatPct, formatWindowLabel, levelClass } from "./format";
import type { ProviderId, ProviderSnapshot } from "./types";

export const CLOSE_DELAY_MS = 180;
export interface NotchState {
  provider: ProviderId | null;
  pinned: boolean;
  settings: boolean;
}
export type NotchEvent =
  | { type: "providers"; ids: ProviderId[] }
  | { type: "hover" | "pin"; id: ProviderId }
  | { type: "leave" | "escape" | "settings" | "reset" };
export const initialNotchState = (): NotchState => ({
  provider: null,
  pinned: false,
  settings: false,
});
export function reduceNotchState(s: NotchState, e: NotchEvent): NotchState {
  switch (e.type) {
    case "providers":
      return s.provider && !e.ids.includes(s.provider)
        ? { ...s, provider: null, pinned: false }
        : s;
    case "hover":
      return s.pinned || s.settings ? s : { ...s, provider: e.id };
    case "pin":
      return {
        ...s,
        settings: false,
        provider: e.id,
        pinned: !(s.pinned && s.provider === e.id),
      };
    case "settings":
      return { ...s, settings: !s.settings };
    case "escape":
      return s.settings ? { ...s, settings: false } : initialNotchState();
    case "leave":
      return s.pinned || s.settings ? s : initialNotchState();
    case "reset":
      return initialNotchState();
  }
}
export function createCloseDelay(close: () => void) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const cancel = () => {
    clearTimeout(timer);
    timer = undefined;
  };
  return {
    cancel,
    schedule: () => {
      cancel();
      timer = setTimeout(close, CLOSE_DELAY_MS);
    },
  };
}
export function headline(s: ProviderSnapshot) {
  const missing = s.status === "auth_required" || s.status === "unavailable";
  const primaryWindow = s.windows[0];
  const pct =
    missing
      ? null
      : primaryWindow && Number.isFinite(primaryWindow.used_percent)
        ? primaryWindow.used_percent
        : Number.isFinite(s.primary_used_percent)
          ? s.primary_used_percent
          : null;
  const matching = primaryWindow ?? s.windows.find((w) => w.used_percent === pct);
  const label =
    pct == null
      ? s.status === "auth_required"
        ? "Sign in"
        : "No data"
      : formatWindowLabel(matching?.label ?? matching?.kind);
  const secondaryRisk = missing
    ? ""
    : s.windows
        .filter(
          (w) =>
            w !== matching &&
            ["level-warn", "level-critical", "level-over"].includes(
              levelClass(w.used_percent),
            ),
        )
        .map(
          (w) =>
            `${formatWindowLabel(w.label ?? w.kind)} ${formatPct(w.used_percent)}`,
        )
        .join(", ");
  return {
    text: formatPct(pct),
    fill: clampPct(pct) ?? 0,
    label,
    level: levelClass(pct, pct != null && pct > 100),
    degraded: s.status !== "ok",
    secondaryRisk,
  };
}
