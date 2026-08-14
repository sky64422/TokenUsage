export type ProviderId = "claude" | "codex" | "grok";
export type WindowKind = "rolling_5h" | "weekly" | "daily" | "session" | "unknown";
export type SnapshotStatus = "ok" | "degraded" | "unavailable" | "auth_required";
export type DataSource =
  | "local_file"
  | "cli"
  | "manual"
  | "estimate"
  | "vendor";
export type UsageUnit = "percent" | "tokens" | "messages" | "credits";

export interface UsageWindow {
  kind: WindowKind;
  used: number;
  limit: number | null;
  unit: UsageUnit;
  resets_at: string | null;
  used_percent: number | null;
  label: string | null;
}

export interface ProviderSnapshot {
  provider_id: ProviderId;
  display_name: string;
  windows: UsageWindow[];
  status: SnapshotStatus;
  source: DataSource;
  as_of: string;
  message: string | null;
  primary_resets_at: string | null;
  primary_used_percent: number | null;
}

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PlanLimits {
  five_hour_tokens: number;
  weekly_tokens: number | null;
}

export type CardTint =
  | "none"
  | "rose"
  | "peach"
  | "mint"
  | "sky"
  | "lavender"
  | "lemon";

export const CARD_TINTS: { value: CardTint; label: string }[] = [
  { value: "none", label: "Default" },
  { value: "rose", label: "Rose" },
  { value: "peach", label: "Peach" },
  { value: "mint", label: "Mint" },
  { value: "sky", label: "Sky" },
  { value: "lavender", label: "Lavender" },
  { value: "lemon", label: "Lemon" },
];

export interface ProviderConfig {
  enabled: boolean;
  limits: PlanLimits;
  card_tint?: CardTint;
}

export interface AppSettings {
  opacity: number;
  window: WindowGeometry;
  hotkey: string;
  autostart: boolean;
  refresh_secs: number;
  claude: ProviderConfig;
  codex: ProviderConfig;
  grok: ProviderConfig;
}

export interface PersistedState {
  settings: AppSettings;
  version: number;
}

export interface DiagnosticsSnapshot {
  lines: string[];
}
