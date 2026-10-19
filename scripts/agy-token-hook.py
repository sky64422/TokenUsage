"""AGY PostInvocation hook: retain exact usage metadata, never conversation text."""
import json
from datetime import datetime
import os
from pathlib import Path
import sys

MAX_INPUT = 64 * 1024
MAX_TAIL = 8 * 1024 * 1024
MAX_LINE = 1024 * 1024
MAX_COUNTER = (1 << 64) - 1
SIDECAR = "tokenusage-models.jsonl"
FIELDS = ("type", "status", "source", "step_index", "created_at",
          "input_tokens", "cache_read_tokens", "output_tokens")


def collect(payload, home=None):
    home = Path(home or Path.home()).resolve()
    model = payload.get("modelName")
    start = payload.get("initialNumSteps")
    if not isinstance(model, str) or not model.strip() or model.lower() == "auto" or len(model.encode()) > 96 or any(ord(c) < 32 for c in model):
        raise ValueError("invalid model metadata")
    if type(start) is not int or start < 0:
        raise ValueError("invalid invocation boundary")
    conversation = payload.get("conversationId")
    if not isinstance(conversation, str) or not conversation or len(conversation) > 128 or any(not (c.isascii() and (c.isalnum() or c in "-_.")) for c in conversation) or conversation in (".", ".."):
        raise ValueError("invalid conversation identifier")
    path = Path(payload["transcriptPath"]).expanduser().resolve()
    allowed = [home / ".gemini" / app / "brain" / conversation / ".system_generated" / "logs" / "transcript.jsonl"
               for app in ("antigravity", "antigravity-cli", "antigravity-ide")]
    # Installed CLI supplies transcript_full.jsonl despite the docs naming the
    # compact file. Both share step indices; read only the compact sibling.
    if path.name not in ("transcript.jsonl", "transcript_full.jsonl") or path.with_name("transcript.jsonl") not in [p.resolve() for p in allowed]:
        raise ValueError("unexpected transcript location")
    path = path.with_name("transcript.jsonl")
    rows = []
    with path.open("rb") as stream:
        size = stream.seek(0, os.SEEK_END)
        offset = max(0, size - MAX_TAIL)
        stream.seek(offset)
        if offset:
            stream.readline(MAX_LINE)
        remaining = size - stream.tell()
        while remaining > 0:
            line = stream.readline(min(remaining, MAX_LINE + 1))
            remaining -= len(line)
            if len(line) > MAX_LINE:
                raise ValueError("oversized transcript record")
            if not line.endswith(b"\n"):
                break
            row = json.loads(line)
            if row.get("type") != "PLANNER_RESPONSE" or row.get("status") != "DONE" or row.get("source") != "MODEL":
                continue
            step = row.get("step_index")
            if type(step) is not int or step < start:
                continue
            if step > MAX_COUNTER:
                raise ValueError("invalid step index")
            for key in ("input_tokens", "cache_read_tokens", "output_tokens"):
                value = row.get(key)
                if value is not None and (type(value) is not int or not 0 <= value <= MAX_COUNTER):
                    raise ValueError("invalid token counter")
            at = row.get("created_at")
            if not isinstance(at, str) or len(at) > 64:
                raise ValueError("invalid timestamp")
            try:
                valid_time = datetime.fromisoformat(at).tzinfo is not None
            except ValueError:
                valid_time = False
            if not valid_time:
                raise ValueError("invalid timestamp")
            # The post hook observes only the just-finished invocation's steps.
            if row.get("model_name", model) != model or row.get("model", model) != model:
                raise ValueError("conflicting explicit model")
            metadata = {key: row[key] for key in FIELDS if key in row}
            metadata["model_name"] = model
            rows.append(metadata)
    if not rows:
        raise ValueError("completed invocation not present in transcript yet")
    # AGY serializes invocations per conversation; different sessions have their
    # own files. A duplicate callback is harmless: the reader deduplicates steps.
    encoded = "".join(json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n" for row in rows)
    with path.with_name(SIDECAR).open("ab") as stream:
        stream.write(encoded.encode("utf-8"))
    return len(rows)


def main():
    try:
        raw = sys.stdin.buffer.read(MAX_INPUT + 1)
        if len(raw) > MAX_INPUT:
            raise ValueError("oversized hook payload")
        collect(json.loads(raw))
    except (OSError, ValueError, KeyError, TypeError) as error:
        # Never echo the payload, path, content, or a provider error message.
        reason = str(error) if type(error) is ValueError else type(error).__name__
        print("TokenUsage: model metadata capture failed (" + reason + ")", file=sys.stderr)
        return 1
    print("{}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
