export type ProviderId = "claude" | "codex" | "grok" | "agy";
export const PROVIDER_IDS: ProviderId[] = ["claude", "codex", "grok", "agy"];

export type WindowKind =
  "rolling_5h" | "weekly" | "daily" | "monthly" | "session" | "unknown";
export type SnapshotStatus =
  "ok" | "degraded" | "unavailable" | "auth_required";
export type DataSource = "vendor" | "unavailable";
export type UsageUnit = "percent";

export interface UsageWindow {
  group?: string | null;
  kind: WindowKind;
  used: number;
  limit: number | null;
  unit: UsageUnit;
  resets_at: string | null;
  used_percent: number | null;
  label: string | null;
}

export interface ProviderSnapshot {
  token_usage?: TokenUsageSnapshot | null;
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

export interface TokenCounts {
  input: number | null;
  cache_read: number | null;
  cache_write: number | null;
  output: number | null;
  reasoning: number | null;
}
export interface TokenPeriod {
  models: { model: string | null; counts: TokenCounts }[];
  totals: TokenCounts;
}
export interface TokenUsageSnapshot {
  source: "local_records";
  as_of: string;
  status: "loading" | "ready" | "partial" | "unavailable";
  today: TokenPeriod;
  week: TokenPeriod;
  month: TokenPeriod;
  windows?: { kind: WindowKind; resets_at: string; group?: string | null; label?: string | null; usage: TokenPeriod }[];
}

export interface ProviderConfig {
  enabled: boolean;
}

export type AppTheme = "dark" | "glass" | "liquid";

export interface AppSettings {
  theme: AppTheme;
  notch: NotchPlacement;
  opacity: number;
  autostart: boolean;
  hover_detail: boolean;
  always_show_notch?: boolean;
  show_orbit?: boolean;
  show_icon_glow?: boolean;
  show_animation?: boolean;
  claude: ProviderConfig;
  codex: ProviderConfig;
  grok: ProviderConfig;
  agy: ProviderConfig;
}

export type NotchEdge = "top" | "right" | "bottom" | "left";
export interface NotchPlacement {
  edge: NotchEdge;
  monitor_hint: string | null;
  offset: number;
}
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type AnchorX = "left" | "right";
export type AnchorY = "top" | "bottom";

export interface NotchLayout {
  metrics: {
    content_scale?: number;
    rest_depth: number;
    rest_length: number;
    depth: number;
    cell: number;
    shoulder: number;
    inner_radius: number;
    inset: number;
    detail_radius: number;
    drag_threshold: number;
  };
  edge: NotchEdge;
  anchor_x: AnchorX;
  anchor_y: AnchorY;
  notch: Rect;
  detail: Rect | null;
  window: Rect;
  scale: number;
  monitor: string;
}
export interface MonitorArea {
  name: string;
  bounds: Rect;
  work: Rect;
  scale: number;
}

export interface PersistedState {
  settings: AppSettings;
  version: number;
}

export interface DiagnosticsSnapshot {
  lines: string[];
}
export interface ProviderActivity {
  provider_id: ProviderId;
  state: "unknown" | "idle" | "running" | "recent";
  observed_at: string | null;
}
