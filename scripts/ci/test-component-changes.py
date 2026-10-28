"""Regression tests for component selection and GitHub event diff handling."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "component_changes", Path(__file__).with_name("component-changes.py"))
changes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(changes)


class ComponentChangesTests(unittest.TestCase):
    def test_docs_skip_every_component(self):
        self.assertFalse(any(changes.select(["README.md", "docs/guide.md"]).values()))

    def test_components_are_independent(self):
        for name, path in {
            "decklink": "src/internal/decklink/audio.rs",
            "ndi": "src/internal/ndi/sdk.rs",
            "powerpoint": "src/internal/presentation/windows-powerpoint.ps1",
            "browser": "src/internal/html/cef_backend.rs",
        }.items():
            with self.subTest(component=name):
                self.assertEqual(
                    {key for key, enabled in changes.select([path]).items() if enabled},
                    {name})

    def test_shared_dependencies_and_workflow_select_all(self):
        for path in ["Cargo.lock", ".github/workflows/component-checks.yml",
                     "scripts/ci/component-changes.py"]:
            self.assertTrue(all(changes.select([path]).values()))

    def run_event(self, kind, event, diff=b""):
        with tempfile.TemporaryDirectory() as directory:
            event_path = Path(directory, "event.json")
            output_path = Path(directory, "output")
            event_path.write_text(json.dumps(event))
            with patch.dict(os.environ, {
                "GITHUB_EVENT_NAME": kind,
                "GITHUB_EVENT_PATH": str(event_path),
                "GITHUB_OUTPUT": str(output_path),
            }), patch.object(changes.subprocess, "check_output", return_value=diff) as git:
                changes.main()
                return dict(line.split("=") for line in output_path.read_text().splitlines()), git.call_args

    def test_push_uses_full_diff_including_deleted_and_renamed_paths(self):
        output, call = self.run_event("push", {"before": "abc", "after": "def"},
                                      b"browser/checks/src/lib.rs\0docs/renamed.md\0")
        self.assertEqual(output["browser"], "true")
        self.assertEqual(output["ndi"], "false")
        self.assertEqual(call.args[0], ["git", "diff", "--name-only", "--no-renames",
                                        "-z", "abc", "def", "--"])

    def test_pull_request_uses_merge_base(self):
        _, call = self.run_event("pull_request", {
            "pull_request": {"base": {"sha": "abc"}, "head": {"sha": "def"}}})
        self.assertIn("abc...def", call.args[0])

    def test_manual_and_initial_push_select_all_without_diff(self):
        for kind, event in [("workflow_dispatch", {}),
                            ("push", {"before": "0" * 40, "after": "def"})]:
            output, call = self.run_event(kind, event)
            self.assertEqual(set(output.values()), {"true"})
            self.assertIsNone(call)


if __name__ == "__main__":
    unittest.main()
