//! Shared vendor snapshot assembly (primary % / reset / status).

use crate::domain::types::{
    DataSource, ProviderId, ProviderSnapshot, SnapshotStatus, UsageWindow,
};
use chrono::{DateTime, Utc};

pub fn finish_snapshot(
    id: ProviderId,
    windows: Vec<UsageWindow>,
    message: Option<String>,
    now: DateTime<Utc>,
) -> ProviderSnapshot {
    let primary_used_percent = windows
        .iter()
        .filter_map(|w| w.used_percent)
        .fold(None, |acc: Option<f64>, p| {
            Some(acc.map(|a| a.max(p)).unwrap_or(p))
        });

    let primary_resets_at = windows
        .iter()
        .filter_map(|w| w.resets_at.as_ref())
        .filter_map(|s| DateTime::parse_from_rfc3339(s).ok())
        .filter(|d| d.with_timezone(&Utc) > now)
        .min()
        .map(|d| d.with_timezone(&Utc).to_rfc3339())
        .or_else(|| windows.iter().filter_map(|w| w.resets_at.clone()).min());

    let status = if windows.is_empty() {
        SnapshotStatus::Degraded
    } else {
        SnapshotStatus::Ok
    };

    ProviderSnapshot {
        provider_id: id,
        display_name: id.display_name().into(),
        windows,
        status,
        source: DataSource::Vendor,
        as_of: now.to_rfc3339(),
        message,
        primary_resets_at,
        primary_used_percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{UsageUnit, WindowKind};
    use chrono::TimeZone;

    fn win(pct: f64, reset: &str) -> UsageWindow {
        UsageWindow {
            kind: WindowKind::Weekly,
            used: pct,
            limit: Some(100.0),
            unit: UsageUnit::Percent,
            resets_at: Some(reset.into()),
            used_percent: Some(pct),
            label: Some("Week".into()),
        }
    }

    #[test]
    fn empty_is_degraded() {
        let now = Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap();
        let snap = finish_snapshot(ProviderId::Grok, vec![], None, now);
        assert_eq!(snap.status, SnapshotStatus::Degraded);
        assert_eq!(snap.source, DataSource::Vendor);
        assert!(snap.primary_used_percent.is_none());
    }

    #[test]
    fn primary_is_max_percent_and_soonest_future_reset() {
        let now = Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap();
        let snap = finish_snapshot(
            ProviderId::Claude,
            vec![
                win(20.0, "2026-08-10T00:00:00Z"),
                win(55.0, "2026-08-02T00:00:00Z"),
            ],
            Some("max".into()),
            now,
        );
        assert_eq!(snap.status, SnapshotStatus::Ok);
        assert!((snap.primary_used_percent.unwrap() - 55.0).abs() < 0.01);
        assert!(snap
            .primary_resets_at
            .as_ref()
            .unwrap()
            .starts_with("2026-08-02"));
        assert_eq!(snap.message.as_deref(), Some("max"));
    }
}
