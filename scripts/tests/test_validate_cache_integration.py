import argparse
import contextlib
import importlib.util
import io
import os
import sqlite3
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location('validate_cache_test', Path(__file__).resolve().parents[1] / 'validate.py')
validate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validate)


class ValidationCacheIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.cwd = Path.cwd()
        self.addCleanup(os.chdir, self.cwd)
        environment = patch.dict(os.environ)
        environment.start()
        self.addCleanup(environment.stop)
        os.environ.pop('DCTX0_PRESERVE_BENCHMARK_HISTORY', None)
        os.environ.pop('DCTX0_VALIDATION_ARTIFACT_ROOT', None)

    def test_only_complete_workspace_coverage_with_performance_is_eligible(self):
        args = argparse.Namespace(coverage=True, focus='workspace', example='all',
                                  skip_performance=False, no_reuse=False)
        self.assertTrue(validate.cache_eligible(args))
        for name, value in [('coverage', False), ('focus', 'verification'),
                            ('example', 'cinema'), ('skip_performance', True),
                            ('no_reuse', True)]:
            with self.subTest(name=name):
                modified = argparse.Namespace(**(vars(args) | {name: value}))
                self.assertFalse(validate.cache_eligible(modified))

    def test_hit_reports_original_receipt_and_runs_no_checks_or_history_writes(self):
        identity = {'fingerprint': 'same'}
        receipt = {'id': 42, 'created': '2026-11-09T12:00:00+00:00',
                   'identity': identity, 'artifacts': []}
        output = io.StringIO()
        with patch.object(validate.sys, 'argv', ['validate.py', '--coverage', '--reuse-max-age-seconds', '90']), \
             patch.object(validate.validation_cache, 'identity', return_value=identity), \
             patch.object(validate.validation_cache, 'find_receipt', return_value=receipt) as find, \
             patch.object(validate.validation_cache, 'record_success') as publish, \
             patch.object(validate.subprocess, 'run') as run, \
             patch.object(validate, 'benchmark') as benchmark, \
             patch.object(validate, 'save') as save, \
             patch.object(validate, 'accept') as accept, contextlib.redirect_stdout(output):
            self.assertEqual(validate.main(), 0)
        self.assertIn('REUSED', output.getvalue())
        self.assertIn('2026-11-09', output.getvalue())
        self.assertIn('42', output.getvalue())
        self.assertEqual(find.call_args.args[:2], (validate.ROOT, identity))
        self.assertEqual(find.call_args.kwargs.get('max_age_seconds'), 90)
        for mock in (publish, run, benchmark, save, accept):
            mock.assert_not_called()

    def test_corrupt_cache_falls_back_to_execution(self):
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(validate.sys, 'argv', ['validate.py', '--coverage']), \
             patch.object(validate.validation_cache, 'identity', return_value={'same': True}), \
             patch.object(validate.validation_cache, 'find_receipt', side_effect=sqlite3.DatabaseError('corrupt')), \
             patch.object(validate, 'functional_cache_root', return_value=temporary), \
             patch.object(validate.subprocess, 'run', side_effect=RuntimeError('first actual check')) as run, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            with self.assertRaisesRegex(RuntimeError, 'first actual check'):
                validate.main()
        run.assert_called_once()
        self.assertIn('executing checks', output.getvalue())

    def test_force_and_partial_runs_do_not_reuse_a_receipt(self):
        cases = [(['--coverage', '--force'], True),
                 (['--coverage', '--no-reuse'], False),
                 ([], False),
                 (['--focus', 'verification'], False),
                 (['--coverage', '--example', 'cinema'], False),
                 (['--coverage', '--skip-performance'], False)]
        for arguments, creates_identity in cases:
            with self.subTest(arguments=arguments), tempfile.TemporaryDirectory() as temporary, \
                 patch.object(validate.sys, 'argv', ['validate.py', *arguments]), \
                 patch.object(validate.validation_cache, 'identity', return_value={'fingerprint': 'same'}) as identity, \
                 patch.object(validate.validation_cache, 'find_receipt') as find, \
                 patch.object(validate.validation_cache, 'record_success') as publish, \
                 patch.object(validate, 'functional_cache_root', return_value=temporary), \
                 patch.object(validate.subprocess, 'run', side_effect=RuntimeError('first command')) as run, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, 'first command'):
                    validate.main()
            self.assertEqual(identity.called, creates_identity)
            find.assert_not_called()
            publish.assert_not_called()
            run.assert_called_once()

    def test_only_successful_complete_unchanged_validation_can_publish(self):
        args = argparse.Namespace(coverage=True, focus='workspace', example='all',
                                  skip_performance=False, no_reuse=False)
        good = {'status': 'complete', 'validation_status': 'pass',
                'validation_steps': [{'returncode': 0}], 'source_hash': 'source',
                'validation_source_hash': 'source', 'validation_source_hash_after': 'source'}
        cases = [({'status': 'incomplete'}, True, True, 'same'),
                 ({'validation_status': 'fail'}, True, True, 'same'),
                 ({'validation_steps': [{'returncode': 1}]}, True, True, 'same'),
                 ({'validation_steps': []}, True, True, 'same'),
                 ({'validation_source_hash_after': 'changed'}, True, True, 'same'),
                 ({'source_hash': 'changed'}, True, True, 'same'),
                 ({}, False, True, 'same'), ({}, True, False, 'same'),
                 ({}, True, True, 'changed')]
        with tempfile.TemporaryDirectory() as temporary, patch.object(validate, 'ROOT', Path(temporary)):
            for changes, passed, gate, final_identity in cases:
                with self.subTest(changes=changes, passed=passed, gate=gate, final_identity=final_identity), \
                     patch.object(validate.validation_cache, 'identity', return_value=final_identity), \
                     patch.object(validate.validation_cache, 'record_success') as publish, \
                     contextlib.redirect_stdout(io.StringIO()):
                    self.assertIsNone(validate.finish_validation_cache(args, 'same', good | changes, passed, gate))
                    publish.assert_not_called()

    def test_successful_unchanged_result_publishes_evidence(self):
        args = argparse.Namespace(coverage=True, focus='workspace', example='all',
                                  skip_performance=False, no_reuse=False)
        record = {'status': 'complete', 'validation_status': 'pass',
                  'validation_steps': [{'returncode': 0}], 'source_hash': 'source',
                  'validation_source_hash': 'source', 'validation_source_hash_after': 'source'}
        receipt = {'id': 1, 'created': '2026-11-09T12:00:00+00:00'}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(validate, 'ROOT', root), \
                 patch.object(validate, 'artifact_root', return_value=root / 'target'), \
                 patch.object(validate.validation_cache, 'identity', return_value='same'), \
                 patch.object(validate.validation_cache, 'record_success', return_value=receipt) as publish, \
                 contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(validate.finish_validation_cache(args, 'same', record, True, True), receipt)
            published_root, identity, artifacts = publish.call_args.args
            self.assertEqual((published_root, identity), (root, 'same'))
            self.assertTrue(artifacts[0].is_file())
            self.assertEqual(artifacts[1], root / 'target/coverage/source-summary.json')

    def test_baseline_promotion_never_reads_or_writes_cache(self):
        with patch.object(validate.sys, 'argv', ['validate.py', '--coverage', '--accept-baseline', 'latest']), \
             patch.object(validate, 'accept') as accept, \
             patch.object(validate.validation_cache, 'identity') as identity, \
             patch.object(validate.validation_cache, 'find_receipt') as find, \
             patch.object(validate.validation_cache, 'record_success') as publish:
            self.assertEqual(validate.main(), 0)
        accept.assert_called_once_with('latest')
        for mock in (identity, find, publish):
            mock.assert_not_called()

    def test_invalid_arguments_are_rejected_before_cache_lookup(self):
        with patch.object(validate.sys, 'argv', ['validate.py', '--coverage', '--iterations', '0']), \
             patch.object(validate.validation_cache, 'identity') as identity, \
             patch.object(validate.validation_cache, 'find_receipt') as find, \
             contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                validate.main()
        identity.assert_not_called()
        find.assert_not_called()


if __name__ == '__main__':
    unittest.main()
