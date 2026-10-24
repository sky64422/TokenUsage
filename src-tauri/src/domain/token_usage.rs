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
    #[serde(default)]
    pub yesterday: TokenPeriod,
    pub week: TokenPeriod,
    pub month: TokenPeriod,
    #[serde(default)]
    pub daily: Vec<DailyTokenUsage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<TokenWindowUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyTokenUsage {
    pub date: NaiveDate,
    pub totals: TokenCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenWindowUsage {
    pub kind: WindowKind,
    pub resets_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub usage: TokenPeriod,
}

#[derive(Clone, PartialEq)]
struct Contribution {
    key: String,
    event: Option<String>,
    at: DateTime<Local>,
    model: Option<String>,
    counts: TokenCounts,
}

const MAX_RETAINED_RECORDS: usize = 100_000;
const MAX_LINEAGE_DEPTH: usize = 64;
pub fn recent_30d_start(now: DateTime<Local>) -> NaiveDate {
    now.date_naive() - chrono::Duration::days(29)
}
pub fn month_start(now: DateTime<Local>) -> NaiveDate {
    now.date_naive().with_day(1).expect("first day of month")
}
pub fn week_start(now: DateTime<Local>) -> NaiveDate {
    now.date_naive() - chrono::Duration::days(now.weekday().num_days_from_monday() as i64)
}
pub fn retention_start(now: DateTime<Local>) -> NaiveDate {
    recent_30d_start(now)
}

pub struct TokenLedger {
    files: BTreeMap<String, BTreeMap<String, Contribution>>,
    lineage: BTreeMap<String, String>,
    records: usize,
    limit: usize,
    excluded_through: Option<NaiveDate>,
    revision: u64,
    pruned_through: Option<NaiveDate>,
    summary_cache: RefCell<Option<SummaryCache>>,
}

struct SummaryCache {
    revision: u64,
    at: DateTime<Local>,
    next_record: Option<DateTime<Local>>,
    status: TokenUsageStatus,
    windows: Vec<RecordedWindow>,
    snapshot: TokenUsageSnapshot,
}

#[derive(Clone, PartialEq)]
struct RecordedWindow {
    kind: WindowKind,
    reset: DateTime<Local>,
    start: DateTime<Local>,
    group: Option<String>,
    label: Option<String>,
}
impl RecordedWindow {
    fn matches_model(&self, model: Option<&str>) -> bool {
        let model = model.unwrap_or("").to_ascii_lowercase();
        let label = self.label.as_deref().unwrap_or("").to_ascii_lowercase();
        match self.group.as_deref().map(str::to_ascii_lowercase).as_deref() {
            Some("gemini") => model.starts_with("gemini-") && !model.contains("argon"),
            Some("argon") => model.contains("argon"),
            Some("claude" | "claude/gpt" | "claude and gpt") => model.starts_with("claude-") || model.starts_with("gpt-"),
            Some(_) => false,
            None if label.starts_with("opus ") => model.starts_with("claude-") && model.contains("opus"),
            None if label.starts_with("sonnet ") => model.starts_with("claude-") && model.contains("sonnet"),
            None => true,
        }
    }
}
impl Default for TokenLedger {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            lineage: BTreeMap::new(),
            records: 0,
            limit: MAX_RETAINED_RECORDS,
            excluded_through: None,
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
            // Preserve missing coverage until its latest event date expires,
            // including future records; freeing a slot cannot recover a lost row.
            let date = at.date_naive();
            let excluded_through = Some(self.excluded_through.map_or(date, |old| old.max(date)));
            if self.excluded_through != excluded_through {
                self.excluded_through = excluded_through;
                self.revision = self.revision.wrapping_add(1);
            }
            return;
        }
        if !exists {
            self.records += 1;
        }
        let contribution = Contribution {
            key: key.into(),
            event: None,
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
        if self.excluded_through.is_some_and(|date| date < oldest) {
            self.excluded_through = None;
            self.revision = self.revision.wrapping_add(1);
        }
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

    /// A completed turn can be re-emitted with a corrected set of model rows.
    pub fn replace_event(&mut self, owner: &str, event: &str, at: DateTime<Local>, rows: Vec<(Option<String>, TokenCounts)>) {
        let prefix = format!("{event}:");
        if let Some(existing) = self.files.get_mut(owner) {
            let keys: Vec<_> = existing.range(prefix.clone()..).take_while(|(key, _)| key.starts_with(&prefix))
                .map(|(key, _)| key.clone()).collect();
            if !keys.is_empty() {
                self.records -= keys.len();
                self.revision = self.revision.wrapping_add(1);
                for key in keys { existing.remove(&key); }
            }
        }
        for (model, counts) in rows {
            let key = format!("{prefix}{}", model.as_deref().unwrap_or(""));
            self.upsert(owner, &key, at, model, counts);
            if let Some(record) = self.files.get_mut(owner).and_then(|records| records.get_mut(&key)) {
                record.event = Some(event.into());
            }
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
        let first_30d_date = recent_30d_start(now);
        let first_week_date = week_start(now);
        let mut today = BTreeMap::<Option<String>, TokenCounts>::new();
        let mut yesterday = BTreeMap::<Option<String>, TokenCounts>::new();
        let yesterday_date = now.date_naive() - chrono::Duration::days(1);
        let mut daily = BTreeMap::<NaiveDate, BTreeMap<Option<String>, TokenCounts>>::new();
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
                Some(RecordedWindow { kind: window.kind, reset, start, group: window.group.clone(), label: window.label.clone() })
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
            .map(|window| (window, BTreeMap::<Option<String>, TokenCounts>::new()))
            .collect();
        let mut seen = HashSet::new();
        let mut next_record = None;
        // Copies of a turn with different model maps must select one complete
        // observation, rather than combining the unknown and named versions.
        let mut event_versions = BTreeMap::<&str, BTreeMap<&str, (DateTime<Local>, usize)>>::new();
        for (owner, rows) in &self.files {
            for row in rows.values().filter(|row| row.at <= now && row.at.date_naive() >= oldest) {
                if let Some(event) = row.event.as_deref() {
                    let version = event_versions.entry(event).or_default().entry(owner)
                        .or_insert((row.at, 0));
                    version.0 = version.0.max(row.at);
                    version.1 += usize::from(row.model.is_some());
                }
            }
        }
        let event_owners: BTreeMap<_, _> = event_versions.into_iter().filter_map(|(event, versions)|
            versions.into_iter().max_by_key(|(owner, version)| (*version, *owner)).map(|(owner, _)| (event, owner))).collect();
        for (owner, r) in self.files.iter().flat_map(|(owner, rows)| rows.values().map(move |row| (owner, row))) {
            if r.at > now {
                next_record = Some(next_record.map_or(r.at, |at: DateTime<Local>| at.min(r.at)));
            }
            if r.at > now || r.at.date_naive() < oldest
                || r.event.as_deref().is_some_and(|event| event_owners.get(event).is_some_and(|selected| *selected != owner))
                || !seen.insert(self.logical_key(&r.key)) {
                continue;
            }
            if !r.counts.valid() {
                status = TokenUsageStatus::Partial;
                continue;
            }
            if r.at.date_naive() >= first_30d_date && !merge(&mut month, r) {
                status = TokenUsageStatus::Partial;
            }
            if r.at.date_naive() >= first_30d_date
                && !merge(daily.entry(r.at.date_naive()).or_default(), r)
            {
                status = TokenUsageStatus::Partial;
            }
            if r.at.date_naive() == yesterday_date && !merge(&mut yesterday, r) {
                status = TokenUsageStatus::Partial;
            }
            if r.at.date_naive() >= first_week_date && !merge(&mut week, r) {
                status = TokenUsageStatus::Partial;
            }
            if r.at.date_naive() == now.date_naive() && !merge(&mut today, r) {
                status = TokenUsageStatus::Partial;
            }
            for (window, models) in &mut window_models {
                if r.at >= window.start && window.matches_model(r.model.as_deref()) && !merge(models, r) {
                    status = TokenUsageStatus::Partial;
                }
            }
        }
        let (today, today_ok) = period(today);
        let (yesterday, yesterday_ok) = period(yesterday);
        let (month, month_ok) = period(month);
        let (week, week_ok) = period(week);
        if !today_ok || !yesterday_ok || !month_ok || !week_ok {
            status = TokenUsageStatus::Partial;
        }
        let daily = (0..30).map(|offset| {
            let date = first_30d_date + chrono::Duration::days(i64::from(offset));
            let (usage, valid) = period(daily.remove(&date).unwrap_or_default());
            if !valid { status = TokenUsageStatus::Partial; }
            DailyTokenUsage { date, totals: usage.totals }
        }).collect();
        if self.excluded_through.is_some_and(|date| date >= oldest)
            && status != TokenUsageStatus::Loading
        {
            status = TokenUsageStatus::Partial;
        }
        let windows = window_models
            .into_iter()
            .map(|(window, models)| {
                let (usage, valid) = period(models);
                if !valid {
                    status = TokenUsageStatus::Partial;
                }
                TokenWindowUsage {
                    kind: window.kind,
                    resets_at: window.reset.to_rfc3339(),
                    group: window.group,
                    label: window.label,
                    usage,
                }
            })
            .collect();
        let snapshot = TokenUsageSnapshot {
            source: "local_records".into(),
            as_of: now.to_rfc3339(),
            status,
            today,
            yesterday,
            week,
            month,
            daily,
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
    fn calendar_summary_keeps_yesterday_across_month_boundary_and_daily_gaps_unknown() {
        let now = Local.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        ledger.upsert("a", "yesterday", now - chrono::Duration::days(1), None, counts(100, 50, 20));
        ledger.upsert("a", "today", now, None, counts(200, 100, 40));
        ledger.upsert("copy", "today", now, None, counts(200, 100, 40));
        ledger.upsert("a", "future", now + chrono::Duration::days(1), None, counts(900, 0, 10));
        ledger.prune(retention_start(now));
        let json = serde_json::to_value(ledger.summarize(now, TokenUsageStatus::Ready, &[])).unwrap();
        assert_eq!(json["yesterday"]["totals"]["input"], 100);
        assert_eq!(json["month"]["totals"]["input"], 300);
        assert_eq!(json["daily"].as_array().unwrap().len(), 30);
        assert_eq!(json["daily"][28]["date"], "2026-10-31");
        assert_eq!(json["daily"][28]["totals"]["input"], 100);
        assert_eq!(json["daily"][29]["date"], "2026-11-01");
        assert_eq!(json["daily"][29]["totals"]["input"], 200);
        let later = now + chrono::Duration::days(2);
        let json = serde_json::to_value(ledger.summarize(later, TokenUsageStatus::Ready, &[])).unwrap();
        assert_eq!(json["daily"].as_array().unwrap().len(), 30);
        assert_eq!(json["daily"][28]["totals"]["input"], 900);
        assert!(json["daily"][29]["totals"]["input"].is_null());
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
        assert_eq!(snapshot.month.totals.total(), Some(55));
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
        assert_eq!(snapshot.month.totals.total(), Some(1500));
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
    fn copied_event_versions_choose_a_complete_model_map_without_double_counting() {
        let now = Local.with_ymd_and_hms(2026, 10, 19, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger::default();
        ledger.replace_event("original", "session:prompt", now, vec![(None, counts(100, 0, 0))]);
        ledger.replace_event("copy", "session:prompt", now,
            vec![(Some("grok-a".into()), counts(60, 0, 0)), (Some("grok-b".into()), counts(40, 0, 0))]);
        let summary = ledger.summarize(now, TokenUsageStatus::Ready, &[]);
        assert_eq!(summary.today.totals.total(), Some(100));
        assert_eq!(summary.today.models.len(), 2);
        ledger.replace_event("copy", "session:prompt", now, vec![(Some("grok-b".into()), counts(90, 0, 0))]);
        let summary = ledger.summarize(now, TokenUsageStatus::Ready, &[]);
        assert_eq!(summary.today.totals.total(), Some(90));
        assert_eq!(summary.today.models.len(), 1);
        ledger.replace_event("original", "session:prompt", now + chrono::Duration::seconds(1), vec![(None, counts(80, 0, 0))]);
        assert_eq!(ledger.summarize(now, TokenUsageStatus::Ready, &[]).today.totals.total(), Some(90));
        let later = ledger.summarize(now + chrono::Duration::seconds(1), TokenUsageStatus::Ready, &[]);
        assert_eq!(later.today.totals.total(), Some(80));
        assert_eq!(later.today.models[0].model, None);
    }

    #[test]
    fn matching_windows_keep_families_and_model_buckets_independent() {
        use crate::domain::types::UsageUnit;
        let now = Local.with_ymd_and_hms(2026, 10, 19, 12, 0, 0).unwrap();
        let base = UsageWindow { group: None, kind: WindowKind::Weekly, used: 20., limit: Some(100.),
            unit: UsageUnit::Percent, resets_at: Some((now + chrono::Duration::days(1)).to_rfc3339()),
            used_percent: Some(20.), label: Some("Week".into()) };
        let mut windows = vec![base.clone(); 5];
        windows[1].label = Some("Opus week".into());
        windows[2].label = Some("Sonnet week".into());
        windows[3].group = Some("Gemini".into());
        windows[4].group = Some("Claude".into());
        let mut argon_win = base.clone();
        argon_win.group = Some("Argon".into());
        windows.push(argon_win);
        let mut ledger = TokenLedger::default();
        for (model, amount) in [(Some("claude-opus-4-7"), 100), (Some("claude-sonnet-4-6"), 200),
            (Some("gemini-3.6-pro"), 300), (Some("gemini-4-argon-high"), 500), (None, 400)] {
            ledger.upsert("file", &format!("{model:?}"), now, model.map(String::from), counts(amount, 0, 0));
        }
        let result = ledger.summarize(now, TokenUsageStatus::Ready, &windows);
        let amounts: Vec<_> = result.windows.iter().map(|w| w.usage.totals.total()).collect();
        assert_eq!(amounts, vec![Some(1500), Some(100), Some(200), Some(300), Some(300), Some(500)]);
        assert_eq!(result.windows[1].label.as_deref(), Some("Opus week"));
        assert_eq!(result.windows[3].group.as_deref(), Some("Gemini"));
        windows[0].label = Some("Sonnet week".into());
        assert_eq!(ledger.summarize(now, TokenUsageStatus::Ready, &windows).windows[0].usage.totals.total(), Some(200));
        windows[0].group = Some("Unrecognized".into());
        assert!(ledger.summarize(now, TokenUsageStatus::Ready, &windows).windows[0].usage.models.is_empty());
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
                "recent_30d_start",
                Local
                    .from_local_datetime(&recent_30d_start(now).and_hms_opt(0, 0, 0).unwrap())
                    .unwrap(),
            ),
            (
                "too_old",
                Local
                    .from_local_datetime(&recent_30d_start(now).and_hms_opt(0, 0, 0).unwrap())
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
    fn rejected_record_coverage_expires_before_new_complete_period() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger {
            limit: 2,
            ..Default::default()
        };
        for key in ["a", "b", "excluded"] {
            ledger.upsert("old-file", key, now, None, counts(100, 0, 20));
        }
        assert_eq!(
            ledger.summarize(now, TokenUsageStatus::Ready, &[]).status,
            TokenUsageStatus::Partial
        );
        let later = now + chrono::Duration::days(40);
        ledger.prune(retention_start(later));
        ledger.upsert("new-file", "new", later, None, counts(100, 0, 20));
        let summary = ledger.summarize(later, TokenUsageStatus::Ready, &[]);
        assert_eq!(summary.today.totals.total(), Some(120));
        assert_eq!(summary.status, TokenUsageStatus::Ready);
    }

    #[test]
    fn freeing_same_day_slots_does_not_erase_excluded_record_coverage() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger {
            limit: 2,
            ..Default::default()
        };
        for owner in ["a", "b", "excluded"] {
            ledger.upsert(owner, owner, now, None, counts(100, 0, 20));
        }
        ledger.remove_owner("a");
        ledger.prune(retention_start(now));
        ledger.upsert("new-file", "new", now, None, counts(100, 0, 20));
        let summary = ledger.summarize(now, TokenUsageStatus::Ready, &[]);
        assert_eq!(summary.today.totals.total(), Some(240));
        assert_eq!(summary.status, TokenUsageStatus::Partial);
    }

    #[test]
    fn latest_rejected_date_preserves_future_coverage_until_its_own_expiry() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let future = now + chrono::Duration::days(40);
        let mut ledger = TokenLedger {
            limit: 2,
            ..Default::default()
        };
        for key in ["a", "b", "old-excluded"] {
            ledger.upsert("old-file", key, now, None, counts(100, 0, 20));
        }
        ledger.upsert(
            "future-file",
            "future-excluded",
            future,
            None,
            counts(100, 0, 20),
        );
        ledger.prune(retention_start(future));
        ledger.upsert("new-file", "new", future, None, counts(100, 0, 20));
        assert_eq!(
            ledger
                .summarize(future, TokenUsageStatus::Ready, &[])
                .status,
            TokenUsageStatus::Partial
        );
        let expired = future + chrono::Duration::days(40);
        ledger.prune(retention_start(expired));
        ledger.upsert("final-file", "final", expired, None, counts(100, 0, 20));
        let summary = ledger.summarize(expired, TokenUsageStatus::Ready, &[]);
        assert_eq!(summary.status, TokenUsageStatus::Ready);
        assert_eq!(summary.today.totals.total(), Some(120));
    }

    #[test]
    fn expiring_coverage_invalidates_cached_summary_even_without_pruned_rows() {
        let now = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let mut ledger = TokenLedger {
            limit: 2,
            ..Default::default()
        };
        for key in ["a", "b", "excluded"] {
            ledger.upsert("file", key, now, None, counts(100, 0, 20));
        }
        ledger.remove_owner("file");
        assert_eq!(ledger.records, 0);
        assert_eq!(
            ledger.summarize(now, TokenUsageStatus::Ready, &[]).status,
            TokenUsageStatus::Partial
        );
        let cached_revision = ledger.summary_cache.borrow().as_ref().unwrap().revision;
        let expired = now + chrono::Duration::days(40);
        ledger.prune(retention_start(expired));
        assert_ne!(ledger.revision, cached_revision);
        assert_eq!(ledger.records, 0);
        assert_eq!(
            ledger
                .summarize(expired, TokenUsageStatus::Ready, &[])
                .status,
            TokenUsageStatus::Ready
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
        assert_eq!(summary.month.totals.total(), Some(33));
    }
}
