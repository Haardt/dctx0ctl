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

    def test_overlapping_receipts_measure_repairs_by_result_availability(self):
        identity = dict.fromkeys(('spec_sources', 'implementation', 'dependencies', 'adapter', 'configuration', 'scope', 'inputs'), 'hash-1')
        failed = dict(package_id='package', run_id='fail', identity=identity, outcome='FAIL', mandatory_complete=True, source_current=True, started_at='2026-10-21T10:00:00Z', finished_at='2026-10-21T10:03:00Z')
        early_pass = dict(failed, run_id='early', outcome='PASS', started_at='2026-10-21T10:01:00Z', finished_at='2026-10-21T10:02:00Z')
        report = metrics.receipt_metrics([failed, early_pass], {'package'})
        self.assertEqual(report['first_pass_yield'], 0)
        self.assertEqual(report['fail_to_current_pass_seconds'], [])
        repair = dict(early_pass, run_id='repaired', finished_at='2026-10-21T10:04:00Z')
        report = metrics.receipt_metrics([repair, failed, early_pass], {'package'})
        self.assertEqual(report['fail_to_current_pass_seconds'], [60])

    def test_unknown_counts_remain_unknown_and_external_ids_keep_activity(self):
        row = dict(self.usage[0], input_tokens=None, cached_input_tokens=None,
                   output_tokens=None, activity='product-implementation',
                   invocation_id='invoke-1', execution_id='exec-1', run_id='run-1')
        report = metrics.summarize(self.progress, [self.package], [row])
        self.assertIsNone(report['usage']['fresh_input_tokens'])
        self.assertIsNone(report['usage']['output_tokens'])
        self.assertIsNone(report['transitions_per_million_fresh_input_tokens'])
        correlated = report['correlation']['responses'][0]
        self.assertEqual(correlated['execution_id'], 'exec-1')
        self.assertEqual(correlated['activity'], 'product-implementation')
        self.assertEqual(report['usage']['responses'], 1)

    def test_provider_semantics_do_not_double_count_reasoning_or_cache(self):
        row = dict(self.usage[0], input_tokens=20, cached_input_tokens=80,
                   cache_semantics='separate-from-input', output_tokens=7,
                   reasoning_tokens=5, reasoning_semantics='included-in-output')
        report = metrics.summarize(self.progress, [self.package], [row])
        self.assertEqual(report['usage']['fresh_input_tokens'], 20)
        self.assertEqual(report['usage']['output_tokens'], 7)
        self.assertEqual(report['correlation']['responses'][0]['reasoning_tokens'], 5)
        for update in [dict(reasoning_tokens=8), dict(cache_semantics='guess'),
                       dict(activity='guess'), dict(invocation_id='')]:
            with self.assertRaises(ValueError):
                metrics.summarize(self.progress, [self.package], [dict(row, **update)])
