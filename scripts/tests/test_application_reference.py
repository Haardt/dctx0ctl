import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from application_reference import effective_plan_hash, reference_comparison, validate_current_assessment


class ApplicationReferenceTests(unittest.TestCase):
    def setUp(self):
        self.program = {'snapshot': 'one', 'check_occurrence': 1, 'contracts': ['a'], 'budgets': {'cases': 5}}
        self.manifest = {'required_cases': ['a']}
        self.old = {'program': self.program, 'manifest': self.manifest}
        self.plan = {'program': copy.deepcopy(self.program), 'manifest': self.manifest, 'case_groups': {'a': 'native'}, 'registrations': [{'group': 'native', 'cases': ['a']}]}
        self.case = {'id': 'a', 'result': {'status': 'PASS'}, 'bindings': {'value': 1}, 'arguments': [1], 'observations': [{'invocation': {'subject': 'a'}}]}
        self.report = {'required_cases': ['a'], 'cases': [self.case], 'result': {'status': 'PASS'}, 'finalized': True, 'missing_cases': [], 'missing_tests': [], 'issues': []}

    def test_compatible_plan_compares_actual_witnesses_and_allows_occurrence_change(self):
        self.plan['program']['snapshot'] = 'two'
        self.plan['program']['check_occurrence'] = 9
        result = reference_comparison(self.old, self.plan, [self.case], [self.case])
        self.assertTrue(result['compatible'])
        self.assertEqual(result['different_invocations'], [])
        changed = copy.deepcopy(self.case)
        changed['arguments'] = [2]
        self.assertEqual(reference_comparison(self.old, self.plan, [changed], [self.case])['different_arguments'], ['a'])
        self.assertEqual(reference_comparison(self.old, self.plan, [self.case], [])['missing_reference_cases'], ['a'])

    def test_changed_plan_reports_unassessed_without_lookup_of_old_case_ids(self):
        self.plan['program']['contracts'] = ['new']
        current = copy.deepcopy(self.case)
        current['id'] = 'new'
        result = reference_comparison(self.old, self.plan, [current], [self.case])
        self.assertFalse(result['compatible'])
        self.assertEqual(result['status'], 'incompatible')
        self.assertIsNone(result['different_arguments'])

    def test_workload_hash_ignores_publication_but_binds_budget_inputs_and_boundary(self):
        bundle = {'plan': self.plan, 'boundary': 'json-decoder'}
        initial = effective_plan_hash(bundle, 'decoder')
        changed = copy.deepcopy(bundle)
        changed['plan']['program']['snapshot'] = 'later'
        changed['plan']['program']['check_occurrence'] = 99
        self.assertEqual(initial, effective_plan_hash(changed, 'decoder'))
        changed['plan']['program']['budgets']['cases'] += 1
        self.assertNotEqual(initial, effective_plan_hash(changed, 'decoder'))
        changed = copy.deepcopy(bundle)
        changed['plan']['program']['contracts'].append('new input')
        self.assertNotEqual(initial, effective_plan_hash(changed, 'decoder'))
        self.assertNotEqual(initial, effective_plan_hash(bundle, 'all'))
        changed = copy.deepcopy(bundle)
        changed['boundary'] = 'value-constructor'
        self.assertNotEqual(initial, effective_plan_hash(changed, 'decoder'))

    def test_current_assessment_requires_every_assigned_case_and_clean_finalization(self):
        validate_current_assessment(self.plan, 'native', self.report)
        for mutate in (
            lambda r: r.update(cases=[]),
            lambda r: r.update(required_cases=[]),
            lambda r: r.update(finalized=False),
            lambda r: r.update(issues=[{'code': 'FAULT'}]),
            lambda r: r['cases'][0].update(result={'status': 'FAIL'}),
        ):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaises(AssertionError):
                validate_current_assessment(self.plan, 'native', changed)
        bad_plan = copy.deepcopy(self.plan)
        bad_plan['registrations'][0]['cases'] = []
        with self.assertRaises(AssertionError):
            validate_current_assessment(bad_plan, 'native', self.report)


if __name__ == '__main__':
    unittest.main()
