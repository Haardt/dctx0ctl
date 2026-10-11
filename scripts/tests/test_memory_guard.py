import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import argparse

spec = importlib.util.spec_from_file_location('memory_guard', Path(__file__).resolve().parents[1] / 'memory_guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class MemoryGuardTests(unittest.TestCase):
    def test_budget_uses_available_memory_and_rejects_missing_or_invalid_values(self):
        self.assertEqual(guard.memory_budget('MemFree: 1 kB\nMemAvailable: 3000 kB'), 2048000)
        for text in ('', 'MemAvailable: 0 kB', 'MemAvailable: -1 kB', 'MemAvailable: 3 MB'):
            with self.assertRaises(ValueError):
                guard.memory_budget(text)

    def test_scope_caps_all_descendants_disables_swap_and_preserves_exit_status(self):
        with patch.object(guard.Path, 'read_text', return_value='MemAvailable: 3000 kB'), patch.object(guard.subprocess, 'run', return_value=argparse.Namespace(returncode=137)) as run:
            self.assertEqual(guard.guarded_run(['bun', 'worker.ts']), 137)
        command = run.call_args.args[0]
        self.assertIn('MemoryMax=2048000', command)
        self.assertIn('MemorySwapMax=0', command)
        self.assertEqual(command[-2:], ['bun', 'worker.ts'])
        with self.assertRaises(ValueError):
            guard.guarded_run([])
