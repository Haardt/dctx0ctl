import argparse
import contextlib
import copy
import importlib.util
import io
import json
import os
import subprocess
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

spec = importlib.util.spec_from_file_location('validate', Path(__file__).resolve().parents[1] / 'validate.py')
validate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validate)


def record():
    return {'schema_version': 1, 'status': 'complete', 'validation_status': 'pass',
            'settings': {'samples': 5}, 'input_hashes': {'cinema': 'input'}, 'toolchain': 'rust',
            'platform': 'linux', 'hardware': {'cpu': 'same'}, 'memory_scope': 'rust process',
            'measurements': {'cinema': {'elapsed_ms': 10, 'peak_rss_bytes': 1000, 'samples': []}}}


class PerformanceTests(unittest.TestCase):
    def test_source_guard_ignores_new_build_caches_but_detects_authored_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            harness = root / 'examples/cinema-rust/.dctx0/testing/rust'
            harness.mkdir(parents=True)
            contract = harness / 'config.json'
            contract.write_text('{"version":1}')
            with patch.object(validate, 'ROOT', root):
                def inventory():
                    return [path for path in validate.validation_sources() if path.is_file()]
                before = validate.digest(inventory())
                for name in ('target/debug/.fingerprint/worker.json',
                             'node_modules/dependency/config.json'):
                    cache = harness / name
                    cache.parent.mkdir(parents=True, exist_ok=True)
                    cache.write_text('{"generated":true}')
                self.assertEqual(validate.digest(inventory()), before)
                self.assertIn(contract, inventory())
                contract.write_text('{"version":2}')
                self.assertNotEqual(validate.digest(inventory()), before)

    def setUp(self):
        # These tests isolate validation/performance behavior; receipt behavior
        # has its own integration suite and must not probe real host toolchains.
        self.cache_patch = patch.object(validate, 'cache_eligible', return_value=False)
        self.cache_patch.start()
        self.environment = patch.dict(os.environ)
        self.environment.start()
        os.environ.pop('DCTX0_PRESERVE_BENCHMARK_HISTORY', None)
        os.environ.pop('DCTX0_VALIDATION_ARTIFACT_ROOT', None)
        self.temporary = tempfile.TemporaryDirectory()
        self.store = Path(self.temporary.name) / 'benchmarks'
        self.patch = patch.object(validate, 'STORE', self.store)
        self.patch.start()
        self.analysis_patch = patch.object(validate, 'read_analysis_report', return_value={'measurements': {}, 'memory_scope': 'mock measurement'})
        self.analysis_patch.start()
        self.args = argparse.Namespace(speed_tolerance=20, ram_tolerance=10)
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()

    def tearDown(self):
        self.cache_patch.stop()
        self.output.__exit__(None, None, None)
        self.analysis_patch.stop()
        self.patch.stop()
        self.temporary.cleanup()
        self.environment.stop()

    def short_cache(self):
        base = Path.home() / '.cache'
        base.mkdir(parents=True, exist_ok=True)
        directory = tempfile.TemporaryDirectory(prefix='t-', dir=base)
        self.addCleanup(directory.cleanup)
        return Path(directory.name).resolve()

    def test_explicit_history_hold_keeps_failed_and_successful_runs_and_never_updates_baseline(self):
        history = self.store / 'history'
        history.mkdir(parents=True)
        originals = {}
        for index in range(25):
            previous = record()
            if index % 2:
                previous['status'] = 'incomplete'
            path = history / f'20000101T0000{index:02d}.json'
            path.write_text(json.dumps(previous))
            originals[path] = path.read_bytes()
        baseline = self.store / 'baseline.json'
        baseline.write_text(json.dumps(record()))
        baseline_bytes = baseline.read_bytes()
        self.args.preserve_history = True
        current = record()
        self.assertTrue(validate.save(current, self.args))
        self.assertEqual(current['history_retention'], 'preserve-all')
        failed = record()
        failed['status'] = 'incomplete'
        self.assertFalse(validate.save(failed, self.args))
        self.assertEqual(len(list(history.glob('*.json'))), 27)
        for path, contents in originals.items():
            self.assertEqual(path.read_bytes(), contents)
        self.assertEqual(baseline.read_bytes(), baseline_bytes)

    def test_held_functional_cache_is_private_short_unique_and_keeps_inherited_provenance(self):
        root = Path(self.temporary.name)
        base = self.short_cache()
        artifacts = root / ('long-artifact-root-' * 6)
        artifacts.mkdir()
        old = artifacts / 'older-report.json'
        old.write_bytes(b'untouched historical evidence')
        os.environ['DCTX0_VALIDATION_TEMP_ROOT'] = str(base)
        os.environ['DCTX0_VALIDATION_ARTIFACT_ROOT'] = str(artifacts)
        os.environ['DCTX0_PRESERVE_BENCHMARK_HISTORY'] = '1'
        first = Path(validate.create_held_functional_cache(artifacts))
        recorded = json.loads((artifacts / 'run.json').read_text())
        self.assertEqual(recorded, {'status': 'incomplete', 'artifact_root': str(artifacts.resolve()), 'functional_cache': str(first)})
        self.assertEqual(first.parent, base.resolve())
        self.assertEqual(first.stat().st_mode & 0o777, 0o700)
        self.assertLessEqual(len(os.fsencode(first / 'dctx0-ipc-ffffffff-ffffffffffffffff' / 's')), 100)
        environment = {**os.environ, 'TMPDIR': str(first)}
        child = subprocess.run([sys.executable, '-c', "import json,os; print(json.dumps({k:os.environ[k] for k in ('TMPDIR','DCTX0_VALIDATION_ARTIFACT_ROOT','DCTX0_PRESERVE_BENCHMARK_HISTORY')}))"], env=environment, text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(child.stdout), {k:environment[k] for k in ('TMPDIR','DCTX0_VALIDATION_ARTIFACT_ROOT','DCTX0_PRESERVE_BENCHMARK_HISTORY')})
        other_artifacts = root / 'next-report'
        other_artifacts.mkdir()
        second = Path(validate.create_held_functional_cache(other_artifacts))
        self.assertNotEqual(first, second)
        self.assertTrue(first.is_dir())
        self.assertEqual(old.read_bytes(), b'untouched historical evidence')
        self.assertEqual(json.loads((artifacts/'run.json').read_text()), recorded)

    def test_held_main_isolates_long_artifacts_and_inherits_short_cache(self):
        root = Path(self.temporary.name)
        base = self.short_cache()
        project = root / ('long-artifact-parent-' * 5)
        project.mkdir()
        old = project / 'old-evidence.json'
        old.write_bytes(b'keep')
        environments = []
        def run(command, **kwargs):
            if 'env' in kwargs:
                environments.append(kwargs['env'].copy())
            return argparse.Namespace(stdout='', returncode=0)
        argv = ['validate.py', '--focus', 'verification', '--preserve-history', '--skip-performance']
        with patch.dict(validate.os.environ, {'DCTX0_VALIDATION_TEMP_ROOT': str(base)}), \
             patch.object(validate, 'ROOT', project), patch.object(validate.sys, 'argv', argv), \
             patch.object(validate.subprocess, 'run', side_effect=run), \
             patch.object(validate, 'validation_sources', return_value=[]), \
             patch.object(validate, 'digest', return_value='fixed'):
            self.assertEqual(validate.main(), 0)
            first_count = len(environments)
            self.assertEqual(validate.main(), 0)
        runs = list((project/'target/validation-runs').glob('*/run.json'))
        self.assertEqual(len(runs), 2)
        records = [json.loads(path.read_text()) for path in runs]
        self.assertEqual(len({record['functional_cache'] for record in records}), 2)
        self.assertTrue(all(record['status'] == 'incomplete' for record in records))
        self.assertTrue(all(Path(record['functional_cache']).is_dir() for record in records))
        self.assertEqual(old.read_bytes(), b'keep')
        self.assertGreater(first_count, 0)
        self.assertGreater(len(environments), first_count)
        for environment in environments:
            self.assertEqual(environment['DCTX0_PRESERVE_BENCHMARK_HISTORY'], '1')
            self.assertTrue(Path(environment['DCTX0_VALIDATION_ARTIFACT_ROOT']).is_relative_to(project))
            self.assertTrue(Path(environment['TMPDIR']).is_relative_to(base))
        self.assertNotEqual(environments[0]['DCTX0_VALIDATION_ARTIFACT_ROOT'], environments[first_count]['DCTX0_VALIDATION_ARTIFACT_ROOT'])
        self.assertNotEqual(environments[0]['TMPDIR'], environments[first_count]['TMPDIR'])

    def test_interrupted_hold_retains_recorded_cache_and_existing_artifacts(self):
        root = Path(self.temporary.name)
        artifacts = root / 'interrupted'
        artifacts.mkdir()
        previous = artifacts / 'old-report.json'
        previous.write_bytes(b'unchanged')
        environment = {**os.environ, 'DCTX0_VALIDATION_TEMP_ROOT': str(self.short_cache()),
                       'DCTX0_VALIDATION_ARTIFACT_ROOT': str(artifacts),
                       'DCTX0_PRESERVE_BENCHMARK_HISTORY': '1',
                       'PYTHONPATH': str(Path(__file__).resolve().parents[1])}
        code = "import os,pathlib,time,validate; print(validate.create_held_functional_cache(pathlib.Path(os.environ['DCTX0_VALIDATION_ARTIFACT_ROOT'])),flush=True); time.sleep(30)"
        child = subprocess.Popen([sys.executable, '-c', code], env=environment, stdout=subprocess.PIPE, text=True)
        try:
            cache = Path(child.stdout.readline().strip())
            self.assertTrue(cache.is_dir())
        finally:
            child.terminate()
            child.wait(timeout=5)
            child.stdout.close()
        record = json.loads((artifacts/'run.json').read_text())
        self.assertEqual(record['status'], 'incomplete')
        self.assertEqual(record['functional_cache'], str(cache))
        self.assertTrue(cache.is_dir())
        self.assertEqual(previous.read_bytes(), b'unchanged')

    def test_validation_cache_requires_absolute_socket_safe_override(self):
        os.environ['DCTX0_VALIDATION_TEMP_ROOT'] = 'relative'
        with self.assertRaisesRegex(ValueError, 'absolute'):
            validate.functional_cache_root()
        os.environ['DCTX0_VALIDATION_TEMP_ROOT'] = '/home/' + 'too-long-' * 10
        with self.assertRaisesRegex(ValueError, 'too long'):
            validate.functional_cache_root()

    def test_application_helper_coverage_unions_source_identical_blocks_and_rejects_stale_maps(self):
        root = Path(self.temporary.name)
        source = root / 'targets/go/applicationtests/client.go'
        source.parent.mkdir(parents=True)
        source.write_text('package applicationtests\n')
        a, b = root / 'unit.coverage', root / 'native.coverage'
        first = 'github.com/Haardt/dctx0ctl/applicationtests/go/client.go:1.1,2.1'
        second = 'github.com/Haardt/dctx0ctl/applicationtests/go/client.go:3.1,4.1'
        a.write_text(f'mode: set\n{first} 1 1\n{second} 3 0\n')
        b.write_text(f'mode: atomic\n{first} 1 0\n{second} 3 4\n')
        with patch.object(validate, 'ROOT', root):
            merged = validate.application_go_coverage([a, b])
            self.assertEqual((merged['statements'], merged['covered_statements']), (4, 4))
            self.assertEqual(merged['statement_percent'], 100)
            for content in (f'mode: atomic\n{first} 2 1\n{second} 3 0\n',
                            f'mode: atomic\n{first} 1 1\n',
                            f'mode: set\n{first} 1 1\n{first} 1 0\n',
                            'mode: set\nforeign/client.go:1.1,2.1 1 1\n'):
                b.write_text(content)
                with self.assertRaises(ValueError):
                    validate.application_go_coverage([a, b])

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
            return argparse.Namespace(stdout='', returncode=1 if failed else 0)
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
            with patch.object(validate.sys, 'argv', ['validate.py', '--focus', 'verification', '--skip-performance']), patch.object(validate.subprocess, 'run', return_value=argparse.Namespace(stdout='', returncode=int(failed))) as runner, patch.object(validate, 'digest', return_value='checked'), patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'benchmark') as benchmark, patch.object(validate, 'save') as save, patch.object(validate, 'atomic', side_effect=lambda path, report: reports.append(report)):
                self.assertEqual(validate.main(), int(failed))
            benchmark.assert_not_called()
            save.assert_not_called()
            cargo_tests = [call.args[0] for call in runner.call_args_list if call.args[0][:2] == ['cargo', 'test']]
            self.assertTrue(cargo_tests)
            self.assertTrue(all(command[-3:] == ['--', '--skip', 'verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'] for command in cargo_tests))
            self.assertEqual(reports[0]['performance_status'], 'deferred')
            self.assertEqual(reports[0]['validation_status'], 'fail' if failed else 'pass')
            self.assertTrue(reports[0]['validation_steps'])

    def test_functional_checks_use_owned_disk_temporary_space_without_changing_release_environment(self):
        environments = []
        cache = self.short_cache()
        def run(command, **kwargs):
            environment = kwargs.get('env')
            if environment is not None:
                temporary = Path(environment['TMPDIR'])
                self.assertTrue(temporary.is_dir())
                self.assertTrue(temporary.is_relative_to(cache / 'dctx0ctl'))
                environments.append(temporary)
            return argparse.Namespace(stdout='', returncode=0)
        measured = record() | {'source_hash': 'checked'}
        with patch.dict(validate.os.environ, {'XDG_CACHE_HOME': str(cache), 'TMPDIR': '/original-temporary-root'}), patch.object(validate.sys, 'argv', ['validate.py', '--focus', 'verification']), patch.object(validate.subprocess, 'run', side_effect=run), patch.object(validate, 'digest', return_value='checked'), patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'benchmark', return_value=measured) as benchmark, patch.object(validate, 'save', return_value=True):
            self.assertEqual(validate.main(), 0)
            self.assertEqual(validate.os.environ['TMPDIR'], '/original-temporary-root')
        benchmark.assert_called_once()
        self.assertTrue(environments)
        self.assertTrue(all(not temporary.exists() for temporary in environments))


    def test_validation_rejects_source_changes_between_checks_and_release_measurements(self):
        for hashes, measured_hash, expected in [(('before', 'changed'), 'changed', 1), (('checked', 'checked'), 'changed', 1), (('checked', 'checked'), 'checked', 0)]:
            saved = []
            measured = record() | {'source_hash': measured_hash}
            with patch.object(validate.sys, 'argv', ['validate.py', '--focus', 'verification']), patch.object(validate.subprocess, 'run', return_value=argparse.Namespace(stdout='', returncode=0)), patch.object(validate, 'digest', side_effect=hashes), patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'benchmark', return_value=measured), patch.object(validate, 'save', side_effect=lambda result, args: saved.append(copy.deepcopy(result)) or True):
                self.assertEqual(validate.main(), expected)
            self.assertEqual(saved[0]['validation_status'], 'fail' if expected else 'pass')
            self.assertEqual(saved[0]['validation_source_hash'], hashes[0])
            self.assertEqual(saved[0]['validation_source_hash_after'], hashes[1])

    def test_fresh_dependencies_are_pinned_before_checks_and_later_edits_still_fail(self):
        for mutate, preparation_failed in [(False, False), (True, False), (False, True)]:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                authored = root / 'adapter.ts'
                dependency = root / 'node_modules/dependency/package.json'
                authored.write_text('authored source')
                saved = []
                def sources():
                    return [authored] + ([dependency] if dependency.exists() else [])
                def run(command, **kwargs):
                    if command[:2] == ['bun', 'install']:
                        dependency.parent.mkdir(parents=True, exist_ok=True)
                        dependency.write_text('{"version":"pinned"}')
                        return argparse.Namespace(stdout='', returncode=int(preparation_failed))
                    self.assertTrue(dependency.exists(), 'Checks must follow dependency preparation')
                    if mutate and command[:2] == ['cargo', 'test']:
                        dependency.write_text('{"version":"changed during tests"}')
                    return argparse.Namespace(stdout='', returncode=0)
                def benchmark(args):
                    return record() | {'source_hash': validate.digest(sources())}
                with patch.object(validate, 'ROOT', root), patch.object(validate.sys, 'argv', ['validate.py', '--focus', 'verification', '--example', 'cinema']), patch.object(validate, 'validation_sources', side_effect=sources), patch.object(validate.subprocess, 'run', side_effect=run), patch.object(validate, 'benchmark', side_effect=benchmark), patch.object(validate, 'save', side_effect=lambda result, args: saved.append(copy.deepcopy(result)) or True):
                    self.assertEqual(validate.main(), int(mutate or preparation_failed))
                self.assertEqual(saved[0]['validation_status'], 'fail' if mutate or preparation_failed else 'pass')
                self.assertEqual(saved[0]['validation_source_hash'] == saved[0]['validation_source_hash_after'], not mutate)
                self.assertEqual(saved[0]['validation_steps'][0]['command'][:2], ['bun', 'install'])



if __name__ == '__main__':
    unittest.main()


class ValidationStatusTests(unittest.TestCase):
    def test_unavailable_race_is_incomplete_never_pass_and_does_not_hide_failures(self):
        command = [validate.sys.executable, 'scripts/go_race.py']
        unavailable = validate.validation_step(command, 2)
        self.assertEqual(validate.validation_status([unavailable]), 'incomplete')
        self.assertEqual(validate.validation_status([unavailable], False), 'fail')
        failure = validate.validation_step(['cargo', 'test'], 2)
        self.assertEqual(validate.validation_status([unavailable, failure]), 'fail')
        self.assertEqual(validate.validation_status([validate.validation_step(command, 1)]), 'fail')
        self.assertEqual(validate.validation_status([validate.validation_step(command, 0)]), 'pass')


class ResearchReceiptTests(unittest.TestCase):
    def test_receipt_requires_exact_source_pin_complete_run_and_owned_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'benchmarks/history/test-analysis/run.json'
            path.parent.mkdir(parents=True)
            receipt = {'schema_version': 1, 'status': 'complete', 'source_hash': 'source', 'source_hash_after': 'source'}
            path.write_text(json.dumps(receipt))
            with patch.object(validate, 'ROOT', root):
                self.assertEqual(validate.read_analysis_report(str(path), 'source'), receipt)
                with self.assertRaises(ValueError):
                    validate.read_analysis_report(str(path), 'different')
                with self.assertRaises(ValueError):
                    validate.read_analysis_report('', 'source')
                with self.assertRaises(ValueError):
                    validate.read_analysis_report('/unowned/receipt.json', 'source')
                receipt['status'] = 'incomplete'
                path.write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    validate.read_analysis_report(str(path), 'source')
