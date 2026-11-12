import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import sys

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'scripts'))
import validate

class NativePerformanceReportTests(unittest.TestCase):
    def setUp(self):
        self.base = Path(tempfile.mkdtemp(prefix='native-report-test-'))
        self.binary = self.base / 'binary'
        self.binary.write_bytes(b'expected executable')
        self.digest = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        self.expected = {'status': 'complete', 'target': 'go', 'binary_hash': self.digest,
                         'seed': 42, 'generated_cases': 100, 'cache': 'warm', 'memory_scope': 'test',
                         'samples': {'check': {'elapsed_ms': 1, 'peak_rss_bytes': 1}},
                         'native_workspace': {'path': '/home/test/v-123', 'tmpdir': '/home/test/v-123',
                                              'filesystem': 'btrfs', 'retained': True, 'supervision': 'linux-owner-subreaper-v1', 'comparability': 'unassessed'}}
        self.patch = patch.object(validate, 'artifact_root', return_value=self.base / 'artifacts')
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.root_patch = patch.object(validate, 'ROOT', self.base)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)

    def writer(self, payload):
        def run(command, **kwargs):
            self.assertIn('--report', command)
            path = Path(command[command.index('--report') + 1])
            self.assertFalse(path.exists())
            if payload is not None:
                path.write_text(json.dumps(payload))
            return None
        return run

    def test_unique_fresh_report_not_stale_fixed_receipt(self):
        stale = self.base / 'target/go-cinema/performance.json'
        stale.parent.mkdir(parents=True)
        stale.write_text('stale must remain unchanged')
        with patch.object(validate.subprocess, 'run', side_effect=self.writer(self.expected)):
            first = validate.run_native_cinema_performance('go', self.binary)
            second = validate.run_native_cinema_performance('go', self.binary)
        self.assertNotEqual(first['report_path'], second['report_path'])
        self.assertEqual(stale.read_text(), 'stale must remain unchanged')
        self.assertTrue(Path(first['report_path']).is_file())

    def test_reject_missing_incomplete_or_wrong_binary_receipts(self):
        for payload in (None, [], {**self.expected, 'status': 'incomplete'}, {**self.expected, 'binary_hash': 'wrong'}, {**self.expected, 'target': 'rust'}, {**self.expected, 'native_workspace': {**self.expected['native_workspace'], 'filesystem': 'tmpfs'}}):
            with self.subTest(payload=payload), patch.object(validate.subprocess, 'run', side_effect=self.writer(payload)):
                with self.assertRaises(RuntimeError):
                    validate.run_native_cinema_performance('go', self.binary)

    def test_workload_compatibility_includes_real_native_environment(self):
        settings = validate.native_workload_settings(self.expected)
        base = {'iterations': 1, 'memory_scope': 'test', 'protocol_version': 1, 'workload_settings': settings}
        self.assertTrue(validate.workload_compatible(base, dict(base)))
        for key, changed in [('filesystem', 'tmpfs'), ('temp_root', '/different'), ('cache', 'cold')]:
            with self.subTest(key=key):
                self.assertFalse(validate.workload_compatible(base, {**base, 'workload_settings': {**settings, key: changed}}))
        self.assertTrue(validate.workload_compatible({'iterations': 1}, {'iterations': 1}))

if __name__ == '__main__':
    unittest.main()
