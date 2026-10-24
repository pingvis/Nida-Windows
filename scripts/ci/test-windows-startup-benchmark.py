"""Regression checks for the isolated startup benchmark's fixtures and verdict."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'startup_benchmark', Path(__file__).with_name('benchmark-windows-startup.py'))
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)


class StartupBenchmarkTests(unittest.TestCase):
    def test_shader_fixture_does_not_create_an_incomplete_saved_show(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shaders = root / 'library'
            shaders.mkdir()
            (shaders / 'example.fs').write_text('shader fixture', encoding='utf-8')
            workspace = root / 'run'
            benchmark.prepare_workspace(workspace, shaders)
            self.assertFalse((workspace / '.varda').exists())
            self.assertEqual((workspace / 'shaders' / 'example.fs').read_text(), 'shader fixture')

    def test_previous_show_is_rejected_and_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            saved = workspace / '.varda'
            saved.mkdir()
            stage = saved / 'stage.json'
            stage.write_text('existing show', encoding='utf-8')
            with self.assertRaisesRegex(RuntimeError, 'fresh workspace'):
                benchmark.prepare_workspace(workspace)
            self.assertEqual(stage.read_text(), 'existing show')

    def test_error_notifications_fail_even_without_a_crash(self):
        self.assertTrue(benchmark.error_record(
            '[2026-10-24T14:48:31Z ERROR varda::internal::notifications] '
            '[Notification] Failed to load workspace: bus routing held black'))
        self.assertTrue(benchmark.error_record('[time ERROR module] native failure'))
        self.assertTrue(benchmark.error_record('PANIC: startup assertion'))
        self.assertFalse(benchmark.error_record('[time WARN module] optional device unavailable'))
        self.assertFalse(benchmark.error_record('[time INFO module] [Notification] MIDI ready'))


if __name__ == '__main__':
    unittest.main()
