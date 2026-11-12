import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import application_test_managed_acceptance as managed


def base(phase, message, status=0, stdout=''):
    return {'result': {'status': 'UNKNOWN'}, 'adoption_authority': False, 'adopted_tasks': [],
            'issues': [{'code': 'APPLICATION_NATIVE_RUN', 'message': message}],
            'phases': [{'phase': phase, 'audit': {'exit_code': status, 'stdout': stdout}}]}


class ManagedProbeTests(unittest.TestCase):
    def test_missing_root_requires_real_successful_catalog_without_adoption(self):
        evidence = base('catalog', 'Bun registration catalog differs from its required native root', stdout='{"tests":[{"name":"RemovedCinemaSuite"}]}')
        managed.assert_native_probe('missing-test', 'typescript', evidence)
        for mutate in (
            lambda value: value['phases'][0]['audit'].update(exit_code=1),
            lambda value: value['phases'][0]['audit'].update(stdout='{"tests":[{"name":"CinemaSuite"}]}'),
            lambda value: value.update(adoption_authority=True),
            lambda value: value.update(adopted_tasks=[1]),
            lambda value: value['issues'][0].update(code='ADAPTER_ERROR'),
        ):
            bad = copy.deepcopy(evidence)
            mutate(bad)
            with self.assertRaises(AssertionError):
                managed.assert_native_probe('missing-test', 'typescript', bad)

    def test_build_probe_cannot_pass_from_preflight_or_catalog_failure(self):
        evidence = base('build-1', 'Application test build-1 exited Some(2)', 2)
        managed.assert_native_probe('build-failure', 'typescript', evidence)
        for phase, status in [('build-1', 0), ('catalog', 2), ('prepare-0', 1)]:
            bad = copy.deepcopy(evidence)
            bad['phases'][0].update(phase=phase)
            bad['phases'][0]['audit']['exit_code'] = status
            with self.assertRaises(AssertionError):
                managed.assert_native_probe('build-failure', 'typescript', bad)

    def test_late_native_failure_requires_complete_central_pass(self):
        evidence = base('native-tests', 'Application test native-tests exited Some(1)', 1)
        evidence['assessment'] = {'result': {'status': 'PASS'}, 'cases': [{'result': {'status': 'PASS'}} for _ in range(5766)], 'missing_cases': [], 'missing_tests': [], 'issues': []}
        managed.assert_native_probe('native-assertion', 'go', evidence)
        bad = copy.deepcopy(evidence)
        bad['assessment']['cases'][0]['result']['status'] = 'FAIL'
        with self.assertRaises(AssertionError):
            managed.assert_native_probe('native-assertion', 'go', bad)
        bad = copy.deepcopy(evidence)
        bad['assessment']['cases'].pop()
        with self.assertRaises(AssertionError):
            managed.assert_native_probe('native-assertion', 'go', bad)

    def test_created_tasks_must_remain_unverified_and_unclaimed(self):
        initial = {'states': [], 'events': {}, 'links': 0, 'adoptions': 0}
        state = {'work': 'open', 'check': 'unverified', 'result': None, 'evidence': [], 'reviews': [], 'actor': None, 'claim': None, 'reservation': None}
        after = {'states': [copy.deepcopy(state) for _ in range(105)], 'events': {'created': 105, 'plan': 1}, 'links': 0, 'adoptions': 0}
        managed.assert_unpublished_task_plan(initial, after)
        for key, value in [('work', 'done'), ('check', 'pass'), ('result', {'value': 1}), ('reviews', [{'approved': True}]), ('claim', {'actor': 'other'})]:
            bad = copy.deepcopy(after)
            bad['states'][0][key] = value
            with self.assertRaises(AssertionError):
                managed.assert_unpublished_task_plan(initial, bad)
        bad = copy.deepcopy(after)
        bad['links'] = 1
        with self.assertRaises(AssertionError):
            managed.assert_unpublished_task_plan(initial, bad)
        bad = copy.deepcopy(after)
        bad['adoptions'] = 1
        with self.assertRaises(AssertionError):
            managed.assert_unpublished_task_plan(initial, bad)


if __name__ == '__main__':
    unittest.main()
