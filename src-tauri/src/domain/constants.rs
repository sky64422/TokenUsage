use crate::domain::types::WindowGeometry;

pub struct RefreshPolicy;
impl RefreshPolicy {
    pub const TICK_SECS: u64 = 1;
    pub const MIN_REFRESH_SECS: u64 = 10;
    pub const MAX_REFRESH_SECS: u64 = 300;
    pub const DEFAULT_REFRESH_SECS: u64 = 30;
}

pub struct WindowPolicy;
impl WindowPolicy {
    pub const MIN_WIDTH: f64 = 280.0;
    pub const MIN_HEIGHT: f64 = 160.0;
    pub const DEFAULT_WIDTH: f64 = 340.0;
    pub const DEFAULT_HEIGHT: f64 = 420.0;
}

pub struct OpacityPolicy;
impl OpacityPolicy {
    pub const MIN: f64 = 0.35;
    pub const MAX: f64 = 1.0;
    pub const DEFAULT: f64 = 0.92;
}

pub struct HotkeyPolicy;
impl HotkeyPolicy {
    pub const DEFAULT: &'static str = "Ctrl+Shift+U";
}

pub struct UsagePolicy;
impl UsagePolicy {
    /// Claude-style rolling session window.
    pub const FIVE_HOURS_SECS: i64 = 5 * 3600;
    pub const WEEK_SECS: i64 = 7 * 24 * 3600;
}

pub fn clamp_opacity(v: f64) -> f64 {
    v.clamp(OpacityPolicy::MIN, OpacityPolicy::MAX)
}

pub fn clamp_refresh_secs(v: u64) -> u64 {
    v.clamp(
        RefreshPolicy::MIN_REFRESH_SECS,
        RefreshPolicy::MAX_REFRESH_SECS,
    )
}

pub fn clamp_geometry(g: &WindowGeometry) -> WindowGeometry {
    WindowGeometry {
        x: g.x,
        y: g.y,
        width: g.width.max(WindowPolicy::MIN_WIDTH),
        height: g.height.max(WindowPolicy::MIN_HEIGHT),
    }
}
