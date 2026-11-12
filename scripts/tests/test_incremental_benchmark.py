import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import incremental_benchmark as bench


def cargo_artifact(name, path, test=True):
    return json.dumps({'reason': 'compiler-artifact', 'target': {'name': name},
                       'profile': {'test': test}, 'executable': path})


class IncrementalBenchmarkTests(unittest.TestCase):
    def test_artifacts_require_actual_unique_test_executables(self):
        output = '\n'.join(['cargo progress', cargo_artifact('integration', '/wrong', False),
                            cargo_artifact('integration', '/release/test'),
                            cargo_artifact('other', '/other')])
        self.assertEqual(bench.artifacts(output, {'integration'}), {'integration': '/release/test'})
        with self.assertRaises(ValueError):
            bench.artifacts(output, {'missing'})
        with self.assertRaises(ValueError):
            bench.artifacts(output + '\n' + cargo_artifact('integration', '/different'), {'integration'})

    def test_filter_precedes_libtest_flags(self):
        self.assertEqual(bench.test_command('/release/test', 'application_incremental'),
                         ['/release/test', 'application_incremental', '--test-threads=1'])
        self.assertEqual(bench.test_command('/release/test', None), ['/release/test', '--test-threads=1'])

    def test_guard_requires_one_nonzero_tree_measurement(self):
        line = 'Memory guard: elapsed_ms=12.50; peak_process_tree_rss_bytes=12345; method=sampled'
        self.assertEqual(bench.guard_metrics(line), {'elapsed_ms': 12.5, 'peak_rss_bytes': 12345})
        for output in ('no metrics', line + '\n' + line,
                       'Memory guard: elapsed_ms=1; peak_process_tree_rss_bytes=0;'):
            with self.subTest(output=output), self.assertRaises(ValueError):
                bench.guard_metrics(output)


if __name__ == '__main__':
    unittest.main()
