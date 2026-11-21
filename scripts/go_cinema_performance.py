#!/usr/bin/env python3
"""Final-stage release costs for Go Cinema; never promotes a benchmark baseline."""
import shutil
import history_retention
from native_performance_workspace import native_workspace, filesystem_type, atomic_json, measure_json
from verification_storage import saved_verification_artifacts
from cinema_expectations import assert_native_cinema
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
from release_support import measured

from go_cinema_acceptance import ROOT, SOURCE, copy_application


def measure(command):
    return measure_json(command, ROOT)


def artifacts(project, run_id):
    rows = saved_verification_artifacts(project / "dctx0/.dctx0ctl/state.sqlite3", run_id)
    result = {}
    for media, raw, expected in rows:
        if hashlib.sha256(raw).hexdigest() != expected:
            raise RuntimeError('Measured artifact hash mismatch')
        if media in ('application/vnd.dctx0.shrink+json', 'application/vnd.dctx0.lifecycle-shrink+json'):
            result.setdefault('shrinks', []).append(json.loads(raw))
        if media in ('application/vnd.dctx0.go-build-inputs+json', 'application/vnd.dctx0.build-audit+json', 'application/vnd.dctx0.runtime-metrics+json'):
            result[media] = json.loads(raw)
    return result



def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    if not Path('/proc/self/status').exists():
        raise RuntimeError('Go release RSS measurement is accepted only on Linux')
    binary = args.binary.resolve()
    report = {'schema_version': 1, 'target': 'go', 'status': 'incomplete', 'binary_hash': hashlib.sha256(binary.read_bytes()).hexdigest(), 'seed': 42, 'generated_cases': 100, 'samples': {}, 'baseline': 'unassessed; never promoted automatically', 'cache': 'first project build and repeated warm build; existing module/toolchain caches', 'memory_scope': 'wait4 ru_maxrss: largest reaped CLI/descendant process, not collective simultaneous RSS; worker reports its own VmHWM separately'}
    preserve = history_retention.preserve_history()
    with native_workspace(preserve) as temporary:
        project = temporary / 'app'
        report_path = args.report or (temporary / 'performance.json' if preserve else ROOT / 'target/go-cinema/performance.json')
        if preserve and report_path.exists():
            raise RuntimeError('Refusing to overwrite retained Go performance report')
        report['native_workspace'] = {'path': str(temporary), 'retained': preserve, 'filesystem': filesystem_type(temporary), 'tmpdir': str(temporary), 'supervision': 'linux-owner-subreaper-v1', 'comparability': 'native costs unassessed when fixture filesystem or cache conditions differ'}
        if preserve:
            report_path.parent.mkdir(parents=True, exist_ok=True)
            atomic_json(report_path, report)
            print(f'Retained Go native workspace: {temporary}; incomplete report: {report_path}', flush=True)
        try:
            copy_application(project)
            if preserve:
                shutil.copyfile(project / '.dctx0/testing/go/inspection-notes.json', temporary / 'inspection-notes-original.json')
            prefix = [binary, '--json']
            def run(label, *command):
                if command[:2] in (('verify','run'),('verify','replay')):
                    command = (*command, '--full')
                data, sample = measure([*prefix, *command])
                report['samples'][label] = sample
                if preserve:
                    atomic_json(report_path, report)
                return data
            run('check', 'check', project / 'dctx0')
            run('discovery', 'verify', 'discover', project / 'dctx0', '--project', project, '--target', 'go')
            run('planning', 'verify', 'plan', project / 'dctx0', '--project', project)
            run('harness-setup', 'verify', 'init', project / 'dctx0', '--project', project, '--target', 'go')
            for cache in ('first-project-build', 'warm-build'):
                native = run(cache, 'verify', 'run', '--reuse', 'off', project / 'dctx0', '--project', project, '--no-adopt')
                result = native['evidence']['run']
                assert_native_cinema(result)
                details = artifacts(project, native['evidence']['id'])
                receipt = details['application/vnd.dctx0.go-build-inputs+json']
                build = details['application/vnd.dctx0.build-audit+json']
                report['samples'][cache]['managed_build_elapsed_ms'] = build['elapsed_us'] / 1000
                report['samples'][cache]['go_compile_elapsed_ms'] = receipt['build_elapsed_us'] / 1000
                report['samples'][cache]['go_compile_peak_rss_bytes'] = receipt['build_peak_rss_bytes']
                report['samples'][cache]['runtime_metrics'] = details['application/vnd.dctx0.runtime-metrics+json']
                report['samples'][cache]['native_engine_elapsed_ms'] = result['elapsed_us'] / 1000
                report['samples'][cache]['go_version'] = receipt['go_version']
                report['samples'][cache]['worker_hash'] = receipt['binary_hash']
            review_sample, review_log = measured([sys.executable, str(ROOT / 'scripts/conformance_review_acceptance.py'), '--binary', str(binary), '--target', 'go', '--project', str(project), '--report', str(project.parent / 'reviews.json')], ROOT, timeout=900, env={**os.environ, 'DCTX0_MEMORY_GUARD':'0'})
            if preserve:
                (temporary / 'conformance-review.log').write_text(review_log)
            if review_sample['exit_code']:
                raise RuntimeError(review_log)
            report['samples']['source-bound-review-and-delivery'] = review_sample
            completed = run('complete-and-adoption', 'verify', 'complete', '--reuse', 'off', project / 'dctx0', '--project', project)
            if completed['result']['status'] != 'PASS' or completed['open_tasks'] or completed['non_pass_tasks'] != 4 or len(completed['reviewed_tasks']) != 4:
                raise RuntimeError('Measured complete acceptance failed')
            report['samples']['complete-and-adoption']['native_adopted_tasks'] = len(completed['adopted_tasks'])
            report['samples']['complete-and-adoption']['inspected_tasks'] = len(completed['inspected_tasks'])
            implementation = project / 'persistence/store.go'
            original = implementation.read_text()
            assert original.count('poll["status"] = "Closed"') == 1
            implementation.write_text(original.replace('poll["status"] = "Closed"', 'poll["status"] = "Open"'))
            broken = run('counterexample-and-shrink', 'verify', 'run', '--reuse', 'off', project / 'dctx0', '--project', project,
                         '--subject', 'HomeCinema.MovieNight.ScreeningPollLifecycle', '--no-adopt')
            failures = [case for case in broken['evidence']['run']['cases'] if case['result']['status'] == 'FAIL']
            if not failures:
                raise RuntimeError(f'Measured mutation has no actual counterexample: {broken["evidence"]["run"]["result"]}; {broken["evidence"]["run"]["provenance"]["incomplete"]}')
            details = artifacts(project, broken['evidence']['id'])
            shrinks = details.get('shrinks', [])
            if not shrinks:
                raise RuntimeError('No actual central shrink evidence')
            report['samples']['counterexample-and-shrink']['central_shrink_elapsed_ms'] = sum(item['elapsed_us'] for item in shrinks) / 1000
            report['samples']['counterexample-and-shrink']['shrink_attempts'] = sum(len(item['report']['attempts']) for item in shrinks)
            report['samples']['counterexample-and-shrink']['expected_result'] = 'FAIL from intentional production mutation'
            replay = run('broken-replay', 'verify', 'replay', project / 'dctx0', broken['evidence']['id'],
                         '--case', failures[0]['id'], '--project', project)
            if replay['case_result']['status'] != 'FAIL':
                raise RuntimeError('Measured replay did not reproduce failure')
            report['samples']['broken-replay']['expected_result'] = 'FAIL from intentional production mutation'
            implementation.write_text(original)
            repaired = run('repaired-replay', 'verify', 'replay', project / 'dctx0', broken['evidence']['id'],
                           '--case', failures[0]['id'], '--project', project)
            if repaired['case_result']['status'] != 'PASS':
                raise RuntimeError('Measured repaired replay failed')

        finally:
            if preserve:
                atomic_json(report_path, report)

    report['status'] = 'complete'
    report_path.parent.mkdir(parents=True, exist_ok=True)
    atomic_json(report_path, report)
    print(f'Go release phases complete; {report_path}')


if __name__ == '__main__':
    main()
