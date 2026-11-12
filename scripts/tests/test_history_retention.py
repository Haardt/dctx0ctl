import argparse
import contextlib
import io
import json
import os
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import docs_site
import history_retention
import release_acceptance
import validate


class RetentionTests(unittest.TestCase):
    def test_real_writer_preserves_existing_failed_and_successful_then_default_prunes(self):
        for module in (release_acceptance, docs_site):
            for preserve in (True, False):
                with self.subTest(writer=module.__name__, preserve=preserve), tempfile.TemporaryDirectory() as tmp:
                    history = Path(tmp) / 'history'
                    history.mkdir()
                    originals = {}
                    for i in range(25):
                        path = history / f'{i:02}.json'
                        path.write_text(json.dumps({'status': 'failed' if i % 2 else 'complete'}))
                        originals[path] = path.read_bytes()
                    env = {history_retention.ENVIRONMENT_KEY: '1'} if preserve else {}
                    with patch.dict(os.environ, env, clear=True), patch.object(module, 'HISTORY', history):
                        saved = module.save({'id': 'new', 'status': 'failed'})
                    self.assertEqual(json.loads(saved.read_text())['history_retention'], 'preserve-all' if preserve else 'latest-20')
                    self.assertEqual(len(list(history.glob('*.json'))), 26 if preserve else 20)
                    if preserve:
                        self.assertTrue(all(path.read_bytes() == data for path, data in originals.items()))
                    else:
                        self.assertTrue(all(not path.exists() for path in list(originals)[:6]))

    def test_preserve_flag_reaches_all_validation_child_invocations(self):
        calls = []
        inherited = []
        actual_run = subprocess.run
        def run(command, **kwargs):
            environment = kwargs.get('env', os.environ)
            calls.append((command, environment.get(history_retention.ENVIRONMENT_KEY)))
            if any(name in command for name in ('scripts/coverage.py', 'scripts/query_acceptance.py', 'scripts/test_analysis_acceptance.py')):
                child = actual_run([sys.executable, '-c', 'import os;print(os.getenv("DCTX0_PRESERVE_BENCHMARK_HISTORY"))'], env=environment, capture_output=True, text=True, check=True)
                inherited.append(child.stdout.strip())
            return argparse.Namespace(returncode=0, stdout='')
        value = {'status': 'complete', 'source_hash': 'same', 'measurements': {}}
        short_base = Path.home() / '.cache'
        short_base.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='t-', dir=short_base) as temp, patch.dict(os.environ, {'XDG_CACHE_HOME': temp}, clear=True), \
             patch.object(validate.sys, 'argv', ['validate.py', '--preserve-history', '--coverage', '--samples', '1', '--iterations', '1']), \
             patch.object(validate, 'cache_eligible', return_value=False), \
             patch.object(validate.subprocess, 'run', side_effect=run), \
             patch.object(validate, 'ROOT', Path(temp)), patch.object(validate.os, 'chdir'), patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'digest', return_value='same'), \
             patch.object(validate, 'benchmark', return_value=value), patch.object(validate, 'save', return_value=True), \
             patch.object(validate, 'read_analysis_report', return_value={'measurements': {}, 'memory_scope': 'test'}), \
             contextlib.redirect_stdout(io.StringIO()):
            validate.main()
            runs = list((Path(temp) / 'target/validation-runs').iterdir())
            self.assertEqual(len(runs), 1)
            marker = json.loads((runs[0] / 'run.json').read_text())
            self.assertTrue(Path(marker['functional_cache']).is_dir())
            self.assertEqual(marker['artifact_root'], str(runs[0].resolve()))
            self.assertEqual(json.loads((runs[0] / 'run.json').read_text())['status'], 'complete')
        self.assertTrue(any('scripts/coverage.py' in command for command, _ in calls))
        self.assertTrue(any('scripts/query_acceptance.py' in command for command, _ in calls))
        self.assertTrue(any('scripts/test_analysis_acceptance.py' in command for command, _ in calls))
        self.assertTrue(all(env == '1' for _, env in calls))
        self.assertEqual(inherited, ['1', '1', '1'])
        go_report = next(command for command, _ in calls if 'scripts/application_test_acceptance.py' in command)
        self.assertIn('validation-runs', go_report[go_report.index('--report') + 1])


    def test_interrupted_hold_keeps_cache_and_incomplete_marker(self):
        short_base = Path.home() / '.cache'
        short_base.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='t-', dir=short_base) as tmp, patch.dict(os.environ, {'XDG_CACHE_HOME': tmp}, clear=True), \
             patch.object(validate, 'ROOT', Path(tmp)), patch.object(validate.os, 'chdir'), \
             patch.object(validate.sys, 'argv', ['validate.py', '--preserve-history']), \
             patch.object(validate.subprocess, 'run', side_effect=RuntimeError('interrupted')), \
             patch.object(validate, 'validation_sources', return_value=[]), patch.object(validate, 'digest', return_value='same'), \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, 'interrupted'):
                validate.main()
            runs = list((Path(tmp) / 'target/validation-runs').iterdir())
            self.assertEqual(len(runs), 1)
            marker = json.loads((runs[0] / 'run.json').read_text())
            self.assertTrue(Path(marker['functional_cache']).is_dir())
            self.assertEqual(marker['status'], 'incomplete')
            self.assertEqual(marker['artifact_root'], str(runs[0].resolve()))

    def test_nested_save_without_namespace_flag_honors_environment(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(validate, 'STORE', Path(tmp)), \
             patch.dict(os.environ, {history_retention.ENVIRONMENT_KEY: '1'}, clear=True), contextlib.redirect_stdout(io.StringIO()):
            history = Path(tmp) / 'history'
            history.mkdir()
            for i in range(25):
                (history / f'{i:02}.json').write_text(json.dumps({'status': 'incomplete'}))
            value = {'status': 'incomplete', 'measurements': {}}
            validate.save(value, argparse.Namespace(speed_tolerance=20, ram_tolerance=10))
            self.assertEqual(value['history_retention'], 'preserve-all')
            self.assertEqual(len(list(history.glob('*.json'))), 26)


if __name__ == '__main__':
    unittest.main()
