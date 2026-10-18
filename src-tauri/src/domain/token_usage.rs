//! Recorded token metadata; independent of vendor quota and activity.
use chrono::{DateTime, Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
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
}

#[derive(Clone)]
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
    month_start(now).min(week_start(now))
}

pub struct TokenLedger {
    files: BTreeMap<String, BTreeMap<String, Contribution>>,
    lineage: BTreeMap<String, String>,
    records: usize,
    limit: usize,
    limited: bool,
}
impl Default for TokenLedger {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            lineage: BTreeMap::new(),
            records: 0,
            limit: MAX_RETAINED_RECORDS,
            limited: false,
        }
    }
}

impl TokenLedger {
    pub fn remove_owner(&mut self, owner: &str) {
        if let Some(rows) = self.files.remove(owner) {
            self.records -= rows.len();
        }
    }

    pub fn register_lineage(&mut self, session: &str, parent: &str) {
        if session != parent && self.lineage.len() < MAX_RETAINED_RECORDS {
            self.lineage.insert(session.into(), parent.into());
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
            self.limited = true;
            return;
        }
        if !exists {
            self.records += 1;
        }
        self.files.entry(owner.into()).or_default().insert(
            key.into(),
            Contribution {
                key: key.into(),
                at,
                model,
                counts,
            },
        );
    }

    pub fn prune(&mut self, oldest: NaiveDate) {
        for rows in self.files.values_mut() {
            rows.retain(|_, r| r.at.date_naive() >= oldest);
        }
        self.records = self.files.values().map(BTreeMap::len).sum();
    }

    pub fn summarize(
        &self,
        now: DateTime<Local>,
        mut status: TokenUsageStatus,
    ) -> TokenUsageSnapshot {
        let oldest = retention_start(now);
        let first_month_date = month_start(now);
        let first_week_date = week_start(now);
        let mut today = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut week = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut month = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut seen = HashSet::new();
        for r in self.files.values().flat_map(|v| v.values()) {
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
        TokenUsageSnapshot {
            source: "local_records".into(),
            as_of: now.to_rfc3339(),
            status,
            today,
            week,
            month,
        }
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
        let snap = ledger.summarize(now, TokenUsageStatus::Ready);
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
        let snap = ledger.summarize(now, TokenUsageStatus::Ready);
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
            Some(TokenLedger::default().summarize(Local::now(), TokenUsageStatus::Ready));
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
            ledger.summarize(now, TokenUsageStatus::Ready).status,
            TokenUsageStatus::Partial
        );
        assert_eq!(
            ledger
                .summarize(now, TokenUsageStatus::Ready)
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
        let summary = ledger.summarize(now, TokenUsageStatus::Ready);
        assert_eq!(summary.today.totals.total(), Some(11));
        assert_eq!(summary.week.totals.total(), Some(22));
        assert_eq!(summary.month.totals.total(), Some(11));
    }
}
