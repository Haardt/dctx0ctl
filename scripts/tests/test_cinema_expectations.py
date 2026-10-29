import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('cinema_expectations', Path(__file__).resolve().parents[1] / 'cinema_expectations.py')
cinema = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cinema)


class CinemaInventoryTests(unittest.TestCase):
    def reference(self):
        cases = [{'id': f'{kind}:{index}', 'kind': kind, 'result': {'status': 'PASS'}}
                 for kind, count in cinema.EXPECTED_CASE_KINDS.items() for index in range(count)]
        return {'required_cases': [case['id'] for case in cases], 'cases': cases,
                'provenance': {'incomplete': []}}

    def test_exact_inventory_rejects_missing_duplicate_wrong_family_nonpass_and_incomplete_cases(self):
        reference = self.reference()
        cinema.assert_native_cinema(reference)
        mutations = []
        missing = copy.deepcopy(reference)
        missing['cases'].pop()
        mutations.append(missing)
        duplicate = copy.deepcopy(reference)
        duplicate['required_cases'][0] = duplicate['required_cases'][1]
        mutations.append(duplicate)
        wrong_id = copy.deepcopy(reference)
        wrong_id['cases'][0]['id'] = 'unrequested'
        mutations.append(wrong_id)
        wrong_kind = copy.deepcopy(reference)
        wrong_kind['cases'][0]['kind'] = 'operation'
        mutations.append(wrong_kind)
        for status in ('FAIL', 'UNKNOWN'):
            nonpass = copy.deepcopy(reference)
            nonpass['cases'][0]['result'] = {'status': status}
            mutations.append(nonpass)
        incomplete = copy.deepcopy(reference)
        incomplete['provenance']['incomplete'] = ['missing native observer']
        mutations.append(incomplete)
        for run in mutations:
            with self.assertRaises(AssertionError):
                cinema.assert_native_cinema(run)
