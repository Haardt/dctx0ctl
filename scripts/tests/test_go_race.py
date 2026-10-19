import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location('go_race', SCRIPTS / 'go_race.py')
race = importlib.util.module_from_spec(spec)
spec.loader.exec_module(race)

HOST = {'GOOS': 'linux', 'GOHOSTOS': 'linux', 'GOARCH': 'arm64', 'GOHOSTARCH': 'arm64',
        'GOVERSION': 'go1.27.1', 'CGO_ENABLED': '1'}
VMA = 'FATAL: ThreadSanitizer: unsupported VMA range\nFATAL: Found 39 - Supported 48\n'


class GoRaceTests(unittest.TestCase):
    def attempt(self, positive=(0, 'RACE_PROBE_OK\n'), negative=(66, 'WARNING: DATA RACE'),
                application=(0, 'ok cinema/domain'), host=None, change_source=False, missing=False):
        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory) / 'app'
            project.mkdir()
            (project / 'go.mod').write_text('module example\n')
            source = project / 'app.go'
            source.write_text('package example\n')
            destination = Path(directory) / 'report/race.json'
            commands = []

            def measured(command, cwd, timeout, env):
                commands.append(command)
                self.assertEqual(cwd, project)
                self.assertIn('exitcode=66', env['GORACE'])
                self.assertEqual(env['GOFLAGS'], '')
                self.assertEqual(env['GOWORK'], 'off')
                if command[1:3] == ['env', '-json']:
                    result = (0, json.dumps(host or HOST))
                elif 'build' in command:
                    result = (0, '')
                elif 'positive' in Path(command[0]).stem:
                    if change_source:
                        source.write_text('package changed\n')
                    result = positive
                elif 'negative' in Path(command[0]).stem:
                    result = negative
                else:
                    result = application
                return {'exit_code': result[0], 'elapsed_ms': 1, 'peak_rss_bytes': 1024}, result[1]

            with patch.object(race.shutil, 'which', return_value=None if missing else '/bin/go'), patch.object(race, 'measured', side_effect=measured):
                code = race.run(project, destination)
            return code, json.loads(destination.read_text()), commands

    def test_success_requires_a_clean_probe_a_detected_real_race_and_uncached_package_tests(self):
        code, report, commands = self.attempt()
        self.assertEqual(code, 0)
        self.assertEqual(report['status'], 'pass')
        self.assertEqual(report['source_hash_before'], report['source_hash_after'])
        self.assertEqual(len(report['checks']), 6)
        self.assertIn('-count=1', commands[-1])
        self.assertIn('-race', commands[-1])
        self.assertIn('-mod=readonly', commands[-1])

    def test_unsupported_arm64_vma_is_unavailable_and_never_runs_or_claims_application_tests(self):
        code, report, commands = self.attempt(positive=(66, VMA))
        self.assertEqual(code, 2)
        self.assertEqual(report['status'], 'unavailable')
        self.assertEqual(report['issue'], 'RACE_HOST_VMA_UNSUPPORTED')
        self.assertEqual(len(commands), 3)
        self.assertNotIn('cinema-race', [c['name'] for c in report['checks']])
        self.assertFalse(race.unsupported_vma(VMA, HOST | {'GOARCH': 'amd64'}))
        self.assertFalse(race.unsupported_vma('FATAL: other problem', HOST))

    def test_other_runtime_errors_and_disabled_or_broken_detectors_fail(self):
        for options in ({'positive': (1, 'unexpected failure')},
                        {'positive': (0, '')}, {'negative': (0, '')},
                        {'negative': (66, 'a different runtime failure')}):
            with self.subTest(options=options):
                code, report, commands = self.attempt(**options)
                self.assertEqual(code, 1)
                self.assertEqual(report['status'], 'fail')
                self.assertFalse(any('test' in command for command in commands))

    def test_actual_application_races_and_build_errors_are_failures_not_host_unavailability(self):
        for output in ('WARNING: DATA RACE', 'application compilation failed', VMA):
            code, report, _ = self.attempt(application=(1, output))
            self.assertEqual(code, 1)
            self.assertEqual(report['status'], 'fail')
            self.assertEqual(report['checks'][-1]['name'], 'cinema-race')

    def test_source_drift_overrides_even_a_known_unsupported_host(self):
        code, report, _ = self.attempt(positive=(66, VMA), change_source=True)
        self.assertEqual(code, 1)
        self.assertEqual(report['issue'], 'RACE_SOURCE_CHANGED')

    def test_missing_toolchain_and_cross_execution_remain_unavailable(self):
        for options in ({'missing': True}, {'host': HOST | {'GOARCH': 'amd64'}}):
            code, report, commands = self.attempt(**options)
            self.assertEqual(code, 2)
            self.assertEqual(report['status'], 'unavailable')
            self.assertLessEqual(len(commands), 1)

    def test_source_inventory_ignores_generated_files_but_pins_tests_and_module_locks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'go.mod').write_text('module example\n')
            original = race.source_hash(root)
            (root / '.dctx0ctl').mkdir()
            (root / '.dctx0ctl/generated.go').write_text('generated')
            self.assertEqual(race.source_hash(root), original)
            (root / 'app_test.go').write_text('package example')
            self.assertNotEqual(race.source_hash(root), original)
