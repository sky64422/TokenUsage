"""Synthetic hook tests; no real sessions or global configuration are touched."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("hook", Path(__file__).with_name("agy-token-hook.py"))
hook = importlib.util.module_from_spec(spec)
spec.loader.exec_module(hook)


class HookTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.path = self.home / ".gemini/antigravity-cli/brain/session/.system_generated/logs/transcript.jsonl"
        self.path.parent.mkdir(parents=True)
        self.payload = dict(modelName="gemini-test", conversationId="session",
                            initialNumSteps=2, transcriptPath=str(self.path))

    def record(self, step):
        return dict(type="PLANNER_RESPONSE", status="DONE", source="MODEL", step_index=step,
                    created_at="2026-10-19T00:00:00Z", input_tokens=10,
                    cache_read_tokens=90, output_tokens=20, text="PRIVATE CONTENT")

    def write(self, rows):
        self.path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")

    def test_only_new_invocation_metadata_and_model_switch(self):
        self.write([self.record(1), self.record(2)])
        self.assertEqual(hook.collect(self.payload, self.home), 1)
        self.write([self.record(1), self.record(2), self.record(3)])
        self.payload.update(initialNumSteps=3, modelName="claude-test")
        hook.collect(self.payload, self.home)
        data = self.path.with_name(hook.SIDECAR).read_text(encoding="utf-8")
        self.assertNotIn("PRIVATE", data)
        rows = [json.loads(line) for line in data.splitlines()]
        self.assertEqual([row["model_name"] for row in rows], ["gemini-test", "claude-test"])
        self.assertEqual([row["step_index"] for row in rows], [2, 3])

    def test_missing_completed_step_does_not_mislabel_history(self):
        self.write([self.record(1)])
        with self.assertRaises(ValueError):
            hook.collect(self.payload, self.home)
        self.assertFalse(self.path.with_name(hook.SIDECAR).exists())

    def test_installed_cli_full_path_uses_compact_metadata(self):
        self.write([self.record(2)])
        self.payload["transcriptPath"] = str(self.path.with_name("transcript_full.jsonl"))
        self.assertEqual(hook.collect(self.payload, self.home), 1)

    def test_rejects_unrelated_path_and_invalid_boundary(self):
        self.write([self.record(2)])
        for changes in [dict(initialNumSteps=-1), dict(initialNumSteps=True),
                        dict(modelName=""), dict(transcriptPath=str(self.home / "other.jsonl"))]:
            with self.assertRaises(ValueError):
                hook.collect(self.payload | changes, self.home)

    def test_non_numeric_usage_is_never_persisted(self):
        row = self.record(2)
        row["input_tokens"] = "PRIVATE CONTENT"
        self.write([row])
        with self.assertRaises(ValueError):
            hook.collect(self.payload, self.home)
        self.assertFalse(self.path.with_name(hook.SIDECAR).exists())

    def test_installer_preserves_other_hooks_and_is_idempotent(self):
        spec = importlib.util.spec_from_file_location("installer", Path(__file__).with_name("install-agy-token-hook.py"))
        installer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(installer)
        config = self.home / ".gemini/config/hooks.json"
        config.parent.mkdir(parents=True)
        original = {"user-hook": {"enabled": False}}
        config.write_text(json.dumps(original), encoding="utf-8")
        installer.install(self.home, Path("C:/Python/python.exe"))
        installer.install(self.home, Path("C:/Python/python.exe"))
        self.assertEqual(json.loads(config.read_text())["user-hook"], original["user-hook"])
        self.assertEqual(len(list(config.parent.glob("*.bak"))), 1)


if __name__ == "__main__":
    unittest.main()
