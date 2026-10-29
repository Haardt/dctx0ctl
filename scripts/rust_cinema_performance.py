#!/usr/bin/env python3
"""Final-stage release costs for Rust Cinema; never promotes a benchmark baseline."""
from verification_storage import saved_verification_artifacts
from cinema_expectations import assert_native_cinema
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import sys
from release_support import measured

from rust_cinema_acceptance import ROOT, SOURCE, copy_application


def measure(command):
    started = time.perf_counter()
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(list(map(str, command)), cwd=ROOT, env={**os.environ, 'DCTX0_MEMORY_GUARD': '0'}, stdout=stdout, stderr=stderr)
        _, status, usage = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
        stdout.seek(0)
        stderr.seek(0)
        text, errors = stdout.read().decode(), stderr.read().decode()
    if process.returncode not in (0, 1):
        raise RuntimeError(errors)
    return json.loads(text), {'elapsed_ms': (time.perf_counter() - started) * 1000, 'peak_rss_bytes': int(usage.ru_maxrss) * 1024, 'exit_code': process.returncode}


def artifacts(project, run_id):
    rows = saved_verification_artifacts(project / "dctx0/.dctx0ctl/state.sqlite3", run_id)
    result = {}
    for media, raw, expected in rows:
        if hashlib.sha256(raw).hexdigest() != expected:
            raise RuntimeError('Measured artifact hash mismatch')
        if media in ('application/vnd.dctx0.shrink+json', 'application/vnd.dctx0.lifecycle-shrink+json'):
            result.setdefault('shrinks', []).append(json.loads(raw))
        if media in ('application/vnd.dctx0.rust-build-inputs+json', 'application/vnd.dctx0.build-audit+json', 'application/vnd.dctx0.runtime-metrics+json'):
            result[media] = json.loads(raw)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/rust-cinema/performance.json')
    args = parser.parse_args()
    if not Path('/proc/self/status').exists():
        raise RuntimeError('Rust release RSS measurement is accepted only on Linux')
    binary = args.binary.resolve()
    report = {'schema_version': 1, 'status': 'incomplete', 'binary_hash': hashlib.sha256(binary.read_bytes()).hexdigest(), 'seed': 42, 'generated_cases': 100, 'samples': {}, 'baseline': 'unassessed; never promoted automatically', 'cache': 'first project build and repeated build using the owned shared Cargo cache; existing module/toolchain caches', 'memory_scope': 'wait4 ru_maxrss: largest reaped CLI/descendant process, not collective simultaneous RSS; worker reports its own VmHWM separately'}
    with tempfile.TemporaryDirectory(prefix='dctx0-rust-release-') as temporary:
        project = Path(temporary) / 'app'
        copy_application(project)
        prefix = [binary, '--json']
        def run(label, *command):
            if command[:2] in (('verify','run'),('verify','replay')):
                command = (*command, '--full')
            data, sample = measure([*prefix, *command])
            report['samples'][label] = sample
            return data
        run('check', 'check', project / 'dctx0')
        run('discovery', 'verify', 'discover', project / 'dctx0', '--project', project, '--target', 'rust')
        run('planning', 'verify', 'plan', project / 'dctx0', '--project', project)
        run('harness-setup', 'verify', 'init', project / 'dctx0', '--project', project, '--target', 'rust')
        for cache in ('first-project-build', 'warm-build'):
            native = run(cache, 'verify', 'run', project / 'dctx0', '--project', project, '--no-adopt')
            result = native['evidence']['run']
            assert_native_cinema(result)
            details = artifacts(project, native['evidence']['id'])
            receipt = details['application/vnd.dctx0.rust-build-inputs+json']
            build = details['application/vnd.dctx0.build-audit+json']
            report['samples'][cache]['managed_build_elapsed_ms'] = build['elapsed_us'] / 1000
            report['samples'][cache]['runtime_metrics'] = details['application/vnd.dctx0.runtime-metrics+json']
            report['samples'][cache]['native_engine_elapsed_ms'] = result['elapsed_us'] / 1000
            report['samples'][cache]['rust_version'] = receipt['runtime_version']
            report['samples'][cache]['worker_hash'] = receipt['binary_hash']
        review_sample, review_log = measured([sys.executable, str(ROOT / 'scripts/conformance_review_acceptance.py'), '--binary', str(binary), '--target', 'rust', '--project', str(project), '--report', str(project.parent / 'reviews.json')], ROOT, timeout=900, env={**os.environ, 'DCTX0_MEMORY_GUARD':'0'})
        if review_sample['exit_code']:
            raise RuntimeError(review_log)
        report['samples']['source-bound-review-and-delivery'] = review_sample
        completed = run('complete-and-adoption', 'verify', 'complete', project / 'dctx0', '--project', project)
        if completed['result']['status'] != 'PASS' or completed['open_tasks'] or completed['non_pass_tasks'] != 4 or len(completed['reviewed_tasks']) != 4:
            raise RuntimeError('Measured complete acceptance failed')
        report['samples']['complete-and-adoption']['native_adopted_tasks'] = len(completed['adopted_tasks'])
        report['samples']['complete-and-adoption']['inspected_tasks'] = len(completed['inspected_tasks'])
        implementation = project / 'src/persistence.rs'
        original = implementation.read_text()
        assert original.count('poll["status"] = json!("Closed");') == 1
        implementation.write_text(original.replace('poll["status"] = json!("Closed");', 'poll["status"] = json!("Open");'))
        broken = run('counterexample-and-shrink', 'verify', 'run', project / 'dctx0', '--project', project,
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

    report['status'] = 'complete'
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Rust release phases complete; {args.report}')


if __name__ == '__main__':
    main()
