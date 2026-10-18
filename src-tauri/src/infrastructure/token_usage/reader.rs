use crate::domain::token_usage::{
    retention_start, TokenCounts, TokenLedger, TokenUsageSnapshot, TokenUsageStatus,
};
use chrono::{DateTime, Local, NaiveDate};
use serde::Deserialize;
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, VecDeque},
    fs::{self, ReadDir},
    hash::{Hash, Hasher},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    time::SystemTime,
};

const DISCOVERY_ENTRIES: usize = 512;
const READ_BUDGET: usize = 8 * 1024 * 1024;
const HOT_READ_BUDGET: usize = 2 * 1024 * 1024;
const HOT_FILES_PER_PASS: usize = 8;
const MAX_PENDING_BYTES: usize = READ_BUDGET;
const FILE_READ_BUDGET: usize = 1024 * 1024;
const MAX_LINE: usize = 1024 * 1024;
const MAX_FILES: usize = 4096;
const FILES_PER_PASS: usize = 64;
const PREFIX_BYTES: usize = 512;
const CHUNK_BYTES: usize = 64 * 1024;
const DISCOVERY_SECONDS: i64 = 30;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct RawCounts {
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_output_tokens: Option<u64>,
    total_tokens: Option<u64>,
}
impl RawCounts {
    fn counts(&self) -> TokenCounts {
        TokenCounts {
            input: self.input_tokens,
            cache_read: self.cached_input_tokens,
            cache_write: self.cache_write_input_tokens,
            output: self.output_tokens,
            reasoning: self.reasoning_output_tokens,
        }
    }
    fn valid(&self) -> bool {
        self.counts().valid()
            && self
                .total_tokens
                .zip(self.counts().total())
                .is_none_or(|(t, sum)| t == sum)
    }
    fn delta(&self, old: &Self) -> Option<TokenCounts> {
        fn diff(a: Option<u64>, b: Option<u64>) -> Option<Option<u64>> {
            match (a, b) {
                (Some(a), Some(b)) => Some(Some(a.checked_sub(b)?)),
                _ => Some(None),
            }
        }
        Some(TokenCounts {
            input: diff(self.input_tokens, old.input_tokens)?,
            cache_read: diff(self.cached_input_tokens, old.cached_input_tokens)?,
            cache_write: diff(self.cache_write_input_tokens, old.cache_write_input_tokens)?,
            output: diff(self.output_tokens, old.output_tokens)?,
            reasoning: diff(self.reasoning_output_tokens, old.reasoning_output_tokens)?,
        })
    }
}

#[derive(Default, Deserialize)]
struct Info {
    total_token_usage: Option<RawCounts>,
    last_token_usage: Option<RawCounts>,
}
#[derive(Default, Deserialize)]
struct Payload {
    id: Option<String>,
    model: Option<String>,
    to_model: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    info: Option<Info>,
    forked_from_id: Option<String>,
}
#[derive(Deserialize)]
struct Record {
    #[serde(rename = "type")]
    kind: String,
    timestamp: Option<String>,
    #[serde(default)]
    payload: Payload,
}

#[derive(Default)]
struct Cursor {
    offset: u64,
    pending: Vec<u8>,
    discarding: bool,
    prefix: Option<(usize, u64)>,
    boundary: Option<(usize, u64)>,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
    priority: Option<SystemTime>,
    session: Option<String>,
    model: Option<String>,
    previous: Option<RawCounts>,
    inherited: bool,
    partial: bool,
    caught_up: bool,
}
impl Cursor {
    fn set_model(&mut self, model: Option<String>) {
        self.model =
            model.filter(|m| !m.is_empty() && m.len() <= 96 && !m.chars().any(char::is_control));
    }
}

pub struct TokenReader {
    root: PathBuf,
    queue: VecDeque<PathBuf>,
    directory: Option<ReadDir>,
    files: BTreeMap<PathBuf, Cursor>,
    ledger: TokenLedger,
    discovered: bool,
    limited: bool,
    discovery_error: bool,
    last_discovery: DateTime<Local>,
    file_turn: usize,
    coverage_gap: Option<NaiveDate>,
}

impl TokenReader {
    pub fn new(root: PathBuf) -> Self {
        Self {
            queue: VecDeque::from([root.clone()]),
            root,
            directory: None,
            files: BTreeMap::new(),
            ledger: TokenLedger::default(),
            discovered: false,
            limited: false,
            discovery_error: false,
            last_discovery: Local::now(),
            file_turn: 0,
            coverage_gap: None,
        }
    }

    pub fn scan(&mut self, now: DateTime<Local>) {
        if self.discovered && (now - self.last_discovery).num_seconds() >= DISCOVERY_SECONDS {
            self.queue.push_back(self.root.clone());
            self.discovered = false;
            self.discovery_error = false;
            self.limited = false;
        }
        self.discover(now);
        // Keep rotation independent of mtimes so active files cannot reorder and
        // repeatedly skip older files. The recent lane also bootstraps unread
        // sessions so current usage appears before the historical backlog drains.
        let paths: Vec<_> = self.files.keys().cloned().collect();
        let mut hot = paths.clone();
        hot.sort_by(|a, b| {
            self.files[b]
                .priority
                .cmp(&self.files[a].priority)
                .then(a.cmp(b))
        });
        hot.truncate(HOT_FILES_PER_PASS);
        let mut pending_bytes = self.files.values().map(|c| c.pending.capacity()).sum();
        let mut hot_budget = HOT_READ_BUDGET;
        let mut hot_attempted = 0;
        for path in &hot {
            if hot_budget == 0 {
                break;
            }
            self.visit(path, &mut hot_budget, &mut pending_bytes, now);
            hot_attempted += 1;
        }
        // A candidate whose reserved budget ran out still needs its background turn.
        hot.truncate(hot_attempted);
        let mut budget = READ_BUDGET - HOT_READ_BUDGET;
        let count = paths.len().min(FILES_PER_PASS - hot.len());
        let mut attempted = 0;
        for index in 0..count {
            if budget == 0 {
                break;
            }
            let path = &paths[(self.file_turn + index) % paths.len()];
            if !hot.contains(path) {
                self.visit(path, &mut budget, &mut pending_bytes, now);
            }
            attempted += 1;
        }
        if !paths.is_empty() {
            self.file_turn = (self.file_turn + attempted) % paths.len();
        }
        let oldest = retention_start(now);
        self.ledger.prune(oldest);
        if self.coverage_gap.is_some_and(|date| date < oldest) {
            self.coverage_gap = None;
        }
    }

    fn visit(
        &mut self,
        path: &PathBuf,
        budget: &mut usize,
        pending_bytes: &mut usize,
        now: DateTime<Local>,
    ) {
        if *budget == 0 {
            return;
        }
        let Some(cursor) = self.files.get_mut(path) else {
            return;
        };
        let allowance = (*budget).min(FILE_READ_BUDGET);
        let mut remaining = allowance;
        let result = read_file(
            path,
            &path.to_string_lossy(),
            cursor,
            &mut self.ledger,
            &mut remaining,
            pending_bytes,
            now,
        );
        *budget -= allowance - remaining;
        if let Err(error) = result {
            if error.kind() == std::io::ErrorKind::NotFound {
                // Known numeric records remain until retention expiry; only the
                // missing cursor and its transient transcript buffer are retired.
                if cursor.partial
                    || !cursor.caught_up
                    || !cursor.pending.is_empty()
                    || cursor.discarding
                {
                    // A single date retains the coverage warning without retaining
                    // deleted-file paths, transcript bytes or unbounded tombstones.
                    let date = now.date_naive();
                    self.coverage_gap = Some(self.coverage_gap.map_or(date, |old| old.max(date)));
                }
                *pending_bytes -= cursor.pending.capacity();
                self.files.remove(path);
            } else {
                cursor.partial = true;
                cursor.caught_up = true;
            }
        }
    }

    fn discover(&mut self, now: DateTime<Local>) {
        for _ in 0..DISCOVERY_ENTRIES {
            if self.directory.is_none() {
                let Some(dir) = self.queue.pop_front() else {
                    self.discovered = true;
                    self.last_discovery = now;
                    break;
                };
                match fs::read_dir(&dir) {
                    Ok(entries) => self.directory = Some(entries),
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::NotFound {
                            self.discovery_error = true;
                        }
                        continue;
                    }
                }
            }
            let Some(entry) = self.directory.as_mut().and_then(Iterator::next) else {
                self.directory = None;
                continue;
            };
            let Ok(entry) = entry else {
                self.discovery_error = true;
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                self.discovery_error = true;
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if self.queue.len() < MAX_FILES {
                    self.queue.push_back(entry.path());
                } else {
                    self.limited = true;
                }
            } else if kind.is_file() && entry.path().extension().is_some_and(|e| e == "jsonl") {
                let priority = entry.metadata().ok().and_then(|m| m.modified().ok());
                if let Some(cursor) = self.files.get_mut(&entry.path()) {
                    cursor.priority = priority;
                } else if self.files.len() < MAX_FILES {
                    self.files.entry(entry.path()).or_insert_with(|| Cursor {
                        priority,
                        ..Default::default()
                    });
                } else {
                    self.limited = true;
                }
            }
        }
    }

    pub fn summary(
        &self,
        now: DateTime<Local>,
        windows: &[crate::domain::types::UsageWindow],
    ) -> TokenUsageSnapshot {
        let status = if self.discovery_error && self.files.is_empty() {
            TokenUsageStatus::Unavailable
        } else if !self.discovered
            || self
                .files
                .values()
                .any(|f| !f.caught_up || !f.pending.is_empty())
        {
            TokenUsageStatus::Loading
        } else if self.limited
            || self.discovery_error
            || self.files.values().any(|f| f.partial)
            || self
                .coverage_gap
                .is_some_and(|date| date >= retention_start(now))
        {
            TokenUsageStatus::Partial
        } else {
            TokenUsageStatus::Ready
        };
        self.ledger.summarize(now, status, windows)
    }
}

fn read_file(
    path: &PathBuf,
    owner: &str,
    c: &mut Cursor,
    ledger: &mut TokenLedger,
    budget: &mut usize,
    pending_bytes: &mut usize,
    now: DateTime<Local>,
) -> std::io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    let mut file = fs::File::open(path)?;
    let meta = file.metadata()?;
    let mut sample = [0; PREFIX_BYTES];
    let prefix_len = (meta.len() as usize).min(PREFIX_BYTES);
    file.read_exact(&mut sample[..prefix_len])?;
    let prefix = fingerprint(&sample[..prefix_len]);
    let prefix_changed = c
        .prefix
        .is_some_and(|(len, previous)| len > prefix_len || fingerprint(&sample[..len]) != previous);
    let mut boundary_changed = false;
    if let Some((len, previous)) = c.boundary.filter(|_| c.offset <= meta.len()) {
        file.seek(SeekFrom::Start(c.offset - len as u64))?;
        file.read_exact(&mut sample[..len])?;
        boundary_changed = fingerprint(&sample[..len]) != previous;
    }
    let changed = boundary_changed
        || c.offset > meta.len()
        || prefix_changed
        || c.created != meta.created().ok()
        || (c.offset > 0 && c.offset == meta.len() && c.modified != meta.modified().ok());
    if changed {
        ledger.remove_owner(owner);
        *pending_bytes -= c.pending.capacity();
        *c = Cursor {
            priority: meta.modified().ok(),
            ..Default::default()
        };
    }
    c.prefix = Some((prefix_len, prefix));
    c.created = meta.created().ok();
    c.modified = meta.modified().ok();
    c.priority = c.modified;
    file.seek(SeekFrom::Start(c.offset))?;
    let take = (*budget).min(meta.len().saturating_sub(c.offset) as usize);
    let mut remaining = take;
    let mut bytes = [0u8; CHUNK_BYTES];
    while remaining > 0 {
        let n = file.read(&mut bytes[..remaining.min(CHUNK_BYTES)])?;
        if n == 0 {
            break;
        }
        c.offset += n as u64;
        *budget -= n;
        remaining -= n;
        for byte in &bytes[..n] {
            if *byte == b'\n' {
                if !c.discarding {
                    let line = std::mem::take(&mut c.pending);
                    *pending_bytes -= line.capacity();
                    ingest(&line, owner, c, ledger, now);
                }
                c.discarding = false;
            } else if !c.discarding {
                if c.pending.len() == c.pending.capacity() && c.pending.len() < MAX_LINE {
                    let capacity = c.pending.capacity();
                    let growth = capacity
                        .max(CHUNK_BYTES)
                        .min(MAX_LINE - capacity)
                        .min(MAX_PENDING_BYTES.saturating_sub(*pending_bytes));
                    c.pending.reserve_exact(growth);
                    *pending_bytes += c.pending.capacity() - capacity;
                }
                if c.pending.len() >= MAX_LINE
                    || c.pending.len() == c.pending.capacity()
                    || *pending_bytes > MAX_PENDING_BYTES
                {
                    *pending_bytes -= c.pending.capacity();
                    c.pending = Vec::new();
                    c.discarding = true;
                    c.partial = true;
                } else {
                    c.pending.push(*byte);
                }
            }
        }
    }
    c.caught_up = c.offset >= meta.len();
    let len = (c.offset as usize).min(PREFIX_BYTES);
    file.seek(SeekFrom::Start(c.offset - len as u64))?;
    file.read_exact(&mut sample[..len])?;
    c.boundary = Some((len, fingerprint(&sample[..len])));
    Ok(())
}

fn fingerprint(bytes: &[u8]) -> u64 {
    // Rewrite checks need equality evidence, never retained transcript content.
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    hash.finish()
}

fn ingest(
    line: &[u8],
    owner: &str,
    c: &mut Cursor,
    ledger: &mut TokenLedger,
    now: DateTime<Local>,
) {
    if line.iter().all(u8::is_ascii_whitespace) {
        return;
    }
    let Ok(record) = serde_json::from_slice::<Record>(line) else {
        c.partial = true;
        return;
    };
    let p = record.payload;
    match record.kind.as_str() {
        "session_meta" => {
            c.session = p.id.filter(|id| {
                id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            });
            if let (Some(session), Some(parent)) = (
                &c.session,
                p.forked_from_id.filter(|id| {
                    id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                }),
            ) {
                ledger.register_lineage(session, &parent);
                c.inherited = true;
            }
        }
        "turn_context" => c.set_model(p.model),
        "response_item" if p.model.is_some() => c.set_model(p.model),
        "event_msg" if p.kind.as_deref() == Some("model_reroute") => c.set_model(p.to_model),
        "event_msg" if p.kind.as_deref() == Some("token_count") => {
            let Some(info) = p.info else {
                return;
            };
            let Some(total) = info.total_token_usage else {
                c.partial = true;
                return;
            };
            let Some(at) = record
                .timestamp
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Local))
            else {
                c.partial = true;
                return;
            };
            let previous = c.previous.replace(total.clone());
            if !total.valid() {
                c.partial = true;
                return;
            }
            if previous.as_ref() == Some(&total) {
                return;
            }
            let counts = match previous {
                Some(prev) if prev.valid() => total.delta(&prev),
                _ if !c.inherited && info.last_token_usage.as_ref() == Some(&total) => {
                    Some(total.counts())
                }
                _ => None,
            };
            let Some(counts) = counts.filter(TokenCounts::valid) else {
                c.partial = true;
                return;
            };
            let Some(session) = &c.session else {
                c.partial = true;
                return;
            };
            if at <= now && at.date_naive() >= retention_start(now) {
                // Logical session, timestamp and cumulative counters survive copied files.
                let key = format!("{session}:{}:{total:?}", at.to_rfc3339());
                ledger.upsert(owner, &key, at, c.model.clone(), counts);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap()
    }
    fn meta(id: &str) -> String {
        format!("{}\n", json!({"type":"session_meta","payload":{"id":id}}))
    }
    fn context(model: &str) -> String {
        format!(
            "{}\n",
            json!({"type":"turn_context","payload":{"model":model}})
        )
    }
    fn event(input: u64, output: u64) -> String {
        format!(
            "{}\n",
            json!({"timestamp":now().to_rfc3339(),"type":"event_msg","payload":{"type":"token_count","info":{
            "total_token_usage":{"input_tokens":input,"cached_input_tokens":0,"output_tokens":output,"total_tokens":input+output},
            "last_token_usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":20,"total_tokens":120}}}})
        )
    }
    fn settle(r: &mut TokenReader) -> TokenUsageSnapshot {
        for _ in 0..30 {
            r.scan(now());
        }
        r.summary(now(), &[])
    }

    #[test]
    fn missing_files_release_cursor_slots_but_keep_known_usage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(&path, meta("session") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
        fs::remove_file(path).unwrap();
        reader.scan(now());
        assert!(reader.files.is_empty());
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(120));
        assert_eq!(reader.summary(now(), &[]).status, TokenUsageStatus::Ready);
        let expired = now() + chrono::Duration::days(40);
        reader.scan(expired);
        assert_eq!(reader.summary(expired, &[]).month.totals.total(), None);
    }

    #[test]
    fn unreadable_existing_path_preserves_cursor_and_known_usage_as_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(&path, meta("session") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        settle(&mut reader);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        reader.scan(now());
        assert_eq!(reader.files.len(), 1);
        let summary = reader.summary(now(), &[]);
        assert_eq!(summary.status, TokenUsageStatus::Partial);
        assert_eq!(summary.today.totals.total(), Some(120));
    }

    #[test]
    fn rewrite_detection_retains_fingerprints_instead_of_transcript_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let marker = "sensitive-conversation-marker";
        let line = format!(
            "{}\n",
            json!({"type":"response_item","payload":{"content":marker}})
        );
        fs::write(
            dir.path().join("session.jsonl"),
            meta("session") + &line + &event(100, 20),
        )
        .unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        settle(&mut reader);
        let cursor = reader.files.values().next().unwrap();
        let retained = format!(
            "{:?}{:?}{:?}",
            cursor.prefix, cursor.boundary, cursor.pending
        );
        let marker_bytes = format!("{:?}", marker.as_bytes());
        assert!(!retained.contains(marker_bytes.trim_matches(['[', ']'])));
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(120));
    }

    #[test]
    fn oversized_unterminated_lines_release_their_pending_allocation() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("session.jsonl"), vec![b'x'; MAX_LINE + 1]).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        reader.scan(now());
        reader.scan(now());
        let cursor = reader.files.values().next().unwrap();
        assert!(cursor.discarding);
        assert_eq!(cursor.pending.capacity(), 0);
        assert!(cursor.partial);
    }

    #[test]
    fn unfinished_lines_have_an_aggregate_memory_bound_and_report_partial_coverage() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..12 {
            fs::write(
                dir.path().join(format!("{index}.jsonl")),
                vec![b'x'; MAX_LINE],
            )
            .unwrap();
        }
        let mut reader = TokenReader::new(dir.path().into());
        for _ in 0..4 {
            reader.scan(now());
        }
        let retained: usize = reader.files.values().map(|c| c.pending.capacity()).sum();
        assert!(retained <= READ_BUDGET, "retained {retained} pending bytes");
        assert!(reader.files.values().any(|c| c.partial));
    }

    #[test]
    fn recent_caught_up_session_is_visited_on_every_pass_without_starving_rotation() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        for index in 0..FILES_PER_PASS * 3 {
            let path = dir.path().join(format!("{index:04}.jsonl"));
            fs::write(&path, "").unwrap();
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH)
                .unwrap();
        }
        let active = dir.path().join("active.jsonl");
        fs::write(&active, meta("active") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        settle(&mut reader);
        reader.file_turn = FILES_PER_PASS;
        fs::OpenOptions::new()
            .append(true)
            .open(active)
            .unwrap()
            .write_all(event(250, 40).as_bytes())
            .unwrap();
        reader.scan(now());
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(290));
        let old = dir.path().join("0100.jsonl");
        fs::write(&old, meta("old") + &event(100, 20)).unwrap();
        for _ in 0..4 {
            reader.scan(now());
        }
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(410));
    }

    #[test]
    fn review_hot_budget_exhaustion_keeps_unvisited_candidates_in_background_rotation() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = ["a.jsonl", "b.jsonl", "c.jsonl"]
            .into_iter()
            .map(|name| dir.path().join(name))
            .collect();
        for path in &paths[..2] {
            fs::write(path, "").unwrap();
        }
        fs::write(&paths[2], meta("third-session") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        settle(&mut reader);
        fs::OpenOptions::new()
            .append(true)
            .open(&paths[2])
            .unwrap()
            .write_all(event(250, 40).as_bytes())
            .unwrap();
        for _ in 0..3 {
            for path in &paths[..2] {
                fs::OpenOptions::new()
                    .append(true)
                    .open(path)
                    .unwrap()
                    .write_all(&vec![b'\n'; FILE_READ_BUDGET])
                    .unwrap();
            }
            for (index, path) in paths.iter().enumerate() {
                fs::File::options()
                    .write(true)
                    .open(path)
                    .unwrap()
                    .set_modified(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs((3 - index) as u64),
                    )
                    .unwrap();
                reader.files.get_mut(path).unwrap().priority = Some(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs((3 - index) as u64),
                );
            }
            reader.scan(now());
        }
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(290));
        assert!(reader.files[&paths[2]].caught_up);
    }

    #[test]
    fn newest_unread_session_is_loaded_before_the_historic_background_backlog() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..8 {
            let path = dir.path().join(format!("archive-{index:02}.jsonl"));
            fs::write(&path, vec![b'\n'; FILE_READ_BUDGET + 1]).unwrap();
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH)
                .unwrap();
        }
        let recent = dir.path().join("z-current.jsonl");
        fs::write(&recent, meta("current-session") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        reader.scan(now());
        let summary = reader.summary(now(), &[]);
        assert_eq!(summary.today.totals.total(), Some(120));
        assert!(reader.files[&recent].caught_up);
        assert!(reader.files.values().any(|cursor| !cursor.caught_up));
    }

    #[test]
    fn review_deleted_incomplete_or_invalid_tails_keep_partial_coverage_until_expiry() {
        for tail in ["{\"type\":\"event_msg\"", "invalid-json\n"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("session.jsonl");
            fs::write(&path, meta("session") + &event(100, 20) + tail).unwrap();
            let mut reader = TokenReader::new(dir.path().into());
            let before = settle(&mut reader);
            assert_eq!(before.today.totals.total(), Some(120));
            assert!(matches!(
                before.status,
                TokenUsageStatus::Loading | TokenUsageStatus::Partial
            ));
            fs::remove_file(path).unwrap();
            reader.scan(now());
            assert!(reader.files.is_empty());
            let after = reader.summary(now(), &[]);
            assert_eq!(after.today.totals.total(), Some(120));
            assert_eq!(after.status, TokenUsageStatus::Partial);
            let expired = now() + chrono::Duration::days(40);
            reader.scan(expired);
            let after_expiry = reader.summary(expired, &[]);
            assert_eq!(after_expiry.status, TokenUsageStatus::Ready);
            assert_eq!(after_expiry.month.totals.total(), None);
        }
    }

    #[test]
    fn old_session_models_cumulative_reports_and_copy_are_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("2020/01/01");
        std::fs::create_dir_all(&old).unwrap();
        let text = meta("session")
            + &context("alpha")
            + &event(100, 20)
            + &context("beta")
            + &event(250, 40)
            + &event(250, 40);
        std::fs::write(old.join("session.jsonl"), &text).unwrap();
        std::fs::write(dir.path().join("copy.jsonl"), &text).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        let result = settle(&mut reader);
        assert_eq!(result.today.totals.total(), Some(290));
        assert_eq!(result.today.models.len(), 2);
        assert_eq!(result.status, TokenUsageStatus::Ready);
    }

    #[test]
    fn append_partial_line_and_truncate_replace_previous_contributions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let text = meta("session") + &context("alpha") + &event(100, 20);
        std::fs::write(&path, &text).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
        use std::io::Write;
        let second = event(250, 40);
        let split = second.len() / 2;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(&second.as_bytes()[..split]).unwrap();
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
        f.write_all(&second.as_bytes()[split..]).unwrap();
        assert_eq!(settle(&mut reader).today.totals.total(), Some(290));
        std::fs::write(&path, meta("replacement") + &event(100, 20)).unwrap();
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
    }

    #[test]
    fn inherited_baseline_is_not_charged_and_bad_values_are_partial() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("fork.jsonl"),
            meta("fork") + &event(1000, 200) + &event(1100, 220),
        )
        .unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        let result = settle(&mut reader);
        assert_eq!(result.today.totals.total(), Some(120));
        assert_eq!(result.status, TokenUsageStatus::Partial);
    }

    #[test]
    fn model_reroute_uses_the_reported_destination_model() {
        let dir = tempfile::tempdir().unwrap();
        let reroute = format!(
            "{}\n",
            json!({"type":"event_msg","payload":{"type":"model_reroute","to_model":"actual-model"}})
        );
        std::fs::write(
            dir.path().join("session.jsonl"),
            meta("session") + &context("requested-model") + &reroute + &event(100, 20),
        )
        .unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(
            settle(&mut reader).today.models[0].model.as_deref(),
            Some("actual-model")
        );
    }

    #[test]
    fn appending_to_a_short_file_reads_only_new_bytes() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(&path, meta("session") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        settle(&mut reader);
        let old_offset = reader.files[&path].offset;
        assert!(old_offset < PREFIX_BYTES as u64);
        let filler = "{\"type\":\"other\"}\n";
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(
                filler
                    .repeat(FILE_READ_BUDGET / filler.len() + 1)
                    .as_bytes(),
            )
            .unwrap();
        reader.scan(now());
        assert_eq!(
            reader.files[&path].offset,
            old_offset + FILE_READ_BUDGET as u64
        );
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(120));
    }

    #[test]
    fn same_size_rewrite_and_discovery_progress_do_not_leave_stale_totals() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        std::fs::write(&path, meta("session") + &context("alpha") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(
            settle(&mut reader).today.models[0].model.as_deref(),
            Some("alpha")
        );
        std::fs::write(&path, meta("session") + &context("bravo") + &event(100, 20)).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH)
            .unwrap();
        assert_eq!(
            settle(&mut reader).today.models[0].model.as_deref(),
            Some("bravo")
        );
    }

    #[test]
    fn inaccessible_root_and_oversized_lines_are_explicit_and_content_is_not_retained() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-directory");
        std::fs::write(&path, "x").unwrap();
        let mut reader = TokenReader::new(path);
        assert_eq!(settle(&mut reader).status, TokenUsageStatus::Unavailable);
        let content = "sensitive-conversation".repeat(MAX_LINE / 10);
        let line = format!(
            "{}\n",
            json!({"type":"response_item","payload":{"content":content}})
        );
        std::fs::write(
            dir.path().join("session.jsonl"),
            meta("private-session") + &line + &event(100, 20),
        )
        .unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        let summary = settle(&mut reader);
        assert_eq!(summary.status, TokenUsageStatus::Partial);
        assert_eq!(summary.today.totals.total(), Some(120));
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("sensitive-conversation"));
        assert!(!json.contains("private-session"));
        assert!(!json.contains("session.jsonl"));
    }

    #[test]
    fn discovery_budget_is_resumable_and_repeated_copies_are_not_extra_usage() {
        let dir = tempfile::tempdir().unwrap();
        let text = meta("same-session") + &event(100, 20);
        for i in 0..DISCOVERY_ENTRIES + 1 {
            std::fs::write(dir.path().join(format!("{i}.jsonl")), &text).unwrap();
        }
        let mut reader = TokenReader::new(dir.path().into());
        reader.scan(now());
        assert_eq!(reader.summary(now(), &[]).status, TokenUsageStatus::Loading);
        let summary = settle(&mut reader);
        assert_eq!(summary.status, TokenUsageStatus::Ready);
        assert_eq!(summary.today.totals.total(), Some(120));
    }

    #[test]
    fn forked_history_is_deduplicated_across_multiple_inherited_events() {
        let dir = tempfile::tempdir().unwrap();
        let history = context("alpha") + &event(100, 20) + &event(200, 40);
        std::fs::write(dir.path().join("parent.jsonl"), meta("parent") + &history).unwrap();
        let fork_meta = format!(
            "{}\n",
            json!({"type":"session_meta","payload":{"id":"fork","forked_from_id":"parent"}})
        );
        std::fs::write(
            dir.path().join("fork.jsonl"),
            fork_meta + &history + &event(300, 60),
        )
        .unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(settle(&mut reader).today.totals.total(), Some(360));
    }

    #[test]
    fn a_retimestamped_fork_baseline_is_not_new_usage() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("parent.jsonl"),
            meta("parent") + &event(100, 20),
        )
        .unwrap();
        let fork_meta = format!(
            "{}\n",
            json!({"type":"session_meta","payload":{"id":"fork","forked_from_id":"parent"}})
        );
        let retimed = event(100, 20).replace(
            &now().to_rfc3339(),
            &(now() - chrono::Duration::seconds(1)).to_rfc3339(),
        );
        std::fs::write(
            dir.path().join("fork.jsonl"),
            fork_meta + &retimed + &event(200, 40),
        )
        .unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(settle(&mut reader).today.totals.total(), Some(240));
    }

    #[test]
    fn a_large_backlog_does_not_starve_another_session() {
        let dir = tempfile::tempdir().unwrap();
        let filler = "{\"type\":\"other\"}\n";
        let big = dir.path().join("a.jsonl");
        std::fs::write(&big, filler.repeat(READ_BUDGET / filler.len() + 1)).unwrap();
        std::fs::write(dir.path().join("z.jsonl"), meta("recent") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        reader.scan(now());
        assert_eq!(reader.summary(now(), &[]).today.totals.total(), Some(120));
    }

    #[test]
    fn a_larger_rewrite_with_the_same_header_replaces_old_contributions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let prefix = meta("session")
            + &format!(
                "{}\n",
                json!({"type":"response_item","payload":{"content":"x".repeat(PREFIX_BYTES*2)}})
            );
        std::fs::write(&path, prefix.clone() + &context("alpha") + &event(100, 20)).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(
            settle(&mut reader).today.models[0].model.as_deref(),
            Some("alpha")
        );
        std::fs::write(&path, prefix + &context("bravo") + &event(100, 20) + "\n").unwrap();
        assert_eq!(
            settle(&mut reader).today.models[0].model.as_deref(),
            Some("bravo")
        );
    }
}
