export type ProviderId = "claude" | "codex" | "grok";
export type ThemeMode = "light" | "dark" | "system";
export type WindowKind = "rolling_5h" | "weekly" | "daily" | "session" | "unknown";
export type SnapshotStatus = "ok" | "degraded" | "unavailable" | "auth_required";
export type DataSource = "local_file" | "cli" | "manual" | "estimate" | "tokscale";
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

export interface ProviderConfig {
  enabled: boolean;
  limits: PlanLimits;
}

export interface AppSettings {
  theme: ThemeMode;
  opacity: number;
  window: WindowGeometry;
  hotkey: string;
  autostart: boolean;
  refresh_secs: number;
  use_tokscale: boolean;
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
