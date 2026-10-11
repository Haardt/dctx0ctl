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
        with patch.dict(guard.os.environ, {'DCTX0_MEMORY_GUARD': '1'}), patch.object(guard, 'systemd_available', return_value=True), patch.object(guard.Path, 'read_text', return_value='MemAvailable: 3000 kB'), patch.object(guard.subprocess, 'run', return_value=argparse.Namespace(returncode=137)) as run:
            self.assertEqual(guard.guarded_run(['bun', 'worker.ts']), 137)
        command = run.call_args.args[0]
        self.assertIn('MemoryMax=2048000', command)
        self.assertIn('MemorySwapMax=0', command)
        self.assertEqual(command[-2:], ['bun', 'worker.ts'])
        with self.assertRaises(ValueError):
            guard.guarded_run([])

    def test_disabled_or_unavailable_guard_runs_command_once_without_meminfo(self):
        for setting, available in [('0', True), ('1', False)]:
            with patch.dict(guard.os.environ, {'DCTX0_MEMORY_GUARD': setting}), patch.object(guard, 'systemd_available', return_value=available), patch.object(guard.Path, 'read_text') as read, patch.object(guard.subprocess, 'run', return_value=argparse.Namespace(returncode=7)) as run:
                self.assertEqual(guard.guarded_run(['bun', 'worker.ts']), 7)
                run.assert_called_once_with(['bun', 'worker.ts'])
                read.assert_not_called()

    def test_detection_requires_linux_tools_and_a_reachable_user_manager(self):
        with patch.object(guard.sys, 'platform', 'darwin'):
            self.assertFalse(guard.systemd_available())
        with patch.object(guard.sys, 'platform', 'linux'), patch.object(guard.shutil, 'which', return_value=None):
            self.assertFalse(guard.systemd_available())
        with patch.object(guard.sys, 'platform', 'linux'), patch.object(guard.shutil, 'which', return_value='/usr/bin/tool'):
            for result in (0, 1):
                with patch.object(guard.subprocess, 'run', return_value=argparse.Namespace(returncode=result)):
                    self.assertEqual(guard.systemd_available(), result == 0)
            with patch.object(guard.subprocess, 'run', side_effect=guard.subprocess.TimeoutExpired('systemctl', 5)):
                self.assertFalse(guard.systemd_available())
