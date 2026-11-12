import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'scripts'))
import go_cinema_performance as perf
import rust_cinema_performance as rust_perf
import native_performance_workspace

class NativeWorkspaceTests(unittest.TestCase):
    def setUp(self):
        cache = Path.home() / '.cache' / 'dctx0ctl'
        cache.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.base = Path(tempfile.mkdtemp(prefix='test-', dir=cache))
        self.env = patch.dict(os.environ, {'DCTX0_VALIDATION_TEMP_ROOT': str(self.base)}, clear=False)
        self.env.start()
        self.addCleanup(self.env.stop)

    def test_preserve_and_default_cleanup_and_environment(self):
        original = os.environ.get('TMPDIR')
        with self.assertRaisesRegex(RuntimeError, 'interrupted'):
            with perf.native_workspace(True) as held:
                (held / 'failed-run.json').write_text('{"status":"incomplete"}')
                self.assertEqual(os.environ['TMPDIR'], str(held))
                raise RuntimeError('interrupted')
        self.assertEqual(json.loads((held / 'failed-run.json').read_text())['status'], 'incomplete')
        self.assertEqual(os.environ.get('TMPDIR'), original)
        with perf.native_workspace(False) as ephemeral:
            self.assertTrue(ephemeral.exists())
        self.assertFalse(ephemeral.exists())

    def test_hold_records_incomplete_and_copies_authored_notes_before_failure(self):
        binary = self.base / 'binary'
        binary.write_bytes(b'fixture binary')
        def copy_application(project):
            notes = project / '.dctx0/testing/go/inspection-notes.json'
            notes.parent.mkdir(parents=True)
            notes.write_text('{"authored":true}')
        with patch.dict(os.environ, {'DCTX0_PRESERVE_BENCHMARK_HISTORY': '1'}, clear=False), patch.object(perf, 'copy_application', copy_application), patch.object(perf, 'measure', side_effect=RuntimeError('failed native child')), patch.object(sys, 'argv', ['perf', '--binary', str(binary)]):
            with self.assertRaisesRegex(RuntimeError, 'failed native child'):
                perf.main()
        workspaces = list(self.base.glob('v-*'))
        self.assertEqual(len(workspaces), 1)
        report = json.loads((workspaces[0] / 'performance.json').read_text())
        self.assertEqual(report['status'], 'incomplete')
        self.assertTrue(report['native_workspace']['retained'])
        self.assertEqual(report['native_workspace']['tmpdir'], str(workspaces[0]))
        self.assertEqual(json.loads((workspaces[0] / 'inspection-notes-original.json').read_text()), {'authored': True})

    def test_reject_relative_temp_root(self):
        with patch.dict(os.environ, {'DCTX0_VALIDATION_TEMP_ROOT': 'relative'}, clear=False):
            with self.assertRaisesRegex(ValueError, 'absolute'):
                with perf.native_workspace(True):
                    pass

    def test_rust_hold_retains_partial_receipt_and_original_inspection(self):
        binary = self.base / 'binary'
        binary.write_bytes(b'fixture binary')
        def copy_application(project):
            notes = project / '.dctx0/testing/rust/inspection-notes.json'
            notes.parent.mkdir(parents=True)
            notes.write_text('{"authored":true}')
        with patch.dict(os.environ, {'DCTX0_PRESERVE_BENCHMARK_HISTORY': '1'}, clear=False), patch.object(rust_perf, 'copy_application', copy_application), patch.object(rust_perf, 'measure', side_effect=RuntimeError('failed native child')), patch.object(sys, 'argv', ['perf', '--binary', str(binary)]):
            with self.assertRaisesRegex(RuntimeError, 'failed native child'):
                rust_perf.main()
        held = next(self.base.glob('v-*'))
        report = json.loads((held / 'performance.json').read_text())
        self.assertEqual((report['status'], report['target']), ('incomplete', 'rust'))
        self.assertEqual(json.loads((held / 'inspection-notes-original.json').read_text()), {'authored': True})

    def test_reject_memory_backed_temp_root(self):
        with patch('native_performance_workspace.filesystem_type', return_value='tmpfs'):
            with self.assertRaisesRegex(ValueError, 'on-disk'):
                with perf.native_workspace(True):
                    pass

    def test_failed_atomic_replace_keeps_previous_report(self):
        report = self.base / 'performance.json'
        report.write_text('{"status":"incomplete","sample":1}')
        with patch.object(native_performance_workspace.os, 'replace', side_effect=OSError('disk failure')):
            with self.assertRaisesRegex(OSError, 'disk failure'):
                native_performance_workspace.atomic_json(report, {'status': 'complete'})
        self.assertEqual(json.loads(report.read_text()), {'status': 'incomplete', 'sample': 1})

    def test_native_measure_interrupt_kills_and_reaps_the_process_group(self):
        child_file = self.base / 'child.pid'
        grandchild_file = self.base / 'grandchild.pid'
        child_code = (
            'import os,pathlib,subprocess,sys,time; '
            f'pathlib.Path({str(child_file)!r}).write_text(str(os.getpid())); '
            'child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(60)"],start_new_session=True); '
            f'pathlib.Path({str(grandchild_file)!r}).write_text(str(child.pid)); '
            'time.sleep(60)'
        )
        worker_code = (
            'import sys\n'
            f'sys.path.insert(0,{str(root / "scripts")!r})\n'
            'from native_performance_workspace import measure_json\n'
            'try:\n'
            f' measure_json([sys.executable,"-c",{child_code!r}],{str(self.base)!r})\n'
            'except KeyboardInterrupt:\n'
            ' pass\n'
        )
        worker = subprocess.Popen([sys.executable, '-c', worker_code], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        child = None
        grandchild = None
        try:
            deadline = time.monotonic() + 10
            while not grandchild_file.exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue(grandchild_file.exists(), 'native child did not start')
            child = int(child_file.read_text())
            grandchild = int(grandchild_file.read_text())
            os.kill(worker.pid, signal.SIGINT)
            self.assertEqual(worker.wait(timeout=10), 0)
            self.assertFalse(Path(f'/proc/{child}').exists(), 'direct native child was not reaped')
            self.assertFalse(Path(f'/proc/{grandchild}').exists(), 'detached native descendant was not reaped')
        finally:
            if worker.poll() is None:
                worker.kill()
                worker.wait(timeout=10)
            if child is not None:
                try:
                    os.killpg(child, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            if grandchild is not None:
                try:
                    os.killpg(grandchild, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            worker.stdout.close()
            worker.stderr.close()

    def test_exited_native_parent_cannot_orphan_detached_child_or_kill_unrelated_process(self):
        pid_file = self.base / 'orphan.pid'
        child_code = (
            'import pathlib,subprocess,sys; '
            'child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(60)"],start_new_session=True); '
            f'pathlib.Path({str(pid_file)!r}).write_text(str(child.pid)); '
            'print("{\\"ok\\":true}",flush=True)'
        )
        unrelated = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'], start_new_session=True)
        detached = None
        try:
            value, sample = native_performance_workspace.measure_json([sys.executable, '-c', child_code], self.base)
            detached = int(pid_file.read_text())
            self.assertEqual(value, {'ok': True})
            self.assertEqual(sample['exit_code'], 0)
            self.assertFalse(Path(f'/proc/{detached}').exists(), 'detached native child was not reaped')
            self.assertIsNone(unrelated.poll(), 'unrelated process was terminated')
        finally:
            unrelated.kill()
            unrelated.wait(timeout=10)
            if detached is not None:
                try:
                    os.killpg(detached, signal.SIGKILL)
                except ProcessLookupError:
                    pass

    def test_native_measure_preserves_wait4_rss_and_json(self):
        value, sample = native_performance_workspace.measure_json([sys.executable, '-c', 'print("{\\"ok\\":true}")'], self.base)
        self.assertEqual(value, {'ok': True})
        self.assertEqual(sample['exit_code'], 0)
        self.assertGreater(sample['peak_rss_bytes'], 0)

    def test_native_owner_token_extends_existing_supervisor_chain(self):
        with patch.dict(os.environ, {'DCTX0_NATIVE_OWNERS': 'outer-owner'}, clear=False):
            result, _ = native_performance_workspace.measure_json([sys.executable, '-c', 'import json,os; print(json.dumps(os.environ["DCTX0_NATIVE_OWNERS"]))'], self.base)
        self.assertTrue(result.startswith('outer-owner:'))
        self.assertEqual(len(result.split(':')[-1]), 32)

    def test_native_start_failure_restores_signal_handler(self):
        previous = signal.getsignal(signal.SIGTERM)
        with self.assertRaises(FileNotFoundError):
            native_performance_workspace.measure_json([str(self.base / 'does-not-exist')], self.base)
        self.assertEqual(signal.getsignal(signal.SIGTERM), previous)

if __name__ == '__main__':
    unittest.main()
