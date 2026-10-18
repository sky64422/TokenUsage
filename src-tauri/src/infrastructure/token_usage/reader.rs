use crate::domain::token_usage::{TokenCounts, TokenLedger, TokenUsageSnapshot, TokenUsageStatus};
use chrono::{DateTime, Local};
use serde::Deserialize;
use std::{collections::{BTreeMap, VecDeque}, fs::{self, ReadDir}, io::{Read, Seek, SeekFrom}, path::PathBuf, time::SystemTime};

const DISCOVERY_ENTRIES: usize = 512;
const READ_BUDGET: usize = 8 * 1024 * 1024;
const MAX_LINE: usize = 1024 * 1024;
const MAX_FILES: usize = 4096;
const FILES_PER_PASS: usize = 64;
const PREFIX_BYTES: usize = 512;
const CHUNK_BYTES: usize = 64 * 1024;
const DISCOVERY_SECONDS: i64 = 30;
const RECENT_DATES_BEFORE_TODAY: i64 = 6;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct RawCounts {
    input_tokens: Option<u64>, cached_input_tokens: Option<u64>, cache_write_input_tokens: Option<u64>,
    output_tokens: Option<u64>, reasoning_output_tokens: Option<u64>, total_tokens: Option<u64>,
}
impl RawCounts {
    fn counts(&self) -> TokenCounts { TokenCounts { input: self.input_tokens, cache_read: self.cached_input_tokens,
        cache_write: self.cache_write_input_tokens, output: self.output_tokens, reasoning: self.reasoning_output_tokens } }
    fn valid(&self) -> bool {
        self.counts().valid() && self.total_tokens.zip(self.counts().total()).is_none_or(|(t, sum)| t == sum)
    }
    fn delta(&self, old: &Self) -> Option<TokenCounts> {
        fn diff(a: Option<u64>, b: Option<u64>) -> Option<Option<u64>> {
            match (a, b) { (Some(a), Some(b)) => Some(Some(a.checked_sub(b)?)), _ => Some(None) }
        }
        Some(TokenCounts { input: diff(self.input_tokens, old.input_tokens)?, cache_read: diff(self.cached_input_tokens, old.cached_input_tokens)?,
            cache_write: diff(self.cache_write_input_tokens, old.cache_write_input_tokens)?, output: diff(self.output_tokens, old.output_tokens)?,
            reasoning: diff(self.reasoning_output_tokens, old.reasoning_output_tokens)? })
    }
}

#[derive(Default, Deserialize)]
struct Info { total_token_usage: Option<RawCounts>, last_token_usage: Option<RawCounts> }
#[derive(Default, Deserialize)]
struct Payload {
    id: Option<String>, model: Option<String>, #[serde(rename="type")] kind: Option<String>, info: Option<Info>,
    forked_from_id: Option<String>, parent_thread_id: Option<String>,
}
#[derive(Deserialize)]
struct Record { #[serde(rename="type")] kind: String, timestamp: Option<String>, #[serde(default)] payload: Payload }

#[derive(Default)]
struct Cursor {
    offset: u64, pending: Vec<u8>, discarding: bool, prefix: Vec<u8>, created: Option<SystemTime>, modified: Option<SystemTime>,
    session: Option<String>, model: Option<String>, previous: Option<RawCounts>, inherited: bool,
    partial: bool, caught_up: bool,
}

pub struct TokenReader {
    root: PathBuf, queue: VecDeque<PathBuf>, directory: Option<ReadDir>, files: BTreeMap<PathBuf, Cursor>,
    ledger: TokenLedger, discovered: bool, limited: bool, discovery_error: bool,
    last_discovery: DateTime<Local>, file_turn: usize,
}

impl TokenReader {
    pub fn new(root: PathBuf) -> Self {
        Self { queue: VecDeque::from([root.clone()]), root, directory: None, files: BTreeMap::new(),
            ledger: TokenLedger::default(), discovered: false, limited: false, discovery_error: false,
            last_discovery: Local::now(), file_turn: 0 }
    }

    pub fn scan(&mut self, now: DateTime<Local>) {
        if self.discovered && (now - self.last_discovery).num_seconds() >= DISCOVERY_SECONDS {
            self.queue.push_back(self.root.clone()); self.discovered = false; self.discovery_error = false;
        }
        self.discover(now);
        let paths: Vec<_> = self.files.keys().cloned().collect();
        let mut budget = READ_BUDGET;
        let count = paths.len().min(FILES_PER_PASS);
        for index in 0..count {
            if budget == 0 { break; }
            let path = &paths[(self.file_turn + index) % paths.len()];
            let owner = path.to_string_lossy();
            let cursor = self.files.get_mut(path).expect("discovered file");
            if read_file(path, &owner, cursor, &mut self.ledger, &mut budget, now).is_err() {
                cursor.partial = true; cursor.caught_up = true;
            }
        }
        if !paths.is_empty() { self.file_turn = (self.file_turn + count) % paths.len(); }
        self.ledger.prune(now.date_naive() - chrono::Duration::days(RECENT_DATES_BEFORE_TODAY));
    }

    fn discover(&mut self, now: DateTime<Local>) {
        for _ in 0..DISCOVERY_ENTRIES {
            if self.directory.is_none() {
                let Some(dir) = self.queue.pop_front() else { self.discovered = true; self.last_discovery = now; break; };
                match fs::read_dir(&dir) {
                    Ok(entries) => self.directory = Some(entries),
                    Err(e) => { if e.kind() != std::io::ErrorKind::NotFound { self.discovery_error = true; } continue; }
                }
            }
            let Some(entry) = self.directory.as_mut().and_then(Iterator::next) else { self.directory = None; continue; };
            let Ok(entry) = entry else { self.discovery_error = true; continue; };
            let Ok(kind) = entry.file_type() else { self.discovery_error = true; continue; };
            if kind.is_symlink() { continue; }
            if kind.is_dir() {
                if self.queue.len() < MAX_FILES { self.queue.push_back(entry.path()); } else { self.limited = true; }
            } else if kind.is_file() && entry.path().extension().is_some_and(|e| e == "jsonl") {
                if self.files.len() < MAX_FILES { self.files.entry(entry.path()).or_default(); }
                else { self.limited = true; }
            }
        }
    }

    pub fn summary(&self, now: DateTime<Local>) -> TokenUsageSnapshot {
        let status = if self.discovery_error && self.files.is_empty() { TokenUsageStatus::Unavailable }
        else if self.limited || self.discovery_error || self.files.values().any(|f| f.partial) { TokenUsageStatus::Partial }
        else if !self.discovered || self.files.values().any(|f| !f.caught_up || !f.pending.is_empty() || f.discarding) { TokenUsageStatus::Loading }
        else { TokenUsageStatus::Ready };
        self.ledger.summarize(now, status)
    }
}

fn read_file(path: &PathBuf, owner: &str, c: &mut Cursor, ledger: &mut TokenLedger, budget: &mut usize, now: DateTime<Local>) -> std::io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() { return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)); }
    let mut file = fs::File::open(path)?;
    let meta = file.metadata()?;
    let mut prefix = vec![0; (meta.len() as usize).min(PREFIX_BYTES)];
    file.read_exact(&mut prefix)?;
    let changed = c.offset > meta.len() || (!c.prefix.is_empty() && c.prefix != prefix) || c.created != meta.created().ok()
        || (c.offset > 0 && c.offset == meta.len() && c.modified != meta.modified().ok());
    if changed { ledger.remove_owner(owner); *c = Cursor::default(); }
    c.prefix = prefix; c.created = meta.created().ok(); c.modified = meta.modified().ok();
    file.seek(SeekFrom::Start(c.offset))?;
    let take = (*budget).min(meta.len().saturating_sub(c.offset) as usize);
    let mut remaining = take;
    let mut bytes = [0u8; CHUNK_BYTES];
    while remaining > 0 {
        let n = file.read(&mut bytes[..remaining.min(CHUNK_BYTES)])?;
        if n == 0 { break; }
        c.offset += n as u64; *budget -= n; remaining -= n;
        for byte in &bytes[..n] {
            if *byte == b'\n' {
                if !c.discarding { let line = std::mem::take(&mut c.pending); ingest(&line, owner, c, ledger, now); }
                c.discarding = false;
            } else if !c.discarding {
                if c.pending.len() < MAX_LINE { c.pending.push(*byte); }
                else { c.pending.clear(); c.discarding = true; c.partial = true; }
            }
        }
    }
    c.caught_up = c.offset >= meta.len();
    Ok(())
}

fn ingest(line: &[u8], owner: &str, c: &mut Cursor, ledger: &mut TokenLedger, now: DateTime<Local>) {
    if line.iter().all(u8::is_ascii_whitespace) { return; }
    let Ok(record) = serde_json::from_slice::<Record>(line) else { c.partial = true; return; };
    let p = record.payload;
    match record.kind.as_str() {
        "session_meta" => {
            c.session = p.id.filter(|id| id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
            c.inherited = p.forked_from_id.is_some() || p.parent_thread_id.is_some();
        }
        "turn_context" => { c.model = p.model.filter(|m| m.len() <= 96 && !m.chars().any(char::is_control)); }
        "event_msg" if p.kind.as_deref() == Some("token_count") => {
            let Some(info) = p.info else { return; };
            let Some(total) = info.total_token_usage else { c.partial = true; return; };
            let Some(at) = record.timestamp.and_then(|s| DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.with_timezone(&Local)) else { c.partial = true; return; };
            let previous = c.previous.replace(total.clone());
            if !total.valid() { c.partial = true; return; }
            if previous.as_ref() == Some(&total) { return; }
            let counts = match previous {
                Some(prev) if prev.valid() => total.delta(&prev),
                _ if !c.inherited && info.last_token_usage.as_ref() == Some(&total) => Some(total.counts()),
                _ => None,
            };
            let Some(counts) = counts.filter(TokenCounts::valid) else { c.partial = true; return; };
            let Some(session) = &c.session else { c.partial = true; return; };
            if at <= now && at.date_naive() >= now.date_naive() - chrono::Duration::days(RECENT_DATES_BEFORE_TODAY) {
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

    fn now() -> DateTime<Local> { Local.with_ymd_and_hms(2026,10,18,12,0,0).unwrap() }
    fn meta(id: &str) -> String { format!("{}\n", json!({"type":"session_meta","payload":{"id":id}})) }
    fn context(model: &str) -> String { format!("{}\n", json!({"type":"turn_context","payload":{"model":model}})) }
    fn event(input: u64, output: u64) -> String {
        format!("{}\n", json!({"timestamp":now().to_rfc3339(),"type":"event_msg","payload":{"type":"token_count","info":{
            "total_token_usage":{"input_tokens":input,"cached_input_tokens":0,"output_tokens":output,"total_tokens":input+output},
            "last_token_usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":20,"total_tokens":120}}}}))
    }
    fn settle(r: &mut TokenReader) -> TokenUsageSnapshot { for _ in 0..30 { r.scan(now()); } r.summary(now()) }

    #[test]
    fn old_session_models_cumulative_reports_and_copy_are_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("2020/01/01"); std::fs::create_dir_all(&old).unwrap();
        let text = meta("session") + &context("alpha") + &event(100,20) + &context("beta") + &event(250,40) + &event(250,40);
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
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("session.jsonl");
        let text = meta("session") + &context("alpha") + &event(100,20);
        std::fs::write(&path, &text).unwrap();
        let mut reader = TokenReader::new(dir.path().into());
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
        use std::io::Write;
        let second = event(250,40); let split = second.len()/2;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(&second.as_bytes()[..split]).unwrap();
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
        f.write_all(&second.as_bytes()[split..]).unwrap();
        assert_eq!(settle(&mut reader).today.totals.total(), Some(290));
        std::fs::write(&path, meta("replacement") + &event(100,20)).unwrap();
        assert_eq!(settle(&mut reader).today.totals.total(), Some(120));
    }

    #[test]
    fn inherited_baseline_is_not_charged_and_bad_values_are_partial() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fork.jsonl"), meta("fork") + &event(1000,200) + &event(1100,220)).unwrap();
        let mut reader = TokenReader::new(dir.path().into()); let result = settle(&mut reader);
        assert_eq!(result.today.totals.total(), Some(120));
        assert_eq!(result.status, TokenUsageStatus::Partial);
    }
}
