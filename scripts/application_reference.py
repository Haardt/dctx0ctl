"""Compare historical evidence only when its complete finite plan matches.

Current case ownership and central assessment are validated independently of
historical comparison, which never supplies evidence for a current case.
"""
import hashlib
import json

def semantic(program):
    return {key: value for key, value in program.items() if key not in ('check_occurrence', 'snapshot')}


def effective_plan_hash(bundle, form):
    """Identify the actual finite workload, independent of publication pins."""
    projection = {
        'schema_version': 1,
        'program': semantic(bundle['plan']['program']),
        'manifest': bundle['plan']['manifest'],
        'case_groups': bundle['plan']['case_groups'],
        'registrations': bundle['plan']['registrations'],
        'configuration': bundle['plan'].get('configuration'),
        'boundary': bundle['boundary'],
        'form': form,
    }
    encoded = json.dumps(projection, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode('utf-8')
    return hashlib.sha256(encoded).hexdigest()


def reference_comparison(old, current_plan, cases, old_cases):
    same_program = semantic(old['program']) == semantic(current_plan['program'])
    same_manifest = old['manifest'] == current_plan['manifest']
    compatible = same_program and same_manifest
    comparison = {
        'status': 'assessed' if compatible else 'incompatible',
        'compatible': compatible,
        'identical_finite_plan': same_program,
        'identical_manifest': same_manifest,
        'different_invocations': None,
        'different_arguments': None,
        'different_results': None,
        'different_inputs': None,
        'missing_reference_cases': None,
    }
    if not compatible:
        return comparison
    reference = {case['id']: case for case in old_cases}
    missing = [case['id'] for case in cases if case['id'] not in reference]
    shared = [case for case in cases if case['id'] in reference]
    comparison.update(
        missing_reference_cases=missing,
        different_invocations=[case['id'] for case in shared if [item['invocation'] for item in case['observations']] != [item['invocation'] for item in reference[case['id']]['observations']]],
        different_arguments=[case['id'] for case in shared if case['arguments'] != reference[case['id']]['arguments']],
        different_results=[case['id'] for case in shared if case['result'] != reference[case['id']]['result']],
        different_inputs=[case['id'] for case in shared if case['bindings'].get('value') != reference[case['id']]['bindings'].get('value')],
    )
    return comparison


def validate_current_assessment(plan, group, assessment):
    """Require exact current group ownership and a complete central PASS."""
    assigned = {case for case, owner in plan['case_groups'].items() if owner == group}
    registrations = [registration for registration in plan['registrations'] if registration['group'] == group]
    registered = [case for registration in registrations for case in registration['cases']]
    required = assessment['required_cases']
    actual = [case['id'] for case in assessment['cases']]
    assert assigned and len(registered) == len(set(registered)) and set(registered) == assigned, 'Current registration ownership differs from assigned cases'
    assert len(required) == len(set(required)) and set(required) == assigned, 'Central required cases differ from current assignment'
    assert len(actual) == len(set(actual)) and set(actual) == assigned, 'Central case results differ from current assignment'
    assert assessment['result']['status'] == 'PASS' and assessment['finalized'] and not assessment['missing_cases'] and not assessment['missing_tests'] and not assessment['issues'], 'Incomplete central assessment'
    assert all(case['result']['status'] == 'PASS' for case in assessment['cases']), 'Current central case did not pass'
