#!/usr/bin/env python3
"""Couple status/preflight regressions with isolated release CLI speed/RSS samples."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import sys
import tempfile
from release_support import locked, measured
from cinema_expectations import EXPECTED_NATIVE_CASES
import validate

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--skip-tests', action='store_true', help='workspace caller supplies Rust regressions')
    parser.add_argument('--samples', type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.samples <= 20:
        parser.error('--samples must be 1..20')
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    output = ROOT / 'target/status-acceptance' / stamp
    output.mkdir(parents=True)
    before = validate.digest(validate.validation_sources())
    report = {'schema_version': 1, 'timestamp': stamp, 'status': 'incomplete',
              'scope': 'release status/preflight only' if args.skip_tests else 'status/preflight regressions and release commands',
              'source_hash_before': before, 'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': bool(validate.capture(['git', 'status', '--porcelain'])),
              'platform': platform.platform(), 'hardware': {'machine': platform.machine(), 'cpu_count': os.cpu_count()},
              'toolchains': {name: validate.capture(cmd) for name, cmd in [('rust', ['rustc', '-vV']), ('bun', ['bun', '--version'])]},
              'settings': {'samples': args.samples, 'warmup_runs': 1, 'cache': 'warm OS/tool caches; fresh isolated history', 'seed': 42, 'profile': 'release CLI; tests measured separately'},
              'baseline': {'status': 'unassessed', 'reason': 'new workloads; no automatic promotion'},
              'commands': [], 'measurements': {}}

    def run(label, command, expected=0):
        print(label, flush=True)
        sample, text = measured(list(map(str, command)), ROOT, timeout=1800, env=os.environ.copy())
        (output / (label + '.log')).write_text(text)
        report['commands'].append({'label': label, 'command': list(map(str, command)), 'expected_exit': expected, 'stdout_bytes': len(text.encode()), **sample})
        if (sample['exit_code'] == 0) != (expected == 0):
            raise RuntimeError(f'{label} returned {sample["exit_code"]}; see {output}')
        return sample, text

    try:
        if not args.skip_tests:
            run('engine-regressions', ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'verification_preflight', '--locked'])
            run('cli-regressions', ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'status', '--test', 'verification_run', '--test', 'verification_lifecycle', '--test', 'verification_complete', '--all-features', '--locked'])
            run('cli-without-targets', ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'status', '--no-default-features', '--locked'])
        if not args.skip_build:
            run('release-build', ['cargo', 'build', '-p', 'dctx0-cli', '--release', '--all-features', '--locked'])
        binary = args.binary.resolve()
        report['binary_hash'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        for example, target in [('cinema', 'typescript'), ('cinema-go', 'go')]:
            app = output / example
            spec = app / 'dctx0'
            shutil.copytree(ROOT / 'examples' / example / 'dctx0', spec, ignore=shutil.ignore_patterns('.dctx0ctl'))
            run(example + '-check', [binary, 'check', spec, '--json'])
            run(example + '-profiles', [binary, 'profile', target, 'create', '--project', app, '--default', '--json'])
            run(example + '-tasks', [binary, 'tasks', 'plan', spec, '--json'])
            run(example + '-init', [binary, 'verify', 'init', spec, '--project', app, '--target', target, '--subject', 'HomeCinema.MovieNight.ReleaseYear', '--json'])
            # Reuse the reviewed native inputs unchanged. The whole-plan probe
            # must distinguish runtime state requirements from absent observers.
            testing = app / '.dctx0/testing' / target
            for name in ('examples.json', 'lifecycles.json'):
                shutil.copyfile(ROOT / 'examples' / example / '.dctx0/testing' / target / name, testing / name)
            database = spec / '.dctx0ctl/state.sqlite3'
            saved = database.read_bytes()
            for phase, command, expected in [
                ('status', [binary, 'status', spec, '--project', app, '--json'], 0),
                ('preflight', [binary, 'verify', 'preflight', spec, '--project', app, '--json'], 1),
                ('diagnostic-preflight', [binary, 'verify', 'preflight', spec, '--project', app, '--diagnostic', '--json'], 0),
                ('whole-plan-preflight', [binary, 'verify', 'preflight', spec, '--project', app, '--subject', 'HomeCinema.MovieNight', '--json'], 1),
            ]:
                run(example + '-' + phase + '-warmup', command, expected)
                samples = []
                for number in range(args.samples):
                    sample, text = run(f'{example}-{phase}-{number}', command, expected)
                    value = json.loads(text)
                    assert len(text.encode()) <= 131_072
                    if phase in ('preflight', 'whole-plan-preflight'):
                        assert not value['executed'] and not value['permits_execution']
                        assert value['blockers']['PREFLIGHT_BINDINGS_UNAVAILABLE'] == 1
                        assert value['potential_task_count'] == 0
                        if phase == 'whole-plan-preflight':
                            assert value['required_cases'] == EXPECTED_NATIVE_CASES
                            assert 'STATE_OBSERVER_REQUIRED' not in value['blockers']
                            assert value['warnings']['STATE_OBSERVER_REQUIRED'] > 0
                            assert set(value['blockers']) == {'PREFLIGHT_BINDINGS_UNAVAILABLE'}
                    elif phase == 'status':
                        assert value['latest_run'] is None
                        assert len(value['next_causes']) <= 3
                    else:
                        assert not value['executed'] and value['permits_execution']
                        assert value['intent'] == 'diagnostic'
                    samples.append(sample)
                report['measurements'][example + '/' + phase] = {
                    'samples': samples, 'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                    'peak_rss_bytes': max((s['peak_rss_bytes'] for s in samples if s['peak_rss_bytes'] is not None), default=None),
                    'scope': 'unchanged complete Cinema specs; unavailable adapter; no application execution'}
            assert saved == database.read_bytes()
        report['source_hash_after'] = validate.digest(validate.validation_sources())
        if before != report['source_hash_after']:
            raise RuntimeError('Validation inputs changed during the run')
        report['status'] = 'complete'
    except Exception as error:
        report['error'] = str(error)
    finally:
        history = ROOT / 'benchmarks/history/status'
        history.mkdir(parents=True, exist_ok=True)
        with locked(history / '.lock'):
            with tempfile.NamedTemporaryFile(mode='w', dir=history, delete=False) as staged:
                json.dump(report, staged, indent=2)
                staged.write('\n'); staged.flush(); os.fsync(staged.fileno())
            destination = history / f'{stamp}-{os.getpid()}.json'
            os.replace(staged.name, destination)
            for old in sorted(history.glob('*.json'))[:-20]:
                old.unlink()
        print(json.dumps({'status': report['status'], 'report': str(destination), 'error': report.get('error')}))
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    sys.exit(main())
