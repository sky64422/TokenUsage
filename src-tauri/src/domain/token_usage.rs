//! Recorded token metadata; independent of vendor quota and activity.
use super::types::{UsageWindow, WindowKind};
use chrono::{DateTime, Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenUsageStatus {
    Loading,
    Ready,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCounts {
    pub input: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
}

impl TokenCounts {
    pub fn total(&self) -> Option<u64> {
        self.input?.checked_add(self.output?)
    }

    pub fn valid(&self) -> bool {
        self.input.zip(self.cache_read).is_none_or(|(i, c)| c <= i)
            && self.output.zip(self.reasoning).is_none_or(|(o, r)| r <= o)
            && self
                .input
                .zip(self.output)
                .is_none_or(|(i, o)| i.checked_add(o).is_some())
    }

    pub fn add(&mut self, other: &Self) -> bool {
        let mut valid = true;
        for (a, b) in [
            (&mut self.input, other.input),
            (&mut self.cache_read, other.cache_read),
            (&mut self.cache_write, other.cache_write),
            (&mut self.output, other.output),
            (&mut self.reasoning, other.reasoning),
        ] {
            let next = a.zip(b).and_then(|(x, y)| x.checked_add(y));
            if a.is_some() && b.is_some() && next.is_none() {
                valid = false;
            }
            *a = next;
        }
        valid
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTokenUsage {
    pub model: Option<String>,
    pub counts: TokenCounts,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenPeriod {
    pub models: Vec<ModelTokenUsage>,
    pub totals: TokenCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUsageSnapshot {
    pub source: String,
    pub as_of: String,
    pub status: TokenUsageStatus,
    pub today: TokenPeriod,
    pub week: TokenPeriod,
    pub month: TokenPeriod,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<TokenWindowUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenWindowUsage {
    pub kind: WindowKind,
    pub resets_at: String,
    pub usage: TokenPeriod,
}

#[derive(Clone, PartialEq)]
struct Contribution {
    key: String,
    at: DateTime<Local>,
    model: Option<String>,
    counts: TokenCounts,
}

const MAX_RETAINED_RECORDS: usize = 100_000;
const MAX_LINEAGE_DEPTH: usize = 64;
pub fn month_start(now: DateTime<Local>) -> NaiveDate {
    now.date_naive().with_day(1).expect("first day of month")
}
pub fn week_start(now: DateTime<Local>) -> NaiveDate {
    now.date_naive() - chrono::Duration::days(now.weekday().num_days_from_monday() as i64)
}
pub fn retention_start(now: DateTime<Local>) -> NaiveDate {
    month_start(now).min((now - chrono::Duration::weeks(1)).date_naive())
}

pub struct TokenLedger {
    files: BTreeMap<String, BTreeMap<String, Contribution>>,
    lineage: BTreeMap<String, String>,
    records: usize,
    limit: usize,
    limited: bool,
    revision: u64,
    pruned_through: Option<NaiveDate>,
    summary_cache: RefCell<Option<SummaryCache>>,
}

struct SummaryCache {
    revision: u64,
    at: DateTime<Local>,
    next_record: Option<DateTime<Local>>,
    status: TokenUsageStatus,
    windows: Vec<(WindowKind, DateTime<Local>, DateTime<Local>)>,
    snapshot: TokenUsageSnapshot,
}
impl Default for TokenLedger {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            lineage: BTreeMap::new(),
            records: 0,
            limit: MAX_RETAINED_RECORDS,
            limited: false,
            revision: 0,
            pruned_through: None,
            summary_cache: RefCell::new(None),
        }
    }
}

impl TokenLedger {
    pub fn remove_owner(&mut self, owner: &str) {
        if let Some(rows) = self.files.remove(owner) {
            self.records -= rows.len();
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn register_lineage(&mut self, session: &str, parent: &str) {
        if session != parent
            && self.lineage.len() < MAX_RETAINED_RECORDS
            && self.lineage.get(session).is_none_or(|old| old != parent)
        {
            self.lineage.insert(session.into(), parent.into());
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn logical_key(&self, key: &str) -> String {
        let Some((session, rest)) = key.split_once(':') else {
            return key.into();
        };
        let mut root = session;
        let mut seen = HashSet::new();
        for _ in 0..MAX_LINEAGE_DEPTH {
            if !seen.insert(root) {
                break;
            }
            let Some(parent) = self.lineage.get(root) else {
                break;
            };
            root = parent;
        }
        format!("{root}:{rest}")
    }

    pub fn upsert(
        &mut self,
        owner: &str,
        key: &str,
        at: DateTime<Local>,
        model: Option<String>,
        counts: TokenCounts,
    ) {
        let exists = self
            .files
            .get(owner)
            .is_some_and(|rows| rows.contains_key(key));
        if !exists && self.records >= self.limit {
            if !self.limited {
                self.revision = self.revision.wrapping_add(1);
            }
            self.limited = true;
            return;
        }
        if !exists {
            self.records += 1;
        }
        let contribution = Contribution {
            key: key.into(),
            at,
            model,
            counts,
        };
        let rows = self.files.entry(owner.into()).or_default();
        if rows.get(key) != Some(&contribution) {
            rows.insert(key.into(), contribution);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn prune(&mut self, oldest: NaiveDate) {
        if self
            .pruned_through
            .is_some_and(|previous| oldest <= previous)
        {
            return;
        }
        self.pruned_through = Some(oldest);
        self.files.retain(|_, rows| {
            rows.retain(|_, r| r.at.date_naive() >= oldest);
            !rows.is_empty()
        });
        let previous = self.records;
        self.records = self.files.values().map(BTreeMap::len).sum();
        if previous != self.records {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn summarize(
        &self,
        now: DateTime<Local>,
        mut status: TokenUsageStatus,
        windows: &[UsageWindow],
    ) -> TokenUsageSnapshot {
        let requested_status = status;
        let oldest = retention_start(now);
        let first_month_date = month_start(now);
        let first_week_date = week_start(now);
        let mut today = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut week = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut month = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut window_models: Vec<_> = windows
            .iter()
            .filter_map(|window| {
                let duration = match window.kind {
                    WindowKind::Rolling5h => chrono::Duration::hours(5),
                    WindowKind::Weekly => chrono::Duration::weeks(1),
                    _ => return None,
                };
                let reset = DateTime::parse_from_rfc3339(window.resets_at.as_ref()?)
                    .ok()?
                    .with_timezone(&Local);
                let start = reset.checked_sub_signed(duration)?;
                if reset <= now || start > now {
                    return None;
                }
                Some((window.kind, reset, start))
            })
            .collect();
        if let Some(cache) = self.summary_cache.borrow().as_ref() {
            if cache.revision == self.revision
                && cache.status == status
                && cache.at.date_naive() == now.date_naive()
                && now >= cache.at
                && cache.next_record.is_none_or(|at| now < at)
                && cache.windows == window_models
            {
                let mut snapshot = cache.snapshot.clone();
                snapshot.as_of = now.to_rfc3339();
                return snapshot;
            }
        }
        let window_keys = window_models.clone();
        let mut window_models: Vec<_> = window_models
            .drain(..)
            .map(|(kind, reset, start)| {
                (
                    kind,
                    reset,
                    start,
                    BTreeMap::<Option<String>, TokenCounts>::new(),
                )
            })
            .collect();
        let mut seen = HashSet::new();
        let mut next_record = None;
        for r in self.files.values().flat_map(|v| v.values()) {
            if r.at > now {
                next_record = Some(next_record.map_or(r.at, |at: DateTime<Local>| at.min(r.at)));
            }
            if r.at > now || r.at.date_naive() < oldest || !seen.insert(self.logical_key(&r.key)) {
                continue;
            }
            if !r.counts.valid() {
                status = TokenUsageStatus::Partial;
                continue;
            }
            if r.at.date_naive() >= first_month_date && !merge(&mut month, r) {
                status = TokenUsageStatus::Partial;
            }
            if r.at.date_naive() >= first_week_date && !merge(&mut week, r) {
                status = TokenUsageStatus::Partial;
            }
            if r.at.date_naive() == now.date_naive() && !merge(&mut today, r) {
                status = TokenUsageStatus::Partial;
            }
            for (_, _, start, models) in &mut window_models {
                if r.at >= *start && !merge(models, r) {
                    status = TokenUsageStatus::Partial;
                }
            }
        }
        let (today, today_ok) = period(today);
        let (month, month_ok) = period(month);
        let (week, week_ok) = period(week);
        if !today_ok || !month_ok || !week_ok {
            status = TokenUsageStatus::Partial;
        }
        if self.limited && status != TokenUsageStatus::Loading {
            status = TokenUsageStatus::Partial;
        }
        let windows = window_models
            .into_iter()
            .map(|(kind, reset, _, models)| {
                let (usage, valid) = period(models);
                if !valid {
                    status = TokenUsageStatus::Partial;
                }
                TokenWindowUsage {
                    kind,
                    resets_at: reset.to_rfc3339(),
                    usage,
                }
            })
            .collect();
        let snapshot = TokenUsageSnapshot {
            source: "local_records".into(),
            as_of: now.to_rfc3339(),
            status,
            today,
            week,
            month,
            windows,
        };
        *self.summary_cache.borrow_mut() = Some(SummaryCache {
            revision: self.revision,
            at: now,
            next_record,
            status: requested_status,
            windows: window_keys,
            snapshot: snapshot.clone(),
        });
        snapshot
    }
}

fn merge(map: &mut BTreeMap<Option<String>, TokenCounts>, r: &Contribution) -> bool {
    match map.get_mut(&r.model) {
        Some(counts) => counts.add(&r.counts),
        None => {
            map.insert(r.model.clone(), r.counts.clone());
            true
        }
    }
}

fn period(map: BTreeMap<Option<String>, TokenCounts>) -> (TokenPeriod, bool) {
    let mut models: Vec<_> = map
        .into_iter()
        .map(|(model, counts)| ModelTokenUsage { model, counts })
        .collect();
    models.sort_by(|a, b| {
        b.counts
            .total()
            .cmp(&a.counts.total())
            .then(a.model.cmp(&b.model))
    });
    let mut totals = models.first().map(|r| r.counts.clone()).unwrap_or_default();
    let mut valid = true;
    for row in models.iter().skip(1) {
        valid &= totals.add(&row.counts);
    }
    (TokenPeriod { models, totals }, valid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn counts(input: u64, read: u64, output: u64) -> TokenCounts {
        TokenCounts {
            input: Some(input),
            cache_read: Some(read),
            output: Some(output),
            ..Default::default()
        }
    }

    #[test]
    fn pruning_expired_usage_releases_empty_owner_maps() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        ledger.upsert(
            "deleted-file",
            "old",
            now - chrono::Duration::days(40),
            None,
            counts(10, 0, 1),
        );
        ledger.prune(retention_start(now));
        assert!(ledger.files.is_empty());
    }

    #[test]
    fn summary_cache_reuses_idle_totals_but_expires_for_records_dates_and_revisions() {
        let now = Local.with_ymd_and_hms(2026, 10, 31, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        ledger.upsert("file", "current", now, None, counts(10, 0, 1));
        ledger.upsert(
            "file",
            "future",
            now + chrono::Duration::hours(1),
            None,
            counts(20, 0, 2),
        );
        assert_eq!(
            ledger
                .summarize(now, TokenUsageStatus::Ready, &[])
                .today
                .totals
                .total(),
            Some(11)
        );
        let idle = now + chrono::Duration::seconds(5);
        let snapshot = ledger.summarize(idle, TokenUsageStatus::Ready, &[]);
        assert_eq!(snapshot.as_of, idle.to_rfc3339());
        assert_eq!(snapshot.today.totals.total(), Some(11));
        assert_eq!(ledger.summary_cache.borrow().as_ref().unwrap().at, now);
        let later = now + chrono::Duration::hours(1);
        assert_eq!(
            ledger
                .summarize(later, TokenUsageStatus::Ready, &[])
                .today
                .totals
                .total(),
            Some(33)
        );
        ledger.upsert(
            "file",
            "current",
            now,
            Some("corrected-model".into()),
            counts(30, 0, 3),
        );
        assert_eq!(
            ledger
                .summarize(later, TokenUsageStatus::Ready, &[])
                .today
                .totals
                .total(),
            Some(55)
        );
        assert_eq!(
            ledger
                .summarize(now, TokenUsageStatus::Ready, &[])
                .today
                .totals
                .total(),
            Some(33)
        );
        let tomorrow = now + chrono::Duration::hours(12);
        let snapshot = ledger.summarize(tomorrow, TokenUsageStatus::Ready, &[]);
        assert_eq!(snapshot.today.totals.total(), None);
        assert_eq!(snapshot.month.totals.total(), None);
        assert_eq!(snapshot.week.totals.total(), Some(55));
        assert_eq!(
            ledger
                .summarize(tomorrow, TokenUsageStatus::Partial, &[])
                .status,
            TokenUsageStatus::Partial
        );
    }

    #[test]
    fn summary_cache_tracks_quota_window_activation_expiration_and_reset_changes() {
        use crate::domain::types::UsageUnit;
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        ledger.upsert("file", "event", now, None, counts(10, 0, 1));
        let mut windows = vec![UsageWindow {
            group: None,
            kind: WindowKind::Rolling5h,
            used: 0.,
            limit: None,
            unit: UsageUnit::Percent,
            resets_at: Some((now + chrono::Duration::hours(6)).to_rfc3339()),
            used_percent: None,
            label: None,
        }];
        assert!(ledger
            .summarize(now, TokenUsageStatus::Ready, &windows)
            .windows
            .is_empty());
        let active = now + chrono::Duration::hours(1);
        assert_eq!(
            ledger
                .summarize(active, TokenUsageStatus::Ready, &windows)
                .windows
                .len(),
            1
        );
        windows[0].resets_at = Some((now + chrono::Duration::hours(5)).to_rfc3339());
        assert_eq!(
            ledger
                .summarize(active, TokenUsageStatus::Ready, &windows)
                .windows[0]
                .usage
                .totals
                .total(),
            Some(11)
        );
        assert!(ledger
            .summarize(
                now + chrono::Duration::hours(5),
                TokenUsageStatus::Ready,
                &windows
            )
            .windows
            .is_empty());
    }

    #[test]
    fn main_bar_models_use_vendor_reset_periods_instead_of_calendar_tabs() {
        use crate::domain::types::{UsageUnit, UsageWindow, WindowKind};
        let now = Local.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
        let reset = now + chrono::Duration::hours(2);
        let windows = vec![
            UsageWindow {
                group: None,
                kind: WindowKind::Rolling5h,
                used: 60.,
                limit: Some(100.),
                unit: UsageUnit::Percent,
                resets_at: Some(reset.to_rfc3339()),
                used_percent: Some(60.),
                label: Some("5h".into()),
            },
            UsageWindow {
                group: None,
                kind: WindowKind::Weekly,
                used: 20.,
                limit: Some(100.),
                unit: UsageUnit::Percent,
                resets_at: Some((now + chrono::Duration::days(1)).to_rfc3339()),
                used_percent: Some(20.),
                label: Some("Week".into()),
            },
        ];
        let mut ledger = TokenLedger::default();
        for (key, at, input) in [
            ("previous-cycle", now - chrono::Duration::hours(4), 1000),
            ("current-cycle", now - chrono::Duration::hours(2), 200),
            ("week-before-month", now - chrono::Duration::days(4), 300),
        ] {
            ledger.upsert("file", key, at, Some(key.into()), counts(input, 0, 0));
        }
        ledger.prune(retention_start(now));
        let snapshot = ledger.summarize(now, TokenUsageStatus::Ready, &windows);
        assert_eq!(snapshot.windows[0].usage.totals.total(), Some(200));
        assert_eq!(snapshot.windows[1].usage.totals.total(), Some(1500));
        assert_eq!(snapshot.windows[0].resets_at, reset.to_rfc3339());
        assert_eq!(snapshot.today.totals.total(), Some(1200));
        assert_eq!(snapshot.month.totals.total(), Some(1200));
        assert_eq!(snapshot.week.totals.total(), Some(1200));
        let mut expired = windows;
        expired[0].resets_at = Some(now.to_rfc3339());
        expired[1].resets_at = None;
        assert!(ledger
            .summarize(now, TokenUsageStatus::Ready, &expired)
            .windows
            .is_empty());
    }

    #[test]
    fn repeated_records_and_subsets_do_not_inflate_totals() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        for owner in ["original", "copy"] {
            ledger.upsert(
                owner,
                "event1",
                now,
                Some("model-a".into()),
                counts(100, 80, 20),
            );
            ledger.upsert(
                owner,
                "event2",
                now,
                Some("model-b".into()),
                counts(150, 100, 20),
            );
        }
        let snap = ledger.summarize(now, TokenUsageStatus::Ready, &[]);
        assert_eq!(snap.today.totals.input, Some(250));
        assert_eq!(snap.today.totals.output, Some(40));
        assert_eq!(snap.today.totals.total(), Some(290));
        assert_eq!(snap.today.models.len(), 2);
        assert_eq!(snap.today.totals.cache_write, None);
    }

    #[test]
    fn periods_use_local_dates_and_exclude_future_records() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 0, 0, 1).unwrap();
        let mut ledger = TokenLedger::default();
        for (key, at) in [
            ("today", now),
            ("yesterday", now - chrono::Duration::seconds(2)),
            (
                "month_start",
                Local
                    .from_local_datetime(&month_start(now).and_hms_opt(0, 0, 0).unwrap())
                    .unwrap(),
            ),
            (
                "too_old",
                Local
                    .from_local_datetime(&month_start(now).and_hms_opt(0, 0, 0).unwrap())
                    .unwrap()
                    - chrono::Duration::seconds(1),
            ),
            ("future", now + chrono::Duration::hours(1)),
        ] {
            ledger.upsert("file", key, at, None, counts(10, 0, 1));
        }
        let snap = ledger.summarize(now, TokenUsageStatus::Ready, &[]);
        assert_eq!(snap.today.totals.input, Some(10));
        assert_eq!(snap.month.totals.input, Some(30));
        assert!(snap.today.models[0].model.is_none());
    }

    #[test]
    fn invalid_subsets_and_overflow_are_not_healthy_zero() {
        assert!(!counts(1, 2, 0).valid());
        assert_eq!(counts(u64::MAX, 0, 1).total(), None);
        let mut a = counts(u64::MAX, 0, 1);
        assert!(!a.add(&counts(1, 0, 1)));
        assert_eq!(a.input, None);
    }

    #[test]
    fn old_quota_cache_deserializes_without_token_details() {
        let old = serde_json::json!({"provider_id":"codex","display_name":"Codex","windows":[],
            "status":"auth_required","source":"unavailable","as_of":"2026-10-18T00:00:00Z",
            "message":null,"primary_resets_at":null,"primary_used_percent":null});
        let mut snap: crate::domain::types::ProviderSnapshot = serde_json::from_value(old).unwrap();
        assert!(snap.token_usage.is_none());
        snap.token_usage =
            Some(TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Ready, &[]));
        assert_eq!(
            snap.status,
            crate::domain::types::SnapshotStatus::AuthRequired
        );
        assert!(snap.primary_used_percent.is_none());
    }

    #[test]
    fn retention_budget_is_bounded_and_excluded_data_is_partial() {
        let now = Local::now();
        let mut ledger = TokenLedger {
            limit: 2,
            ..Default::default()
        };
        for key in ["a", "b", "c"] {
            ledger.upsert("file", key, now, None, counts(10, 0, 1));
        }
        assert_eq!(ledger.records, 2);
        assert_eq!(
            ledger.summarize(now, TokenUsageStatus::Ready, &[]).status,
            TokenUsageStatus::Partial
        );
        assert_eq!(
            ledger
                .summarize(now, TokenUsageStatus::Ready, &[])
                .today
                .totals
                .total(),
            Some(22)
        );
    }

    #[test]
    fn calendar_week_starts_monday_and_can_cross_a_month_boundary() {
        let now = Local.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        for (key, at) in [
            ("today", now),
            ("monday", now - chrono::Duration::days(6)),
            ("previous_sunday", now - chrono::Duration::days(7)),
        ] {
            ledger.upsert("file", key, at, None, counts(10, 0, 1));
        }
        let summary = ledger.summarize(now, TokenUsageStatus::Ready, &[]);
        assert_eq!(summary.today.totals.total(), Some(11));
        assert_eq!(summary.week.totals.total(), Some(22));
        assert_eq!(summary.month.totals.total(), Some(11));
    }
}
