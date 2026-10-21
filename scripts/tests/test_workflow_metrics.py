import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('workflow_metrics', Path(__file__).parents[1] / 'workflow_metrics.py')
metrics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(metrics)


class WorkflowMetricsTests(unittest.TestCase):
    def setUp(self):
        self.start = dict(project='p', event_seq=1)
        self.end = dict(project='p', event_seq=3)
        self.progress = dict(before=self.start, after=self.end, newly_pass=[1, 2], added_pass=[3], lost_pass=[4])
        self.package = dict(id='package', pin=self.start, tasks=[dict(overview=dict(task_id=1))])
        self.usage = [dict(response_id='r1', package_id='package', input_tokens=100, cached_input_tokens=80, output_tokens=7)]

    def test_counts_fresh_input_once_and_only_transitions_in_exported_scope(self):
        report = metrics.summarize(self.progress, [self.package], self.usage)
        self.assertEqual(report['usage']['fresh_input_tokens'], 20)
        self.assertEqual(report['usage']['output_tokens'], 7)
        self.assertEqual(report['pass_endpoint_transitions_in_package_scope'], [1])
        self.assertEqual(report['transitions_per_million_fresh_input_tokens'], 50000)
        self.assertIsNone(metrics.summarize(self.progress, [self.package], [])['transitions_per_million_fresh_input_tokens'])

    def test_rejects_duplicate_usage_invalid_counts_and_foreign_or_out_of_order_pins(self):
        invalid = [self.usage * 2,
                   [{**self.usage[0], 'package_id': 'unknown'}],
                   [{**self.usage[0], 'cached_input_tokens': 101}],
                   [{**self.usage[0], 'output_tokens': -1}],
                   [{**self.usage[0], 'input_tokens': True}]]
        for usage in invalid:
            with self.assertRaises(ValueError): metrics.summarize(self.progress, [self.package], usage)
        for pin in [dict(project='foreign', event_seq=1), dict(project='p', event_seq=4)]:
            with self.assertRaises(ValueError): metrics.summarize(self.progress, [{**self.package, 'pin': pin}], [])
        with self.assertRaises(ValueError): metrics.summarize(self.progress, [self.package] * 2, [])
        with self.assertRaises(ValueError): metrics.summarize({**self.progress, 'before': self.end, 'after': self.start}, [self.package], [])

    def test_receipts_measure_delivery_and_repair_and_reject_partial_pass_delivery(self):
        identity = dict.fromkeys(('spec_sources', 'implementation', 'dependencies', 'adapter', 'configuration', 'scope', 'inputs'), 'hash-1')
        failed = dict(package_id='package', run_id='fail', identity=identity, outcome='FAIL', mandatory_complete=True, source_current=True, started_at='2026-10-21T10:00:00Z', finished_at='2026-10-21T10:01:00Z')
        passed = dict(failed, run_id='pass', outcome='PASS', started_at='2026-10-21T10:02:00Z', finished_at='2026-10-21T10:03:00Z', delivered_at='2026-10-21T10:05:00Z')
        report = metrics.summarize(self.progress, [self.package], self.usage, [failed, passed])['verification']
        self.assertEqual(report['first_pass_yield'], 0)
        self.assertEqual(report['identical_input_repeat_checks'], 1)
        self.assertEqual(report['evidence_to_delivery_seconds']['median'], 120)
        self.assertEqual(report['fail_to_current_pass_seconds'], [120])
        for change in [dict(mandatory_complete=False), dict(source_current=False), dict(outcome='UNKNOWN'), dict(finished_at='2026-10-21T09:00:00Z'), dict(started_at='2026-10-21T10:02:00')]:
            with self.assertRaises(ValueError): metrics.summarize(self.progress, [self.package], [], [dict(passed, **change)])
