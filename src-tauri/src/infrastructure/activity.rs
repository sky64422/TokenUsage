//! Read-only CLI lifecycle hints, isolated from vendor quota. Transcript content is
//! skipped by serde and never retained in the monitor, emitted, or logged.
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::providers::paths;
use crate::domain::{
    activity::{ActivityState, ProviderActivity},
    types::ProviderId,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const DISCOVERY_SECONDS: i64 = 30;
const CODEX_RUNNING_SECONDS: i64 = 120;
const CLAUDE_RECENT_SECONDS: i64 = 15;
const ANTIGRAVITY_RUNNING_SECONDS: i64 = 45;
const ANTIGRAVITY_RECENT_SECONDS: i64 = 120;
const IDLE_EVIDENCE_SECONDS: i64 = 86_400;
const MAX_TAIL_BYTES: u64 = 128 * 1024;
const MAX_SESSIONS: usize = 64;
const MAX_DISCOVERY_ENTRIES: usize = 4096;
const SAMPLE_SECONDS: u64 = 2;
const ACTIVITY_EVENT: &str = "provider-activity";

struct ActivityCache(Arc<Mutex<Vec<ProviderActivity>>>);

pub fn get(app: &AppHandle) -> Vec<ProviderActivity> {
    app.state::<ActivityCache>()
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub fn start(app: &AppHandle) {
    let unknown = ProviderId::all()
        .into_iter()
        .map(|provider_id| ProviderActivity {
            provider_id,
            state: ActivityState::Unknown,
            observed_at: None,
        })
        .collect();
    let cache = Arc::new(Mutex::new(unknown));
    if !app.manage(ActivityCache(Arc::clone(&cache))) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut monitor = ActivityMonitor::new();
        loop {
            let next = monitor.sample(Utc::now());
            let changed = {
                let mut previous = cache
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if *previous == next {
                    false
                } else {
                    *previous = next.clone();
                    true
                }
            };
            if changed && app.emit(ACTIVITY_EVENT, &next).is_err() {
                eprintln!("Activity update could not be delivered");
            }
            std::thread::sleep(std::time::Duration::from_secs(SAMPLE_SECONDS));
        }
    });
}

#[derive(Default)]
struct Evidence {
    last: Option<DateTime<Utc>>,
    state: Option<ActivityState>,
}

// Deliberately omit content, prompt, message text, paths, identities and usage.
#[derive(Deserialize)]
struct Record {
    timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    kind: String,
    payload: Option<Payload>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    role: Option<String>,
    stop_reason: Option<String>,
}

impl Evidence {
    fn ingest(&mut self, provider: ProviderId, bytes: &[u8]) {
        // Ignore the incomplete final line while another process appends it.
        for line in bytes
            .split_inclusive(|b| *b == b'\n')
            .filter(|line| line.ends_with(b"\n"))
        {
            let Ok(record) = serde_json::from_slice::<Record>(line) else {
                continue;
            };
            if self.last.is_some_and(|last| record.timestamp < last) {
                continue;
            }
            let next = match provider {
                ProviderId::Codex if record.kind == "event_msg" => {
                    match record.payload.and_then(|payload| payload.kind).as_deref() {
                        Some("task_started") => Some(ActivityState::Running),
                        Some("task_complete" | "turn_aborted") => Some(ActivityState::Idle),
                        // Structural progress renews an observed, unmatched start only.
                        Some(
                            "agent_message" | "agent_reasoning" | "exec_command_begin"
                            | "exec_command_end" | "item_started" | "item_completed",
                        ) if self.state == Some(ActivityState::Running) => self.state,
                        _ => None,
                    }
                }
                ProviderId::Claude => match record.message {
                    Some(message)
                        if (record.kind == "user" && message.role.as_deref() == Some("user"))
                            || (record.kind == "assistant"
                                && message.role.as_deref() == Some("assistant")) =>
                    {
                        Some(
                            if matches!(
                                message.stop_reason.as_deref(),
                                Some("end_turn" | "stop_sequence" | "max_tokens")
                            ) {
                                ActivityState::Idle
                            } else {
                                ActivityState::Recent
                            },
                        )
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(state) = next {
                self.state = Some(state);
                self.last = Some(record.timestamp);
            }
        }
    }

    fn state(&self, now: DateTime<Utc>) -> ActivityState {
        let (Some(last), Some(state)) = (self.last, self.state) else {
            return ActivityState::Unknown;
        };
        let age = now.signed_duration_since(last).num_seconds();
        let ttl = match state {
            ActivityState::Running => CODEX_RUNNING_SECONDS,
            ActivityState::Recent => CLAUDE_RECENT_SECONDS,
            ActivityState::Idle => IDLE_EVIDENCE_SECONDS,
            ActivityState::Unknown => return ActivityState::Unknown,
        };
        if age < 0 || age > ttl {
            ActivityState::Unknown
        } else {
            state
        }
    }
}

#[derive(Default)]
struct TrackedFile {
    signature: Option<(u64, SystemTime)>,
    evidence: Evidence,
}

impl TrackedFile {
    fn refresh(&mut self, path: &Path, provider: ProviderId) {
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::File::open(path)?;
            let metadata = file.metadata()?;
            let signature = (metadata.len(), metadata.modified()?);
            if self.signature == Some(signature) {
                return Ok(());
            }
            if self
                .signature
                .is_some_and(|previous| signature.0 <= previous.0)
            {
                self.evidence = Evidence::default();
            }
            let offset = metadata.len().saturating_sub(MAX_TAIL_BYTES);
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = Vec::new();
            file.take(MAX_TAIL_BYTES).read_to_end(&mut bytes)?;
            let start = if offset > 0 {
                bytes
                    .iter()
                    .position(|b| *b == b'\n')
                    .map_or(bytes.len(), |i| i + 1)
            } else {
                0
            };
            self.evidence.ingest(provider, &bytes[start..]);
            self.signature = Some(signature);
            Ok(())
        })();
        if result.is_err() {
            // Unreadable is unknown, never an idle or busy fallback.
            self.evidence = Evidence::default();
            self.signature = None;
        }
    }
}

pub struct ActivityMonitor {
    codex_root: Option<PathBuf>,
    claude_root: Option<PathBuf>,
    files: HashMap<(ProviderId, PathBuf), TrackedFile>,
    last_discovery: Option<DateTime<Utc>>,
    disabled: bool,
}

impl Default for ActivityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivityMonitor {
    pub fn new() -> Self {
        Self {
            codex_root: paths::codex_home().map(|p| p.join("sessions")),
            claude_root: paths::claude_home().map(|p| p.join("projects")),
            files: HashMap::new(),
            last_discovery: None,
            disabled: std::env::var("TOKENUSAGE_SKIP_DIRECT_QUOTA").as_deref() == Ok("1"),
        }
    }

    /// Call on a blocking worker. Discovery runs at most every 30s; unchanged
    /// files are not parsed again, but TTLs are evaluated on every sample.
    pub fn sample(&mut self, now: DateTime<Utc>) -> Vec<ProviderActivity> {
        if !self.disabled {
            if self
                .last_discovery
                .is_none_or(|last| now < last || now - last >= Duration::seconds(DISCOVERY_SECONDS))
            {
                self.discover(now);
                self.last_discovery = Some(now);
            }
            for ((provider, path), tracked) in &mut self.files {
                tracked.refresh(path, *provider);
            }
        }
        ProviderId::all()
            .into_iter()
            .map(|provider| {
                if provider == ProviderId::Agy {
                    let (state, observed_at) = if self.disabled {
                        (ActivityState::Unknown, None)
                    } else {
                        antigravity_state(now)
                    };
                    ProviderActivity {
                        provider_id: ProviderId::Agy,
                        state,
                        observed_at,
                    }
                } else {
                    aggregate(
                        provider,
                        self.files
                            .iter()
                            .filter(|((id, _), _)| *id == provider)
                            .map(|(_, file)| &file.evidence),
                        now,
                    )
                }
            })
            .collect()
    }

    fn discover(&mut self, now: DateTime<Utc>) {
        let mut selected = Vec::new();
        for provider in [ProviderId::Codex, ProviderId::Claude] {
            let mut candidates = Vec::new();
            let mut budget = MAX_DISCOVERY_ENTRIES;
            match provider {
                ProviderId::Codex => {
                    if let Some(root) = &self.codex_root {
                        // Fixed date directories avoid walking years of session history.
                        // CLI date directories may follow local time across UTC midnight.
                        for day in [now, now + Duration::days(1), now - Duration::days(1)] {
                            collect(
                                &root.join(day.format("%Y/%m/%d").to_string()),
                                0,
                                &mut budget,
                                &mut candidates,
                            );
                        }
                    }
                }
                ProviderId::Claude => {
                    if let Some(root) = &self.claude_root {
                        collect(root, 1, &mut budget, &mut candidates);
                    }
                }
                _ => {}
            }
            candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            selected.extend(
                candidates
                    .into_iter()
                    .take(MAX_SESSIONS)
                    .map(|(_, path)| (provider, path)),
            );
        }
        // Keep recently observed sessions across date rollover/discovery caps.
        self.files.retain(|key, file| {
            selected.contains(key)
                || matches!(
                    file.evidence.state(now),
                    ActivityState::Running | ActivityState::Recent
                )
        });
        for key in selected {
            self.files.entry(key).or_default();
        }
    }
}

fn collect(root: &Path, depth: usize, budget: &mut usize, files: &mut Vec<(SystemTime, PathBuf)>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(entry) = entry else { continue };
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // Never follow symlinks into unrelated trees.
        if kind.is_dir() && depth > 0 {
            collect(&entry.path(), depth - 1, budget, files);
        }
        if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl") {
            if let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) {
                files.push((modified, entry.path()));
            }
        }
    }
}

fn antigravity_state(now: DateTime<Utc>) -> (ActivityState, Option<DateTime<Utc>>) {
    let Some(home) = dirs::home_dir() else {
        return (ActivityState::Unknown, None);
    };
    antigravity_state_in(&home.join(".gemini"), now)
}

fn antigravity_state_in(gemini_dir: &Path, now: DateTime<Utc>) -> (ActivityState, Option<DateTime<Utc>>) {
    let Ok(rd) = fs::read_dir(gemini_dir) else {
        return (ActivityState::Unknown, None);
    };
    let mut newest: Option<DateTime<Utc>> = None;
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("antigravity") {
            continue;
        }
        let brain_dir = entry.path().join("brain");
        let Ok(brain_rd) = fs::read_dir(brain_dir) else {
            continue;
        };
        for conv in brain_rd.flatten() {
            let transcript = conv
                .path()
                .join(".system_generated")
                .join("logs")
                .join("transcript.jsonl");
            if let Ok(meta) = fs::metadata(&transcript) {
                if let Ok(modified) = meta.modified() {
                    let dt: DateTime<Utc> = modified.into();
                    if newest.map_or(true, |prev| dt > prev) {
                        newest = Some(dt);
                    }
                }
            }
        }
    }
    let Some(at) = newest else {
        return (ActivityState::Unknown, None);
    };
    let age = now.signed_duration_since(at).num_seconds();
    let state = if age >= 0 && age <= ANTIGRAVITY_RUNNING_SECONDS {
        ActivityState::Running
    } else if age > ANTIGRAVITY_RUNNING_SECONDS && age <= ANTIGRAVITY_RECENT_SECONDS {
        ActivityState::Recent
    } else if age > ANTIGRAVITY_RECENT_SECONDS && age <= IDLE_EVIDENCE_SECONDS {
        ActivityState::Idle
    } else {
        ActivityState::Unknown
    };
    (state, Some(at))
}

fn aggregate<'a>(
    provider: ProviderId,
    evidence: impl Iterator<Item = &'a Evidence>,
    now: DateTime<Utc>,
) -> ProviderActivity {
    let mut result = ProviderActivity {
        provider_id: provider,
        state: ActivityState::Unknown,
        observed_at: None,
    };
    let priority = |state| match state {
        ActivityState::Running => 3,
        ActivityState::Recent => 2,
        ActivityState::Idle => 1,
        ActivityState::Unknown => 0,
    };
    for item in evidence {
        let state = item.state(now);
        if priority(state) > priority(result.state)
            || (state != ActivityState::Unknown
                && state == result.state
                && item.last > result.observed_at)
        {
            result.state = state;
            result.observed_at = item.last;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::ProviderId;

    fn now() -> DateTime<Utc> {
        "2026-10-02T00:00:10Z".parse().unwrap()
    }

    #[test]
    fn newer_idle_session_does_not_hide_another_running_session() {
        let active = Evidence {
            last: Some(now() - Duration::seconds(5)),
            state: Some(ActivityState::Running),
        };
        let idle = Evidence {
            last: Some(now()),
            state: Some(ActivityState::Idle),
        };
        let result = aggregate(ProviderId::Codex, [&idle, &active].into_iter(), now());
        assert_eq!(result.state, ActivityState::Running);
    }

    #[test]
    fn cached_file_expires_without_another_write_and_unreadable_becomes_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(&path, b"{\"timestamp\":\"2026-10-02T00:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\"}}\n").unwrap();
        let mut tracked = TrackedFile::default();
        tracked.refresh(&path, ProviderId::Codex);
        assert_eq!(tracked.evidence.state(now()), ActivityState::Running);
        tracked.refresh(&path, ProviderId::Codex);
        assert_eq!(
            tracked.evidence.state(now() + Duration::minutes(5)),
            ActivityState::Unknown
        );
        fs::remove_file(path.clone()).unwrap();
        tracked.refresh(&path, ProviderId::Codex);
        assert_eq!(tracked.evidence.state(now()), ActivityState::Unknown);
    }

    #[test]
    fn preview_monitor_never_reads_configured_transcripts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("session.jsonl"), b"sensitive content").unwrap();
        let mut monitor = ActivityMonitor {
            codex_root: Some(dir.path().into()),
            claude_root: Some(dir.path().into()),
            files: HashMap::new(),
            last_discovery: None,
            disabled: true,
        };
        assert!(monitor
            .sample(now())
            .iter()
            .all(|item| item.state == ActivityState::Unknown));
        assert!(monitor.last_discovery.is_none());
    }

    #[test]
    fn explicit_codex_start_is_running_and_completion_stops_it() {
        let mut evidence = Evidence::default();
        evidence.ingest(ProviderId::Codex, br#"{"timestamp":"2026-10-02T00:00:00Z","type":"event_msg","payload":{"type":"task_started"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Running);
        evidence.ingest(ProviderId::Codex, br#"{"timestamp":"2026-10-02T00:00:05Z","type":"event_msg","payload":{"type":"task_complete"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Idle);
    }

    #[test]
    fn completion_with_same_timestamp_as_start_still_stops_activity() {
        let mut evidence = Evidence::default();
        evidence.ingest(ProviderId::Codex, br#"{"timestamp":"2026-10-02T00:00:00Z","type":"event_msg","payload":{"type":"task_started"}}
{"timestamp":"2026-10-02T00:00:00Z","type":"event_msg","payload":{"type":"task_complete"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Idle);
    }

    #[test]
    fn aborted_and_stale_tasks_do_not_keep_spinning() {
        let mut evidence = Evidence::default();
        evidence.ingest(ProviderId::Codex, br#"{"timestamp":"2026-10-01T00:00:00Z","type":"event_msg","payload":{"type":"task_started"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Unknown);
        evidence.ingest(ProviderId::Codex, br#"{"timestamp":"2026-10-02T00:00:00Z","type":"event_msg","payload":{"type":"turn_aborted"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Idle);
    }

    #[test]
    fn claude_is_only_recent_and_end_turn_is_idle() {
        let mut evidence = Evidence::default();
        evidence.ingest(ProviderId::Claude, br#"{"timestamp":"2026-10-02T00:00:00Z","type":"user","message":{"role":"user","content":"not retained"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Recent);
        assert_eq!(
            evidence.state(now() + chrono::Duration::minutes(5)),
            ActivityState::Unknown
        );
        evidence.ingest(ProviderId::Claude, br#"{"timestamp":"2026-10-02T00:00:05Z","type":"assistant","message":{"role":"assistant","stop_reason":"end_turn"}}
"#);
        assert_eq!(evidence.state(now()), ActivityState::Idle);
    }

    #[test]
    fn malformed_unknown_future_and_partial_records_are_not_activity() {
        for bytes in [b"not json\n".as_slice(), br#"{"type":"summary"}
"#, br#"{"timestamp":"2099-01-01T00:00:00Z","type":"event_msg","payload":{"type":"task_started"}}
"#, br#"{"timestamp":"2026-10-02T00:00:00Z","type":"event_msg","payload":{"type":"task_started"}}"#] {
            let mut evidence = Evidence::default();
            evidence.ingest(ProviderId::Codex, bytes);
            assert_eq!(evidence.state(now()), ActivityState::Unknown);
        }
    }

    #[test]
    fn antigravity_activity_detects_running_recent_and_idle() {
        let dir = tempfile::tempdir().unwrap();
        let logs_dir = dir
            .path()
            .join("antigravity-cli")
            .join("brain")
            .join("conv-1")
            .join(".system_generated")
            .join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let transcript = logs_dir.join("transcript.jsonl");
        fs::write(&transcript, b"{\"type\":\"done\"}\n").unwrap();

        // Fresh write is running
        let t_now = Utc::now();
        let (state, at) = antigravity_state_in(dir.path(), t_now);
        assert_eq!(state, ActivityState::Running);
        assert!(at.is_some());

        // Evaluated 60s in the future is recent
        let (state_recent, _) = antigravity_state_in(dir.path(), t_now + Duration::seconds(60));
        assert_eq!(state_recent, ActivityState::Recent);

        // Evaluated 300s in the future is idle
        let (state_idle, _) = antigravity_state_in(dir.path(), t_now + Duration::seconds(300));
        assert_eq!(state_idle, ActivityState::Idle);
    }
}
