import importlib.util
import argparse
import contextlib
import io
import json
import tempfile
from unittest.mock import patch
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('coverage_runner', Path(__file__).resolve().parents[1] / 'coverage.py')
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


class CoverageTests(unittest.TestCase):
    def test_merging_keeps_real_uncovered_lines_and_combines_instantiations(self):
        result = coverage.merged_lines('SF:a.rs\nDA:1,2\nDA:2,0\nDA:3,0\nend_of_record\nSF:a.rs\nDA:2,3\nend_of_record\nSF:b.rs\nDA:1,0\nend_of_record')
        self.assertEqual(result['lines'], 4)
        self.assertEqual(result['covered_lines'], 2)
        self.assertEqual(result['line_percent'], 50)
        self.assertEqual(result['uncovered_lines'], [{'file': 'a.rs', 'line': 3}, {'file': 'b.rs', 'line': 1}])

    def test_empty_malformed_or_negative_counts_fail(self):
        for text in ['', 'DA:1,1', 'SF:a.rs\nDA:0,1', 'SF:a.rs\nDA:1,-1', 'SF:a.rs\nDA:invalid,1']:
            with self.assertRaises(ValueError):
                coverage.merged_lines(text)


    def test_deferred_benchmarks_never_claim_full_coverage_even_when_all_counts_are_green(self):
        for deferred in (False, True):
            with tempfile.TemporaryDirectory() as directory:
                stale = Path(directory) / 'target/llvm-cov-target/old-binary'
                stale.parent.mkdir(parents=True)
                stale.write_bytes(b'old source mapping')
                calls = []
                environments = []
                def run(command, **kwargs):
                    calls.append(command)
                    environments.append(kwargs['env'])
                    build = Path(kwargs['env']['CARGO_TARGET_DIR'])
                    self.assertEqual(kwargs['env']['CARGO_LLVM_COV_TARGET_DIR'], str(build))
                    self.assertTrue(build.is_dir())
                    self.assertEqual((build / 'CACHEDIR.TAG').read_text().splitlines()[0],
                                     'Signature: 8a477f597d28d172789f06886806bc55')
                    self.assertNotEqual(build, Path(directory) / 'target/llvm-cov-target')
                    if '--lcov' in command:
                        Path(command[-1]).write_text('SF:a.rs\nDA:1,1\nend_of_record\n')
                    if '--json' in command:
                        Path(command[-1]).write_text(json.dumps({'data': [{'totals': {'functions': {'percent': 100}}}]}))
                    return argparse.Namespace(returncode=0)
                arguments = ['coverage.py'] + (['--skip-performance'] if deferred else [])
                with patch.object(coverage, 'ROOT', Path(directory)), patch.object(coverage.sys, 'argv', arguments), patch.object(coverage.subprocess, 'run', side_effect=run), contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(coverage.main(), 0)
                report = json.loads((Path(directory) / 'target/coverage/source-summary.json').read_text())
                self.assertEqual(report['coverage_scope'], 'partial-performance-deferred' if deferred else 'workspace')
                self.assertEqual(report['line_percent'], 100)
                self.assertEqual('--skip' in calls[0], deferred)
                self.assertEqual(len(calls), 3)
                self.assertEqual(len({environment['CARGO_TARGET_DIR'] for environment in environments}), 1)
                self.assertFalse(Path(environments[0]['CARGO_TARGET_DIR']).exists())
                self.assertEqual(report['source_hash_before'], report['source_hash_after'])
                self.assertEqual(stale.read_bytes(), b'old source mapping')

    def test_changed_compilation_inputs_cannot_certify_final_tree_coverage(self):
        with tempfile.TemporaryDirectory() as directory:
            def run(command, **kwargs):
                if '--lcov' in command:
                    Path(command[-1]).write_text('SF:a.rs\nDA:1,1\nend_of_record\n')
                if '--json' in command:
                    Path(command[-1]).write_text(json.dumps({'data': [{'totals': {'functions': {'percent': 100}}}]}))
                return argparse.Namespace(returncode=0)
            with patch.object(coverage, 'ROOT', Path(directory)), patch.object(coverage.sys, 'argv', ['coverage.py']), patch.object(coverage, 'source_hash', side_effect=['before', 'after']), patch.object(coverage.subprocess, 'run', side_effect=run), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(coverage.main(), 1)
            report = json.loads((Path(directory) / 'target/coverage/source-summary.json').read_text())
            self.assertNotEqual(report['source_hash_before'], report['source_hash_after'])


if __name__ == '__main__':
    unittest.main()
