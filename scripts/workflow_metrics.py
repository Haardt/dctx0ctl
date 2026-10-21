#!/usr/bin/env python3
"""Join exported task progress/packages with external per-response usage, without billing estimates."""
import argparse
import datetime as dt
import math
import statistics
import json
from pathlib import Path
import sys


def body(value):
    return value.get('body', {}).get('data', value)


def receipt_metrics(receipts, package_ids):
    """Timestamped external receipts retain attribution and source-current limitations."""
    seen, groups, identities = set(), {}, set()
    latencies, repairs = [], []
    repeated = 0
    def stamp(value):
        if not isinstance(value, str):
            raise ValueError('Receipt timestamps must be strings')
        result = dt.datetime.fromisoformat(value.replace('Z', '+00:00'))
        if result.tzinfo is None:
            raise ValueError('Receipt timestamps require an explicit timezone')
        return result
    for receipt in receipts:
        if receipt['package_id'] not in package_ids or not isinstance(receipt['run_id'], str) or not receipt['run_id'] or receipt['run_id'] in seen:
            raise ValueError('Receipts need exported packages and unique nonempty run IDs')
        seen.add(receipt['run_id'])
        if receipt['outcome'] not in ('PASS', 'FAIL', 'UNKNOWN'):
            raise ValueError('Unknown receipt outcome')
        if type(receipt['mandatory_complete']) is not bool or type(receipt['source_current']) is not bool:
            raise ValueError('Receipt completeness/freshness must be explicit booleans')
        keys = ('spec_sources', 'implementation', 'dependencies', 'adapter', 'configuration', 'scope', 'inputs')
        identity = tuple(receipt['identity'][key] for key in keys)
        if any(not isinstance(value, str) or not value for value in identity):
            raise ValueError('Receipt identity components must be nonempty hashes')
        if identity in identities:
            repeated += 1
        identities.add(identity)
        start, finish = stamp(receipt['started_at']), stamp(receipt['finished_at'])
        if finish < start:
            raise ValueError('Receipt finish precedes start')
        groups.setdefault(receipt['package_id'], []).append((start, finish, receipt))
        if receipt.get('delivered_at') is not None:
            delivered = stamp(receipt['delivered_at'])
            if delivered < finish or receipt['outcome'] != 'PASS' or not receipt['mandatory_complete'] or not receipt['source_current']:
                raise ValueError('Delivery latency needs a complete source-current PASS and ordered timestamps')
            latencies.append((delivered - finish).total_seconds())
    first_pass = 0
    for checks in groups.values():
        checks.sort(key=lambda row: row[0])
        first = checks[0][2]
        first_pass += first['outcome'] == 'PASS' and first['mandatory_complete'] and first['source_current']
        failed_at = None
        # Concurrent checks may finish in a different order from their starts.
        for start, finish, receipt in sorted(checks, key=lambda row: row[1]):
            if receipt['outcome'] == 'FAIL' and failed_at is None:
                failed_at = finish
            if failed_at and receipt['outcome'] == 'PASS' and receipt['mandatory_complete'] and receipt['source_current']:
                repairs.append((finish - failed_at).total_seconds())
                failed_at = None
    return {
        'receipts': len(receipts), 'checked_packages': len(groups),
        'first_pass_yield': first_pass / len(groups) if groups else None,
        'identical_input_repeat_checks': repeated,
        'evidence_to_delivery_seconds': {'samples': latencies,
            'median': statistics.median(latencies) if latencies else None,
            'p95_nearest_rank': sorted(latencies)[math.ceil(len(latencies) * .95) - 1] if latencies else None},
        'fail_to_current_pass_seconds': repairs,
    }


def summarize(progress, packages, usage, receipts=None):
    progress = body(progress)
    before, after = progress['before'], progress['after']
    if before['project'] != after['project'] or before['event_seq'] > after['event_seq']:
        raise ValueError('Progress must have ordered pins for one project')
    by_id = {}
    for raw in packages:
        package = body(raw)
        pin = package['pin']
        if pin['project'] != after['project'] or not before['event_seq'] <= pin['event_seq'] <= after['event_seq']:
            raise ValueError('Package lies outside the project/event interval')
        if package['id'] in by_id:
            raise ValueError('Duplicate package ID')
        by_id[package['id']] = package
    totals = {key: 0 for key in ('fresh_input_tokens', 'cached_input_tokens', 'output_tokens', 'responses')}
    rows = {key: dict(totals) for key in by_id}
    seen = set()
    for row in usage:
        response = row['response_id']
        if not isinstance(response, str) or not response or response in seen:
            raise ValueError('Response IDs must be nonempty and unique across sources')
        seen.add(response)
        if row['package_id'] not in by_id:
            raise ValueError('Usage references an unexported package')
        counts = [row[key] for key in ('input_tokens', 'cached_input_tokens', 'output_tokens')]
        if any(type(value) is not int or value < 0 for value in counts) or counts[1] > counts[0]:
            raise ValueError('Usage requires nonnegative integer counts and cache <= input')
        values = dict(zip(('fresh_input_tokens', 'cached_input_tokens', 'output_tokens', 'responses'),
                          (counts[0] - counts[1], counts[1], counts[2], 1)))
        for key, value in values.items():
            totals[key] += value
            rows[row['package_id']][key] += value
    covered = {task['overview']['task_id'] for package in by_id.values() for task in package['tasks']}
    transitions = set(progress['newly_pass']) | set(progress.get('added_pass', []))
    scoped = transitions & covered
    return {
        'schema_version': 1, 'before': before, 'after': after,
        'usage': totals, 'packages': rows, 'verification': receipt_metrics(receipts or [], by_id),
        'pass_endpoint_transitions_in_package_scope': sorted(scoped),
        'removed_active_pass': progress['lost_pass'],
        'transitions_per_million_fresh_input_tokens': len(scoped) * 1_000_000 / totals['fresh_input_tokens'] if totals['fresh_input_tokens'] else None,
        'limitations': [
            'External usage attribution is supplied by the caller; unassigned responses are not measured.',
            'Endpoint PASS transitions are historical state changes, not current-source or feature acceptance.',
            'Task sizes and scopes must match before comparing efficiency. No billing or elapsed CPU claim.',
            'Receipt identities, timestamps and source_current are external assertions, not revalidated native evidence.',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--progress', type=Path, required=True)
    parser.add_argument('--package', type=Path, action='append', required=True)
    parser.add_argument('--usage', type=Path, required=True, help='JSON array: response_id, package_id, input_tokens, cached_input_tokens, output_tokens')
    parser.add_argument('--receipts', type=Path, help='Optional timestamped execution/delivery receipts; see workflow-efficiency.md')
    args = parser.parse_args()
    try:
        result = summarize(json.loads(args.progress.read_text()),
                           [json.loads(path.read_text()) for path in args.package],
                           json.loads(args.usage.read_text()),
                           json.loads(args.receipts.read_text()) if args.receipts else [])
    except (KeyError, TypeError, ValueError, OSError) as error:
        parser.error(str(error))
    json.dump(result, sys.stdout, indent=2)
    print()


if __name__ == '__main__':
    main()
