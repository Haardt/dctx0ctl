import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('coverage_runner', Path(__file__).resolve().parents[1] / 'coverage.py')
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


class CoverageTests(unittest.TestCase):
    def test_merging_keeps_real_uncovered_lines_and_combines_instantiations(self):
        result = coverage.merged_lines('SF:a.rs\nDA:1,2\nDA:2,0\nDA:3,0\nend_of_record\nSF:a.rs\nDA:2,3\nend_of_record\nSF:b.rs\nDA:1,0\nend_of_record')
        self.assertEqual(result['lines'], 4)
        self.assertEqual(result['covered_lines'], 2)
        self.assertEqual(result['line_percent'], 50)
        self.assertEqual(result['uncovered_lines'], [{'file': 'a.rs', 'line': 3}, {'file': 'b.rs', 'line': 1}])

    def test_empty_malformed_or_negative_counts_fail(self):
        for text in ['', 'DA:1,1', 'SF:a.rs\nDA:0,1', 'SF:a.rs\nDA:1,-1', 'SF:a.rs\nDA:invalid,1']:
            with self.assertRaises(ValueError):
                coverage.merged_lines(text)


if __name__ == '__main__':
    unittest.main()
