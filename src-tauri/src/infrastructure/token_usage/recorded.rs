//! Exact usage metadata from provider records; transcript content is never deserialized.
use crate::domain::token_usage::{retention_start, TokenCounts, TokenLedger};
use crate::domain::types::ProviderId;
use chrono::{DateTime, Local};
use serde::Deserialize;
use std::collections::BTreeMap;

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_MODEL_BYTES: usize = 96;
const MAX_MODELS_PER_TURN: usize = 64;
const GROK_SESSION_UPDATE: &str = "_x.ai/session/update";

#[derive(Deserialize)]
struct Header {
    #[serde(rename = "type")]
    kind: Option<String>,
    method: Option<String>,
}

pub(super) fn ingest(
    provider: ProviderId,
    line: &[u8],
    scope: &str,
    owner: &str,
    ledger: &mut TokenLedger,
    now: DateTime<Local>,
) -> Result<(), ()> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(());
    }
    let header: Header = serde_json::from_slice(line).map_err(|_| ())?;
    match provider {
        ProviderId::Agy if header.kind.as_deref() == Some("PLANNER_RESPONSE") => {
            agy(line, scope, owner, ledger, now)
        }
        ProviderId::Claude if header.kind.as_deref() == Some("assistant") => {
            claude(line, owner, ledger, now)
        }
        ProviderId::Grok if header.method.as_deref() == Some(GROK_SESSION_UPDATE) => {
            grok(line, owner, ledger, now)
        }
        _ => Ok(()),
    }
}

fn identifier(value: &str) -> Result<&str, ()> {
    if !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        Ok(value)
    } else {
        Err(())
    }
}

fn model(value: Option<String>) -> Result<Option<String>, ()> {
    match value {
        Some(value)
            if value.trim().is_empty()
                || value.len() > MAX_MODEL_BYTES
                || value.chars().any(char::is_control) =>
        {
            Err(())
        }
        value => Ok(value),
    }
}

fn timestamp(value: Option<&str>) -> Result<DateTime<Local>, ()> {
    DateTime::parse_from_rfc3339(value.ok_or(())?)
        .map(|at| at.with_timezone(&Local))
        .map_err(|_| ())
}

fn checked_sum(values: &[Option<u64>]) -> Result<Option<u64>, ()> {
    let mut sum = 0u64;
    let mut complete = true;
    for value in values {
        match value {
            Some(value) => sum = sum.checked_add(*value).ok_or(())?,
            None => complete = false,
        }
    }
    Ok(complete.then_some(sum))
}

fn valid_counts(counts: TokenCounts) -> Result<TokenCounts, ()> {
    let cache = checked_sum(&[counts.cache_read, counts.cache_write])?;
    if !counts.valid()
        || counts
            .input
            .zip(counts.cache_write)
            .is_some_and(|(input, write)| write > input)
        || counts.input.zip(cache).is_some_and(|(input, c)| c > input)
        || [
            counts.input,
            counts.cache_read,
            counts.cache_write,
            counts.output,
            counts.reasoning,
        ]
        .iter()
        .all(Option::is_none)
    {
        Err(())
    } else {
        Ok(counts)
    }
}

#[derive(Deserialize)]
struct AgyHeader {
    status: Option<String>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct AgyRecord {
    step_index: Option<u64>,
    created_at: Option<String>,
    input_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    output_tokens: Option<u64>,
    model: Option<String>,
    model_name: Option<String>,
}

fn agy(
    line: &[u8],
    scope: &str,
    owner: &str,
    ledger: &mut TokenLedger,
    now: DateTime<Local>,
) -> Result<(), ()> {
    let header: AgyHeader = serde_json::from_slice(line).map_err(|_| ())?;
    if header.status.as_deref() != Some("DONE") || header.source.as_deref() != Some("MODEL") {
        return Ok(());
    }
    let record: AgyRecord = serde_json::from_slice(line).map_err(|_| ())?;
    let at = timestamp(record.created_at.as_deref())?;
    if at.date_naive() < retention_start(now) {
        return Ok(());
    }
    let key = format!("{}:{}", identifier(scope)?, record.step_index.ok_or(())?);
    if record.model.is_some() && record.model_name.is_some() && record.model != record.model_name {
        return Err(());
    }
    let model = model(record.model.or(record.model_name))?;
    // AGY's input excludes cache reads; normalize to the shared inclusive input.
    let counts = valid_counts(TokenCounts {
        input: checked_sum(&[record.input_tokens, record.cache_read_tokens])?,
        cache_read: record.cache_read_tokens,
        output: record.output_tokens,
        ..Default::default()
    })?;
    ledger.upsert(owner, &key, at, model, counts);
    Ok(())
}

#[derive(Deserialize)]
struct ClaudeHeader {
    #[serde(rename = "isApiErrorMessage")]
    api_error: Option<bool>,
}

#[derive(Deserialize)]
struct ClaudeRecord {
    timestamp: Option<String>,
    #[serde(rename = "sessionId", alias = "session_id")]
    session: Option<String>,
    #[serde(rename = "requestId")]
    request: Option<String>,
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

fn claude(
    line: &[u8],
    owner: &str,
    ledger: &mut TokenLedger,
    now: DateTime<Local>,
) -> Result<(), ()> {
    let header: ClaudeHeader = serde_json::from_slice(line).map_err(|_| ())?;
    if header.api_error == Some(true) {
        return Ok(());
    }
    let record: ClaudeRecord = serde_json::from_slice(line).map_err(|_| ())?;
    let message = record.message.ok_or(())?;
    if matches!(message.model.as_deref(), Some("<synthetic>" | "synthetic")) {
        return Ok(());
    }
    let at = timestamp(record.timestamp.as_deref())?;
    if at.date_naive() < retention_start(now) {
        return Ok(());
    }
    let session = identifier(record.session.as_deref().ok_or(())?)?;
    // A response can have several transcript chunks with distinct UUIDs.
    let response = identifier(
        message
            .id
            .as_deref()
            .or(record.request.as_deref())
            .ok_or(())?,
    )?;
    let usage = message.usage.ok_or(())?;
    let counts = valid_counts(TokenCounts {
        input: checked_sum(&[
            usage.input_tokens,
            usage.cache_read_input_tokens,
            usage.cache_creation_input_tokens,
        ])?,
        cache_read: usage.cache_read_input_tokens,
        cache_write: usage.cache_creation_input_tokens,
        output: usage.output_tokens,
        reasoning: None,
    })?;
    ledger.upsert(
        owner,
        &format!("{session}:{response}"),
        at,
        model(message.model)?,
        counts,
    );
    Ok(())
}

#[derive(Deserialize)]
struct GrokEnvelope {
    timestamp: Option<i64>,
    params: Option<GrokParams>,
}

#[derive(Deserialize)]
struct GrokParams {
    #[serde(rename = "sessionId")]
    session: Option<String>,
    update: Option<GrokUpdate>,
}

#[derive(Deserialize)]
struct GrokUpdate {
    prompt_id: Option<String>,
    usage: Option<GrokUsage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    cached_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    model_usage: Option<BTreeMap<String, GrokUsage>>,
}

impl GrokUsage {
    fn counts(&self) -> Result<TokenCounts, ()> {
        let counts = valid_counts(TokenCounts {
            input: self.input_tokens,
            cache_read: self.cached_read_tokens,
            cache_write: self.cache_creation_tokens,
            output: self.output_tokens,
            reasoning: self.reasoning_tokens,
        })?;
        if self
            .total_tokens
            .zip(counts.total())
            .is_some_and(|(total, sum)| total != sum)
        {
            return Err(());
        }
        Ok(counts)
    }
}

fn grok(
    line: &[u8],
    owner: &str,
    ledger: &mut TokenLedger,
    now: DateTime<Local>,
) -> Result<(), ()> {
    // Deserialize the discriminator separately so content updates need no usage schema.
    #[derive(Deserialize)]
    struct UpdateHeader {
        params: Option<ParamsHeader>,
    }
    #[derive(Deserialize)]
    struct ParamsHeader {
        update: Option<KindHeader>,
    }
    #[derive(Deserialize)]
    struct KindHeader {
        #[serde(rename = "sessionUpdate")]
        kind: Option<String>,
    }
    let header: UpdateHeader = serde_json::from_slice(line).map_err(|_| ())?;
    if header
        .params
        .and_then(|p| p.update)
        .and_then(|u| u.kind)
        .as_deref()
        != Some("turn_completed")
    {
        return Ok(());
    }
    let record: GrokEnvelope = serde_json::from_slice(line).map_err(|_| ())?;
    let at = DateTime::from_timestamp(record.timestamp.ok_or(())?, 0)
        .ok_or(())?
        .with_timezone(&Local);
    if at.date_naive() < retention_start(now) {
        return Ok(());
    }
    let params = record.params.ok_or(())?;
    let session = identifier(params.session.as_deref().ok_or(())?)?;
    let update = params.update.ok_or(())?;
    let prompt = identifier(update.prompt_id.as_deref().ok_or(())?)?;
    let usage = update.usage.ok_or(())?;
    let has_models = usage
        .model_usage
        .as_ref()
        .is_some_and(|models| !models.is_empty());
    let totals = if has_models
        && [
            usage.input_tokens,
            usage.output_tokens,
            usage.cached_read_tokens,
            usage.cache_creation_tokens,
            usage.reasoning_tokens,
        ]
        .iter()
        .all(Option::is_none)
    {
        TokenCounts::default()
    } else {
        usage.counts()?
    };
    let mut rows = Vec::new();
    if let Some(models) = usage
        .model_usage
        .as_ref()
        .filter(|models| !models.is_empty())
    {
        if models.len() > MAX_MODELS_PER_TURN {
            return Err(());
        }
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for (name, usage) in models {
            if usage.model_usage.is_some() {
                return Err(());
            }
            let counts = usage.counts()?;
            inputs.push(counts.input);
            outputs.push(counts.output);
            rows.push((model(Some(name.clone()))?, counts));
        }
        let input_sum = checked_sum(&inputs)?;
        let output_sum = checked_sum(&outputs)?;
        for (sum, total) in [
            (input_sum, totals.input),
            (output_sum, totals.output),
            (checked_sum(&[input_sum, output_sum])?, usage.total_tokens),
        ] {
            if sum.zip(total).is_some_and(|(sum, total)| sum != total) {
                return Err(());
            }
        }
    } else {
        rows.push((None, totals));
    }
    // Replace only after full validation: later model maps supersede unknown rows.
    ledger.replace_event(owner, &format!("{session}:{prompt}"), at, rows);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::token_usage::{TokenUsageSnapshot, TokenUsageStatus};
    use chrono::TimeZone;
    use serde_json::{json, Value};

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 19, 12, 0, 0).unwrap()
    }

    fn add(provider: ProviderId, value: &Value, ledger: &mut TokenLedger) -> Result<(), ()> {
        ingest(
            provider,
            &serde_json::to_vec(value).unwrap(),
            "conversation",
            "file",
            ledger,
            now(),
        )
    }

    fn snapshot(ledger: &TokenLedger) -> TokenUsageSnapshot {
        ledger.summarize(now(), TokenUsageStatus::Ready, &[])
    }

    fn agy_record() -> Value {
        json!({"type":"PLANNER_RESPONSE", "status":"DONE", "source":"MODEL", "step_index":1,
            "created_at":now().to_rfc3339(), "input_tokens":10, "cache_read_tokens":90,
            "output_tokens":20, "text":"not retained"})
    }

    fn claude_record() -> Value {
        json!({"type":"assistant", "sessionId":"session", "timestamp":now().to_rfc3339(),
            "uuid":"chunk-one", "requestId":"request", "message":{"id":"msg_response",
            "model":"claude-sonnet", "usage":{"input_tokens":10,"cache_read_input_tokens":80,
            "cache_creation_input_tokens":10,"output_tokens":20}, "content":"not retained"}})
    }

    fn grok_record() -> Value {
        json!({"timestamp":now().timestamp(), "method":GROK_SESSION_UPDATE,
            "params":{"sessionId":"session","_meta":{"totalTokens":999999},
            "update":{"sessionUpdate":"turn_completed","prompt_id":"prompt-one", "usage":{
            "inputTokens":100,"outputTokens":20,"totalTokens":120,"cachedReadTokens":80,
            "cacheCreationTokens":10,"reasoningTokens":5,"modelCalls":1,
            "modelUsage":{"grok-4.7":{"inputTokens":100,"outputTokens":20,"totalTokens":120,
            "cachedReadTokens":80,"cacheCreationTokens":10,"reasoningTokens":5}}}}}})
    }

    #[test]
    fn agy_cache_is_additive_and_steps_and_copies_are_deduplicated() {
        let mut ledger = TokenLedger::default();
        let mut record = agy_record();
        add(ProviderId::Agy, &record, &mut ledger).unwrap();
        add(ProviderId::Agy, &record, &mut ledger).unwrap();
        ingest(
            ProviderId::Agy,
            &serde_json::to_vec(&record).unwrap(),
            "conversation",
            "copy",
            &mut ledger,
            now(),
        )
        .unwrap();
        record["step_index"] = json!(2);
        add(ProviderId::Agy, &record, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.totals.input, Some(200));
        assert_eq!(period.totals.cache_read, Some(180));
        assert_eq!(period.totals.total(), Some(240));
        assert_eq!(period.totals.cache_write, None);
        assert_eq!(period.totals.reasoning, None);
        assert_eq!(period.models[0].model, None);
    }

    #[test]
    fn agy_uses_only_explicit_models_and_rejects_conflicting_fields() {
        let mut ledger = TokenLedger::default();
        let mut record = agy_record();
        record["model_name"] = json!("explicit-model");
        add(ProviderId::Agy, &record, &mut ledger).unwrap();
        assert_eq!(
            snapshot(&ledger).today.models[0].model.as_deref(),
            Some("explicit-model")
        );
        record["model"] = json!("different-model");
        assert_eq!(add(ProviderId::Agy, &record, &mut ledger), Err(()));
    }

    #[test]
    fn claude_response_id_deduplicates_chunks_and_normalizes_all_input_categories() {
        let mut ledger = TokenLedger::default();
        let mut record = claude_record();
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        record["uuid"] = json!("different-chunk");
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.totals.input, Some(100));
        assert_eq!(period.totals.cache_read, Some(80));
        assert_eq!(period.totals.cache_write, Some(10));
        assert_eq!(period.totals.total(), Some(120));
        assert_eq!(period.totals.reasoning, None);
    }

    #[test]
    fn claude_request_fallback_and_session_alias_preserve_unknown_model() {
        let mut ledger = TokenLedger::default();
        let mut record = claude_record();
        record.as_object_mut().unwrap().remove("sessionId");
        record["session_id"] = json!("session");
        record["message"]["id"] = Value::Null;
        record["message"]["model"] = Value::Null;
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        assert_eq!(snapshot(&ledger).today.totals.total(), Some(120));
        assert_eq!(snapshot(&ledger).today.models[0].model, None);
    }

    #[test]
    fn grok_turns_are_independent_and_not_cumulative() {
        let mut ledger = TokenLedger::default();
        let mut record = grok_record();
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        record["params"]["update"]["prompt_id"] = json!("prompt-two");
        let usage = &mut record["params"]["update"]["usage"];
        *usage = json!({"inputTokens":10,"outputTokens":2,"modelCalls":1,
            "modelUsage":{"grok-4.7":{"inputTokens":10,"outputTokens":2}}});
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.totals.input, Some(110));
        assert_eq!(period.totals.output, Some(22));
        assert_eq!(period.models.len(), 1);
    }

    #[test]
    fn grok_prefers_exact_model_rows_without_double_counting_turn_totals() {
        let mut ledger = TokenLedger::default();
        let mut record = grok_record();
        record["params"]["update"]["usage"] = json!({"inputTokens":100,"outputTokens":20,
            "modelUsage":{"grok-a":{"inputTokens":60,"outputTokens":15},
            "grok-b":{"inputTokens":40,"outputTokens":5}}});
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.models.len(), 2);
        assert_eq!(period.totals.total(), Some(120));
        record["params"]["update"]["usage"]["inputTokens"] = json!(101);
        record["params"]["update"]["prompt_id"] = json!("bad-turn");
        assert_eq!(add(ProviderId::Grok, &record, &mut ledger), Err(()));
        assert_eq!(snapshot(&ledger).today.totals.total(), Some(120));
    }

    #[test]
    fn grok_can_use_model_rows_when_top_level_counters_are_missing() {
        let mut ledger = TokenLedger::default();
        let mut record = grok_record();
        record["params"]["update"]["usage"] = json!({"modelUsage":{
            "grok-a":{"inputTokens":60,"outputTokens":15}}});
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        assert_eq!(snapshot(&ledger).today.totals.total(), Some(75));
        record["params"]["update"]["usage"]["totalTokens"] = json!(75);
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        record["params"]["update"]["usage"]["totalTokens"] = json!(76);
        assert_eq!(add(ProviderId::Grok, &record, &mut ledger), Err(()));
    }

    #[test]
    fn grok_without_model_map_uses_unknown_model_and_input_includes_cache() {
        let mut ledger = TokenLedger::default();
        let mut record = grok_record();
        record["params"]["update"]["usage"]
            .as_object_mut()
            .unwrap()
            .remove("modelUsage");
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.models[0].model, None);
        assert_eq!(period.totals.total(), Some(120));
        assert_eq!(period.totals.cache_read, Some(80));
        assert_eq!(period.totals.reasoning, Some(5));
    }

    #[test]
    fn grok_replaces_unknown_models_and_removes_models_absent_from_later_maps() {
        let mut ledger = TokenLedger::default();
        let mut record = grok_record();
        let exact = record.clone();
        record["params"]["update"]["usage"]
            .as_object_mut()
            .unwrap()
            .remove("modelUsage");
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        assert_eq!(snapshot(&ledger).today.models[0].model, None);
        add(ProviderId::Grok, &exact, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.models.len(), 1);
        assert_eq!(period.models[0].model.as_deref(), Some("grok-4.7"));
        assert_eq!(period.totals.total(), Some(120));

        record["params"]["update"]["usage"] = json!({"inputTokens":100,"outputTokens":20,
            "modelUsage":{"grok-a":{"inputTokens":60,"outputTokens":15},
            "grok-b":{"inputTokens":40,"outputTokens":5}}});
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        assert_eq!(snapshot(&ledger).today.models.len(), 2);
        record["params"]["update"]["usage"] = json!({"inputTokens":100,"outputTokens":20,
            "modelUsage":{"grok-a":{"inputTokens":100,"outputTokens":20}}});
        add(ProviderId::Grok, &record, &mut ledger).unwrap();
        let period = snapshot(&ledger).today;
        assert_eq!(period.models.len(), 1);
        assert_eq!(period.models[0].model.as_deref(), Some("grok-a"));
        assert_eq!(period.totals.total(), Some(120));

        record["params"]["update"]["usage"]["inputTokens"] = json!(101);
        assert_eq!(add(ProviderId::Grok, &record, &mut ledger), Err(()));
        assert_eq!(snapshot(&ledger).today, period);
    }

    #[test]
    fn grok_copies_with_different_model_versions_use_one_whole_turn() {
        for unknown_first in [false, true] {
            let mut ledger = TokenLedger::default();
            let exact = grok_record();
            let mut unknown = exact.clone();
            unknown["params"]["update"]["usage"]
                .as_object_mut()
                .unwrap()
                .remove("modelUsage");
            let records = if unknown_first {
                [(&unknown, "a-copy"), (&exact, "z-copy")]
            } else {
                [(&exact, "a-copy"), (&unknown, "z-copy")]
            };
            for (record, owner) in records {
                ingest(
                    ProviderId::Grok,
                    &serde_json::to_vec(record).unwrap(),
                    "conversation",
                    owner,
                    &mut ledger,
                    now(),
                )
                .unwrap();
            }
            let period = snapshot(&ledger).today;
            assert_eq!(period.models.len(), 1);
            assert_eq!(period.models[0].model.as_deref(), Some("grok-4.7"));
            assert_eq!(period.totals.total(), Some(120));
        }
    }

    #[test]
    fn missing_and_null_counters_are_unknown_instead_of_zero() {
        for provider in [ProviderId::Agy, ProviderId::Claude, ProviderId::Grok] {
            let mut ledger = TokenLedger::default();
            let mut record = match provider {
                ProviderId::Agy => agy_record(),
                ProviderId::Claude => claude_record(),
                _ => grok_record(),
            };
            match provider {
                ProviderId::Agy => record["cache_read_tokens"] = Value::Null,
                ProviderId::Claude => {
                    record["message"]["usage"]["cache_creation_input_tokens"] = Value::Null
                }
                _ => record["params"]["update"]["usage"] = json!({"outputTokens":20}),
            }
            add(provider, &record, &mut ledger).unwrap();
            assert_eq!(snapshot(&ledger).today.totals.input, None);
            assert_eq!(snapshot(&ledger).today.totals.output, Some(20));
            assert_eq!(snapshot(&ledger).today.totals.total(), None);
        }
    }

    #[test]
    fn invalid_subsets_and_arithmetic_overflow_are_rejected() {
        let mut ledger = TokenLedger::default();
        for usage in [
            json!({"inputTokens":10,"cachedReadTokens":11}),
            json!({"inputTokens":10,"cacheCreationTokens":11}),
            json!({"inputTokens":10,"cachedReadTokens":6,"cacheCreationTokens":5}),
            json!({"outputTokens":10,"reasoningTokens":11}),
            json!({"inputTokens":u64::MAX,"outputTokens":1}),
            json!({"inputTokens":10,"outputTokens":2,"totalTokens":13}),
            json!({"inputTokens":-1,"outputTokens":2}),
            json!({"inputTokens":1.5,"outputTokens":2}),
            json!({"inputTokens":null,"outputTokens":null}),
        ] {
            let mut record = grok_record();
            record["params"]["update"]["usage"] = usage;
            assert_eq!(add(ProviderId::Grok, &record, &mut ledger), Err(()));
        }
        let mut record = agy_record();
        record["input_tokens"] = json!(u64::MAX);
        assert_eq!(add(ProviderId::Agy, &record, &mut ledger), Err(()));
        let mut record = claude_record();
        record["message"]["usage"]["input_tokens"] = json!(u64::MAX);
        assert_eq!(add(ProviderId::Claude, &record, &mut ledger), Err(()));
        assert!(snapshot(&ledger).today.models.is_empty());
    }

    #[test]
    fn malformed_timestamps_identifiers_and_models_are_rejected() {
        let mut ledger = TokenLedger::default();
        for (provider, mut record, field) in [
            (ProviderId::Agy, agy_record(), "created_at"),
            (ProviderId::Claude, claude_record(), "timestamp"),
            (ProviderId::Grok, grok_record(), "timestamp"),
        ] {
            record[field] = json!("invalid-time");
            assert_eq!(add(provider, &record, &mut ledger), Err(()));
        }
        for id in [
            "",
            "session:collision",
            "bad\nidentifier",
            &"a".repeat(MAX_IDENTIFIER_BYTES + 1),
        ] {
            let mut record = claude_record();
            record["sessionId"] = json!(id);
            assert_eq!(add(ProviderId::Claude, &record, &mut ledger), Err(()));
            let mut record = grok_record();
            record["params"]["update"]["prompt_id"] = json!(id);
            assert_eq!(add(ProviderId::Grok, &record, &mut ledger), Err(()));
        }
        for name in ["", " \t", "bad\nmodel", &"a".repeat(MAX_MODEL_BYTES + 1)] {
            let mut record = claude_record();
            record["message"]["model"] = json!(name);
            assert_eq!(add(ProviderId::Claude, &record, &mut ledger), Err(()));
        }
    }

    #[test]
    fn irrelevant_wrappers_non_model_steps_and_error_messages_are_ignored() {
        let mut ledger = TokenLedger::default();
        for (provider, record) in [
            (
                ProviderId::Claude,
                json!({"type":"progress", "data":claude_record()}),
            ),
            (
                ProviderId::Claude,
                json!({"type":"user", "toolUseResult":{"usage":{"input_tokens":1000}}}),
            ),
            (
                ProviderId::Grok,
                json!({"method":GROK_SESSION_UPDATE,"params":{"update":{"sessionUpdate":"agent_message_chunk","usage":"content"}}}),
            ),
            (
                ProviderId::Grok,
                json!({"type":"turn_completed","usage":{"inputTokens":1000}}),
            ),
            (
                ProviderId::Agy,
                json!({"type":"PLANNER_RESPONSE","status":"RUNNING","source":"MODEL","input_tokens":"unsupported"}),
            ),
            (
                ProviderId::Agy,
                json!({"type":"PLANNER_RESPONSE","status":"DONE","source":"USER","input_tokens":1000}),
            ),
        ] {
            add(provider, &record, &mut ledger).unwrap();
        }
        let mut record = claude_record();
        record["isApiErrorMessage"] = json!(true);
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        record["isApiErrorMessage"] = json!(false);
        record["message"]["model"] = json!("<synthetic>");
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        assert!(snapshot(&ledger).today.models.is_empty());
        assert_eq!(
            ingest(
                ProviderId::Agy,
                b"not-json",
                "conversation",
                "file",
                &mut ledger,
                now()
            ),
            Err(())
        );
    }

    #[test]
    fn retention_and_future_records_follow_ledger_dates_without_content_retention() {
        let mut ledger = TokenLedger::default();
        let mut record = claude_record();
        record["timestamp"] = json!((now() - chrono::Duration::days(60)).to_rfc3339());
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        assert!(snapshot(&ledger).today.models.is_empty());
        record["timestamp"] = json!((now() + chrono::Duration::seconds(1)).to_rfc3339());
        add(ProviderId::Claude, &record, &mut ledger).unwrap();
        assert!(snapshot(&ledger).today.models.is_empty());
        let later = ledger.summarize(
            now() + chrono::Duration::seconds(2),
            TokenUsageStatus::Ready,
            &[],
        );
        assert_eq!(later.today.totals.total(), Some(120));
        let serialized = serde_json::to_string(&later).unwrap();
        for private in ["not retained", "msg_response", "session", "file"] {
            assert!(!serialized.contains(private));
        }
    }
}
