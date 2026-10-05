pub struct RefreshPolicy;
impl RefreshPolicy {
    pub const TICK_SECS: u64 = 1;
    pub const DEFAULT_REFRESH_SECS: u64 = 5;
}

pub struct OpacityPolicy;
impl OpacityPolicy {
    pub const MIN: f64 = 0.35;
    pub const MAX: f64 = 1.0;
    /// Readable glass without feeling heavy.
    pub const DEFAULT: f64 = 0.92;
}

pub fn clamp_opacity(v: f64) -> f64 {
    v.clamp(OpacityPolicy::MIN, OpacityPolicy::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_opacity() {
        assert!((clamp_opacity(0.1) - OpacityPolicy::MIN).abs() < 0.001);
        assert!((clamp_opacity(2.0) - OpacityPolicy::MAX).abs() < 0.001);
    }
}
