use crate::domain::constants::UsagePolicy;
use crate::domain::types::{
    DataSource, PlanLimits, ProviderId, ProviderSnapshot, SnapshotStatus, UsageUnit, UsageWindow,
    WindowKind,
};
use chrono::{DateTime, Duration, Utc};

/// A single billed usage event (local log-derived).
#[derive(Debug, Clone)]
pub struct UsageEvent {
    pub at: DateTime<Utc>,
    pub tokens: f64,
}

/// Sum tokens whose timestamps fall in `[start, end)`.
pub fn sum_tokens_in_range(events: &[UsageEvent], start: DateTime<Utc>, end: DateTime<Utc>) -> f64 {
    events
        .iter()
        .filter(|e| e.at >= start && e.at < end)
        .map(|e| e.tokens)
        .sum()
}

/// Active 5-hour block: from the earliest event still within the last 5 hours,
/// or "now ??5h" if empty. Reset is that start + 5h.
pub fn active_five_hour_window(
    events: &[UsageEvent],
    now: DateTime<Utc>,
) -> (DateTime<Utc>, DateTime<Utc>, f64) {
    let window_start_floor = now - Duration::seconds(UsagePolicy::FIVE_HOURS_SECS);
    let in_window: Vec<&UsageEvent> = events
        .iter()
        .filter(|e| e.at >= window_start_floor && e.at <= now)
        .collect();
    if in_window.is_empty() {
        let start = window_start_floor;
        let end = start + Duration::seconds(UsagePolicy::FIVE_HOURS_SECS);
        return (start, end, 0.0);
    }
    // Claude-style: window anchored to first activity in the current rolling span.
    let start = in_window
        .iter()
        .map(|e| e.at)
        .min()
        .unwrap_or(window_start_floor);
    let end = start + Duration::seconds(UsagePolicy::FIVE_HOURS_SECS);
    let used = in_window.iter().map(|e| e.tokens).sum();
    (start, end, used)
}

pub fn weekly_window(
    events: &[UsageEvent],
    now: DateTime<Utc>,
) -> (DateTime<Utc>, DateTime<Utc>, f64) {
    let start = now - Duration::seconds(UsagePolicy::WEEK_SECS);
    let end = start + Duration::seconds(UsagePolicy::WEEK_SECS);
    let used = sum_tokens_in_range(events, start, now);
    // Approximate weekly reset as now + remaining until rolling 7d from oldest activity, else now+7d from start
    let resets = now + Duration::seconds(UsagePolicy::WEEK_SECS) - (now - start);
    let _ = resets;
    // Prefer: if we have events, reset when the oldest event in the week ages out
    let resets_at = if let Some(oldest) = events
        .iter()
        .filter(|e| e.at >= start && e.at <= now)
        .map(|e| e.at)
        .min()
    {
        oldest + Duration::seconds(UsagePolicy::WEEK_SECS)
    } else {
        now + Duration::seconds(UsagePolicy::WEEK_SECS)
    };
    (start, resets_at.min(end.max(now)), used)
}

pub fn percent(used: f64, limit: Option<f64>) -> Option<f64> {
    let lim = limit?;
    if lim <= 0.0 {
        return None;
    }
    Some(((used / lim) * 100.0).clamp(0.0, 999.0))
}

pub fn build_snapshot_from_events(
    provider: ProviderId,
    events: Vec<UsageEvent>,
    limits: &PlanLimits,
    now: DateTime<Utc>,
    source: DataSource,
    message: Option<String>,
) -> ProviderSnapshot {
    let (_s5, reset5, used5) = active_five_hour_window(&events, now);
    let (_sw, reset_w, used_w) = weekly_window(&events, now);

    let mut windows = Vec::new();

    let p5 = percent(used5, Some(limits.five_hour_tokens));
    windows.push(UsageWindow {
        kind: WindowKind::Rolling5h,
        used: used5,
        limit: Some(limits.five_hour_tokens),
        unit: UsageUnit::Tokens,
        resets_at: Some(reset5.to_rfc3339()),
        used_percent: p5,
        label: Some("5-hour".into()),
    });

    if let Some(weekly_limit) = limits.weekly_tokens {
        let pw = percent(used_w, Some(weekly_limit));
        windows.push(UsageWindow {
            kind: WindowKind::Weekly,
            used: used_w,
            limit: Some(weekly_limit),
            unit: UsageUnit::Tokens,
            resets_at: Some(reset_w.to_rfc3339()),
            used_percent: pw,
            label: Some("Weekly".into()),
        });
    }

    let primary_used_percent = windows
        .iter()
        .filter_map(|w| w.used_percent)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Prefer nearest future reset among windows that are non-trivial.
    let primary_resets_at = windows
        .iter()
        .filter_map(|w| w.resets_at.as_ref())
        .cloned()
        .min_by(|a, b| a.cmp(b));

    let status = if events.is_empty() {
        SnapshotStatus::Degraded
    } else {
        SnapshotStatus::Ok
    };

    let msg = message.or_else(|| {
        if events.is_empty() {
            Some("No recent local usage logs".into())
        } else {
            Some("Local estimate from session logs".into())
        }
    });

    ProviderSnapshot {
        provider_id: provider,
        display_name: provider.display_name().into(),
        windows,
        status,
        source,
        as_of: now.to_rfc3339(),
        message: msg,
        primary_resets_at,
        primary_used_percent,
    }
}

pub fn unavailable_snapshot(provider: ProviderId, message: impl Into<String>) -> ProviderSnapshot {
    let now = Utc::now();
    ProviderSnapshot {
        provider_id: provider,
        display_name: provider.display_name().into(),
        windows: vec![],
        status: SnapshotStatus::Unavailable,
        source: DataSource::LocalFile,
        as_of: now.to_rfc3339(),
        message: Some(message.into()),
        primary_resets_at: None,
        primary_used_percent: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_hour_sums_only_in_window() {
        let now = Utc::now();
        let events = vec![
            UsageEvent {
                at: now - Duration::hours(6),
                tokens: 1000.0,
            },
            UsageEvent {
                at: now - Duration::hours(1),
                tokens: 500.0,
            },
            UsageEvent {
                at: now - Duration::minutes(10),
                tokens: 250.0,
            },
        ];
        let (_s, _e, used) = active_five_hour_window(&events, now);
        assert!((used - 750.0).abs() < 0.01);
    }

    #[test]
    fn percent_clamps() {
        assert_eq!(percent(50.0, Some(100.0)), Some(50.0));
        assert_eq!(percent(10.0, None), None);
        assert_eq!(percent(10.0, Some(0.0)), None);
    }

    #[test]
    fn build_snapshot_empty_is_degraded() {
        let limits = PlanLimits {
            five_hour_tokens: 1000.0,
            weekly_tokens: Some(5000.0),
        };
        let snap = build_snapshot_from_events(
            ProviderId::Claude,
            vec![],
            &limits,
            Utc::now(),
            DataSource::Estimate,
            None,
        );
        assert_eq!(snap.status, SnapshotStatus::Degraded);
        assert_eq!(snap.windows.len(), 2);
        assert!(snap.primary_resets_at.is_some());
    }

    #[test]
    fn unavailable_snapshot_shape() {
        let snap = unavailable_snapshot(ProviderId::Grok, "missing");
        assert_eq!(snap.status, SnapshotStatus::Unavailable);
        assert!(snap.windows.is_empty());
        assert_eq!(snap.message.as_deref(), Some("missing"));
    }
}
