import argparse
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('validate', Path(__file__).resolve().parents[1] / 'validate.py')
validate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validate)


def record():
    return {'schema_version': 1, 'status': 'complete', 'validation_status': 'pass',
            'settings': {'samples': 5}, 'input_hashes': {'cinema': 'input'}, 'toolchain': 'rust',
            'platform': 'linux', 'hardware': {'cpu': 'same'}, 'memory_scope': 'rust process',
            'measurements': {'cinema': {'elapsed_ms': 10, 'peak_rss_bytes': 1000, 'samples': []}}}


class PerformanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.store = Path(self.temporary.name) / 'benchmarks'
        self.patch = patch.object(validate, 'STORE', self.store)
        self.patch.start()
        self.args = argparse.Namespace(speed_tolerance=20, ram_tolerance=10)
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()

    def tearDown(self):
        self.output.__exit__(None, None, None)
        self.patch.stop()
        self.temporary.cleanup()

    def test_speed_and_ram_regressions_fail_independently(self):
        baseline = record()
        self.assertTrue(validate.compare(record(), baseline, self.args))
        for metric, value in [('elapsed_ms', 13), ('peak_rss_bytes', 1101)]:
            current = record()
            current['measurements']['cinema'][metric] = value
            self.assertFalse(validate.compare(current, baseline, self.args))
        self.assertTrue(validate.compare(record(), None, self.args))
        baseline['toolchain'] = 'different'
        self.assertFalse(validate.compatible(record(), baseline))
        self.assertTrue(validate.compare(record(), baseline, self.args))

    def test_last_twenty_runs_are_retained_without_pruning_or_updating_baseline(self):
        validate.save(record(), self.args)
        validate.accept('latest')
        before = (self.store / 'baseline.json').read_text()
        for _ in range(22):
            validate.save(record(), self.args)
        self.assertEqual(len(list((self.store / 'history').glob('*.json'))), 20)
        self.assertEqual((self.store / 'baseline.json').read_text(), before)
        self.assertEqual(json.loads(before)['measurements'], record()['measurements'] | {'cinema': record()['measurements']['cinema'] | {'recent_comparable_runs': 0}})

    def test_baseline_acceptance_requires_explicit_successful_complete_run(self):
        with self.assertRaises(ValueError):
            validate.accept('latest')
        current = record()
        current['validation_status'] = 'fail'
        validate.save(current, self.args)
        with self.assertRaises(ValueError):
            validate.accept('latest')
        current = record()
        current['status'] = 'incomplete'
        validate.save(current, self.args)
        with self.assertRaises(ValueError):
            validate.accept('latest')
        validate.save(record(), self.args)
        selected = sorted((self.store / 'history').glob('*.json'))[-1]
        validate.accept(selected.name)
        self.assertEqual(json.loads((self.store / 'baseline.json').read_text())['baseline_source'], selected.name)
        for name in ['../baseline.json', 'missing.json']:
            with self.assertRaises(ValueError):
                validate.accept(name)

    def test_unassessed_and_regressed_results_are_recorded_honestly(self):
        validate.save(record(), self.args)
        latest = sorted((self.store / 'history').glob('*.json'))[-1]
        self.assertEqual(json.loads(latest.read_text())['performance_gate'], 'unassessed')
        validate.accept('latest')
        current = record()
        current['measurements']['cinema']['elapsed_ms'] = 20
        self.assertFalse(validate.save(current, self.args))
        latest = sorted((self.store / 'history').glob('*.json'))[-1]
        self.assertEqual(json.loads(latest.read_text())['performance_gate'], 'fail')

    def test_new_phases_remain_unassessed_while_existing_regressions_still_fail(self):
        validate.save(record(), self.args)
        validate.accept('latest')
        current = record()
        current['measurements']['cinema/compile'] = {'elapsed_ms': 5, 'peak_rss_bytes': 2000, 'samples': [], 'iterations': 3, 'memory_scope': 'phase setup'}
        self.assertTrue(validate.save(current, self.args))
        latest = sorted((self.store / 'history').glob('*.json'))[-1]
        data = json.loads(latest.read_text())
        self.assertEqual(data['performance_gate'], 'partial')
        self.assertEqual(data['unassessed_workloads'], ['cinema/compile'])
        current['measurements']['cinema']['elapsed_ms'] = 30
        self.assertFalse(validate.save(current, self.args))
        self.assertFalse(validate.workload_compatible({'iterations': 2}, {'iterations': 3}))
        self.assertFalse(validate.workload_compatible({'iterations': 3}, {'iterations': 3, 'protocol_version': 2}))


    def test_measurement_uses_post_exec_rss_and_rejects_missing_memory_or_invalid_specs(self):
        for overrides, raises in [({}, False), ({'peak_rss_bytes': None}, True), ({'valid': False}, True)]:
            data = {'elapsed_ms': 20, 'peak_rss_bytes': 4096, 'valid': True} | overrides
            def launch(command, **kwargs):
                kwargs['stdout'].write(json.dumps(data).encode())
                return argparse.Namespace(pid=123, returncode=None)
            with patch.object(validate.subprocess, 'Popen', side_effect=launch), patch.object(validate.os, 'wait4', return_value=(123, 0, argparse.Namespace(ru_maxrss=999999))):
                if raises:
                    with self.assertRaises(RuntimeError):
                        validate.measure(Path('/binary'), 'specs', 2)
                else:
                    result = validate.measure(Path('/binary'), 'specs', 2)
                    self.assertEqual(result['peak_rss_bytes'], 4096)
                    self.assertEqual(result['elapsed_ms'], 10)


    def test_validation_collects_independent_failures_and_still_records_measurements(self):
        calls = []
        saved = []
        def run(command, **kwargs):
            calls.append(command)
            failed = command[:2] == ['cargo', 'test'] or 'scripts/coverage.py' in command
            return argparse.Namespace(returncode=1 if failed else 0)
        with patch.object(validate.sys, 'argv', ['validate.py', '--coverage', '--samples', '1', '--iterations', '1']), patch.object(validate.subprocess, 'run', side_effect=run), patch.object(validate, 'benchmark', return_value=record()), patch.object(validate, 'save', side_effect=lambda result, args: saved.append(copy.deepcopy(result)) or True):
            self.assertEqual(validate.main(), 1)
        self.assertTrue(any('scripts/coverage.py' in command for command in calls))
        self.assertTrue(all('--no-fail-fast' in command for command in calls if command[:2] == ['cargo', 'test']))
        self.assertEqual(saved[0]['validation_status'], 'fail')
        self.assertEqual(len(saved[0]['validation_steps']), len(calls))
        self.assertGreater(sum(step['returncode'] != 0 for step in saved[0]['validation_steps']), 1)


    def test_deferred_performance_keeps_checks_and_never_writes_benchmark_history(self):
        for failed in (False, True):
            reports = []
            with patch.object(validate.sys, 'argv', ['validate.py', '--focus', 'verification', '--skip-performance']), patch.object(validate.subprocess, 'run', return_value=argparse.Namespace(returncode=int(failed))) as runner, patch.object(validate, 'digest', return_value='checked'), patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'benchmark') as benchmark, patch.object(validate, 'save') as save, patch.object(validate, 'atomic', side_effect=lambda path, report: reports.append(report)):
                self.assertEqual(validate.main(), int(failed))
            benchmark.assert_not_called()
            save.assert_not_called()
            cargo_tests = [call.args[0] for call in runner.call_args_list if call.args[0][:2] == ['cargo', 'test']]
            self.assertTrue(cargo_tests)
            self.assertTrue(all(command[-3:] == ['--', '--skip', 'verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'] for command in cargo_tests))
            self.assertEqual(reports[0]['performance_status'], 'deferred')
            self.assertEqual(reports[0]['validation_status'], 'fail' if failed else 'pass')
            self.assertTrue(reports[0]['validation_steps'])

    def test_validation_rejects_source_changes_between_checks_and_release_measurements(self):
        for hashes, measured_hash, expected in [(('before', 'changed'), 'changed', 1), (('checked', 'checked'), 'changed', 1), (('checked', 'checked'), 'checked', 0)]:
            saved = []
            measured = record() | {'source_hash': measured_hash}
            with patch.object(validate.sys, 'argv', ['validate.py', '--focus', 'verification']), patch.object(validate.subprocess, 'run', return_value=argparse.Namespace(returncode=0)), patch.object(validate, 'digest', side_effect=hashes), patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'benchmark', return_value=measured), patch.object(validate, 'save', side_effect=lambda result, args: saved.append(copy.deepcopy(result)) or True):
                self.assertEqual(validate.main(), expected)
            self.assertEqual(saved[0]['validation_status'], 'fail' if expected else 'pass')
            self.assertEqual(saved[0]['validation_source_hash'], hashes[0])
            self.assertEqual(saved[0]['validation_source_hash_after'], hashes[1])



if __name__ == '__main__':
    unittest.main()
