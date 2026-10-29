"""Reviewed finite Cinema corpus, shared by native acceptance and measurements.

2026-10-27: values-6/rejections-3 plus the explicit castVote preparation expand
the previous 5,674 cases to 5,766. The specification and production sources are
unchanged. Exact kind counts prevent a missing obligation family being hidden by
an unrelated increase elsewhere. Never infer these expectations from a run.
"""
from collections import Counter

EXPECTED_CASE_KINDS = {
    'contract': 3862,
    'operation': 1784,
    'assertion': 101,
    'example': 12,
    'entity-identity': 5,
    'lifecycle': 2,
}
EXPECTED_NATIVE_CASES = sum(EXPECTED_CASE_KINDS.values())


def assert_native_cinema(run):
    assert len(run['required_cases']) == len(run['cases']) == EXPECTED_NATIVE_CASES, (
        f"Cinema case inventory changed: {len(run['cases'])}/{len(run['required_cases'])}; "
        f"expected {EXPECTED_NATIVE_CASES}"
    )
    assert len(set(run['required_cases'])) == EXPECTED_NATIVE_CASES, 'Duplicate required case IDs'
    assert {case['id'] for case in run['cases']} == set(run['required_cases']), 'Case identities differ'
    assert Counter(case['kind'] for case in run['cases']) == EXPECTED_CASE_KINDS, 'Case families changed'
    assert all(case['result']['status'] == 'PASS' for case in run['cases']), [
        (case['id'], case['result']) for case in run['cases'] if case['result']['status'] != 'PASS'
    ][:5]
    assert not run['provenance']['incomplete'], run['provenance']['incomplete']
