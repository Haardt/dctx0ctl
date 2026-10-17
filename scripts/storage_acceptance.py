#!/usr/bin/env python3
"""Couple storage tests with release speed/RSS, including a 240 MB synthetic run."""
import argparse
import datetime
import fcntl
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import validate


def run(command, log):
    started = time.perf_counter()
    with log.open('wb') as output:
        process = subprocess.Popen(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
        _, status, usage = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
    return {'command': command, 'exit_code': process.returncode,
            'elapsed_ms': (time.perf_counter() - started) * 1000,
            'peak_rss_bytes': usage.ru_maxrss * 1024,
            'memory_scope': 'Linux wait4 RUSAGE_CHILDREN high-water RSS; command including compiler/test children',
            'log': str(log.relative_to(ROOT))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scope', choices=('store', 'engine', 'cli', 'focused', 'all'), default='all')
    parser.add_argument('--samples', type=int, default=3)
    args = parser.parse_args()
    os.environ.setdefault("DCTX0_TEST_PYTHON", sys.executable)
    if sys.platform != 'linux' or args.samples < 1:
        parser.error('This coupled measurement runner requires Linux and positive samples')
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    folder = ROOT / 'target' / 'storage-acceptance' / stamp
    folder.mkdir(parents=True)
    before = validate.digest(validate.validation_sources())
    report = {'schema_version': 1, 'timestamp': stamp, 'status': 'incomplete',
              'source_hash_before': before, 'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': validate.capture(['git', 'status', '--porcelain']),
              'toolchain': validate.capture(['rustc', '--version']),
              'platform': platform.platform(), 'hardware': {'architecture': validate.capture(['uname', '-m']), 'cpu_count': os.cpu_count(), 'cpu': validate.capture(['lscpu'])},
              'scope': args.scope, 'checks': [], 'measurements': {},
              'baseline': 'unassessed: new storage workloads; existing baselines unchanged',
              'settings': {'samples': args.samples, 'iterations': 1, 'warmup': 1,
                           'profile': 'release, no coverage', 'cache': 'warm OS cache; fresh SQLite per process',
                           'synthetic_cases': 10000, 'synthetic_value_bytes_per_case': 24000}}
    commands = []
    if args.scope == 'all':
        commands.extend([['bun', 'run', '--cwd', 'ui/site', 'check'], ['bun', 'test', 'ui/site/src/model.test.ts'], ['bun', 'run', '--cwd', 'ui/site', 'verify-assets']])
    if args.scope in ('store', 'focused', 'all'):
        commands.append(['cargo', 'test', '-p', 'dctx0-store', '--locked'])
    if args.scope == 'focused':
        commands.append(['cargo', 'test', '-p', 'dctx0-engine', '--lib', '--test', 'verification_run', 'storage_', '--locked'])
    if args.scope in ('engine', 'all'):
        commands.append(['cargo', 'test', '-p', 'dctx0-engine', '--lib', '--test', 'site', '--test', 'verification_run', '--locked'])
    if args.scope in ('cli', 'focused', 'all'):
        commands.append(['cargo', 'test', '-p', 'dctx0-cli', '--test', 'verification_history', '--all-features', '--locked'])
    commands.append(['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'])
    try:
        for index, command in enumerate(commands):
            print('Running:', ' '.join(command), flush=True)
            result = run(command, folder / f'check-{index}.log')
            report['checks'].append(result)
            if result['exit_code']:
                # Performance still runs after a failed test when a release binary can be built.
                print('Failed; see', result['log'], flush=True)
        binary = ROOT / 'target/release/dctx0'
        if report['checks'][-1]['exit_code']:
            raise RuntimeError('Release build failed; speed/RSS measurements are incomplete')
        for name in ('cinema', 'cinema-go'):
            path = f'examples/{name}/dctx0'
            for phase in ('compile', 'verification-generate', 'verification-assess-unbound',
                          'verification-history-large-publish', 'verification-history-large-read',
                          'verification-history-summary'):
                print('Measuring:', name, phase, flush=True)
                validate.measure(binary, path, 1, phase)
                samples = [validate.measure(binary, path, 1, phase) for _ in range(args.samples)]
                report['measurements'][f'{name}/{phase}'] = {'samples': samples,
                    'memory_scope': 'Linux post-exec VmHWM; includes Rust core preparation, synthetic run and SQLite; no native compiler/harness',
                    'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                    'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples), 'workload_version': 1}
        after = validate.digest(validate.validation_sources())
        report['source_hash_after'] = after
        report['status'] = 'complete' if before == after and all(c['exit_code'] == 0 for c in report['checks']) else 'failed'
    except Exception as error:
        report['error'] = str(error)
        print(error, file=sys.stderr)
    finally:
        history = ROOT / 'benchmarks/history/storage'
        history.mkdir(parents=True, exist_ok=True)
        with (history / '.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            target = history / f'{stamp}-{os.getpid()}.json'
            with tempfile.NamedTemporaryFile('w', dir=history, delete=False) as output:
                json.dump(report, output, indent=2)
                output.write('\n')
                output.flush()
                os.fsync(output.fileno())
            os.replace(output.name, target)
            for old in sorted(history.glob('*.json'))[:-20]:
                old.unlink()
        print('Evidence:', target.relative_to(ROOT), flush=True)
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    raise SystemExit(main())
