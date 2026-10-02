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

export type CardTint =
  "none" | "rose" | "peach" | "mint" | "sky" | "lavender" | "lemon";

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
  card_tint?: CardTint;
}

export interface AppSettings {
  notch: NotchPlacement;
  opacity: number;
  hotkey: string;
  autostart: boolean;
  hover_detail: boolean;
  refresh_secs: number;
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
