use super::evidence::{aggregate, Evidence, IDLE_EVIDENCE_SECONDS};
use crate::domain::{
    activity::{ActivityState, ProviderActivity},
    types::ProviderId,
};
use crate::infrastructure::providers::paths;
use chrono::{DateTime, Duration, Utc};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

const DISCOVERY_SECONDS: i64 = 30;
const ANTIGRAVITY_RUNNING_SECONDS: i64 = 45;
const ANTIGRAVITY_RECENT_SECONDS: i64 = 120;
const MAX_TAIL_BYTES: u64 = 128 * 1024;
const MAX_SESSIONS: usize = 64;
const MAX_DISCOVERY_ENTRIES: usize = 4096;

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
            let invalidations = self.evidence.ingest(provider, &bytes[start..]);
            for _ in 0..invalidations {
                crate::infrastructure::providers::quota::grok_fetch::clear_cache();
            }
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
    grok_root: Option<PathBuf>,
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
            grok_root: paths::grok_home().map(|p| p.join("sessions")),
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
        for provider in [ProviderId::Codex, ProviderId::Claude, ProviderId::Grok] {
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
                ProviderId::Grok => {
                    if let Some(root) = &self.grok_root {
                        collect_grok(root, 2, &mut budget, &mut candidates);
                    }
                }
                _ => {}
            }
            candidates.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));
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

fn collect_grok(
    root: &Path,
    depth: usize,
    budget: &mut usize,
    files: &mut Vec<(SystemTime, PathBuf)>,
) {
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
        if kind.is_dir() && depth > 0 {
            collect_grok(&entry.path(), depth - 1, budget, files);
        }
        if kind.is_file() && entry.file_name() == "events.jsonl" {
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

fn antigravity_state_in(
    gemini_dir: &Path,
    now: DateTime<Utc>,
) -> (ActivityState, Option<DateTime<Utc>>) {
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
                    if newest.is_none_or(|prev| dt > prev) {
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
    let state = if (0..=ANTIGRAVITY_RUNNING_SECONDS).contains(&age) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        "2026-10-02T00:00:10Z".parse().unwrap()
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
            grok_root: Some(dir.path().into()),
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
