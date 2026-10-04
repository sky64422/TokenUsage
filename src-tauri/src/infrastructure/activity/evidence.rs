use crate::domain::{
    activity::{ActivityState, ProviderActivity},
    types::ProviderId,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

const CODEX_RUNNING_SECONDS: i64 = 120;
const GROK_RUNNING_SECONDS: i64 = 120;
const CLAUDE_RECENT_SECONDS: i64 = 15;
pub(super) const IDLE_EVIDENCE_SECONDS: i64 = 86_400;

#[derive(Default)]
pub(super) struct Evidence {
    pub(super) last: Option<DateTime<Utc>>,
    pub(super) state: Option<ActivityState>,
}

// Deliberately omit content, prompt, message text, paths, identities and usage.
#[derive(Deserialize)]
struct Record {
    #[serde(default)]
    timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    ts: Option<DateTime<Utc>>,
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
    pub(super) fn ingest(&mut self, provider: ProviderId, bytes: &[u8]) -> usize {
        let mut invalidations = 0;
        // Ignore the incomplete final line while another process appends it.
        for line in bytes
            .split_inclusive(|b| *b == b'\n')
            .filter(|line| line.ends_with(b"\n"))
        {
            let Ok(record) = serde_json::from_slice::<Record>(line) else {
                continue;
            };
            let record_time = record.timestamp.or(record.ts);
            let Some(record_time) = record_time else {
                continue;
            };
            if self.last.is_some_and(|last| record_time < last) {
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
                ProviderId::Grok => match record.kind.as_str() {
                    "turn_started" | "loop_started" | "phase_changed" | "tool_started"
                    | "first_token" => Some(ActivityState::Running),
                    "turn_ended" => {
                        invalidations += 1;
                        Some(ActivityState::Idle)
                    }
                    "tool_completed" if self.state == Some(ActivityState::Running) => self.state,
                    _ => None,
                },
                _ => None,
            };
            if let Some(state) = next {
                self.state = Some(state);
                self.last = Some(record_time);
            }
        }
        invalidations
    }

    pub(super) fn state(&self, now: DateTime<Utc>) -> ActivityState {
        let (Some(last), Some(state)) = (self.last, self.state) else {
            return ActivityState::Unknown;
        };
        let age = now.signed_duration_since(last).num_seconds();
        let ttl = match state {
            ActivityState::Running => GROK_RUNNING_SECONDS.max(CODEX_RUNNING_SECONDS),
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

pub(super) fn aggregate<'a>(
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
    use chrono::Duration;

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
    fn grok_turn_started_is_running_and_turn_ended_is_idle() {
        let mut evidence = Evidence::default();
        evidence.ingest(
            ProviderId::Grok,
            br#"{"ts":"2026-10-02T00:00:00Z","type":"turn_started","session_id":"s1"}
"#,
        );
        assert_eq!(evidence.state(now()), ActivityState::Running);
        evidence.ingest(
            ProviderId::Grok,
            br#"{"ts":"2026-10-02T00:00:05Z","type":"phase_changed","phase":"streaming_reasoning"}
"#,
        );
        assert_eq!(evidence.state(now()), ActivityState::Running);
        evidence.ingest(
            ProviderId::Grok,
            br#"{"ts":"2026-10-02T00:00:10Z","type":"turn_ended","outcome":"completed"}
"#,
        );
        assert_eq!(evidence.state(now()), ActivityState::Idle);
    }
    #[test]
    fn grok_invalidation_reports_only_accepted_complete_records() {
        let mut evidence = Evidence::default();
        let records = b"{\"ts\":\"2026-10-02T00:00:05Z\",\"type\":\"turn_started\"}\n{\"ts\":\"2026-10-02T00:00:04Z\",\"type\":\"turn_ended\"}\n{\"ts\":\"2026-10-02T00:00:05Z\",\"type\":\"turn_ended\"}\n{\"ts\":\"2026-10-02T00:00:06Z\",\"type\":\"turn_ended\"}\n{\"ts\":\"2026-10-02T00:00:07Z\",\"type\":\"turn_ended\"}";
        assert_eq!(evidence.ingest(ProviderId::Grok, records), 2);
        assert_eq!(evidence.state(now()), ActivityState::Idle);
        // Re-reading equal-time evidence remains accepted, preserving invalidation policy.
        assert_eq!(evidence.ingest(ProviderId::Grok, records), 1);
        assert_eq!(evidence.ingest(ProviderId::Codex, records), 0);
    }
}
