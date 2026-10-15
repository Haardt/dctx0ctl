#!/usr/bin/env python3
"""Native verification acceptance coupled to uninstrumented release time/RSS.

Run on each host; this does not simulate another OS. Never promotes a baseline.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

from release_support import measured, locked
from release import native_target

ROOT = Path(__file__).resolve().parents[1]
HISTORY = ROOT / 'benchmarks/history/native-portability'


def content_hash():
    digest = hashlib.sha256()
    paths = subprocess.check_output(['git', 'ls-files', '-z', 'Cargo.toml', 'Cargo.lock', 'crates', 'targets', 'scripts', 'fixtures', 'examples/cinema', 'examples/cinema-go', 'examples/cinema-rust', '.github/workflows/release.yml', '.gitattributes'], cwd=ROOT).decode().split('\0')
    # The contributor runner may itself be new in a dirty checkout.
    paths += ['scripts/native_portability.py', 'crates/dctx0-engine/tests/native_portable.rs', 'crates/dctx0-engine/tests/support/native_process.rs']
    for name in sorted(set(paths) - {''}):
        path = ROOT / name
        digest.update(name.encode()); digest.update(path.read_bytes())
    return digest.hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release' / ('dctx0.exe' if os.name == 'nt' else 'dctx0'))
    parser.add_argument('--coverage', action='store_true', help='retain native transport/target branch coverage separately from Linux workspace coverage')
    parser.add_argument('--transport-only', action='store_true', help='partial development run, with native build/execution measurements')
    args = parser.parse_args()
    now = datetime.now(timezone.utc)
    stamp = now.strftime('%Y%m%dT%H%M%S.%fZ') + f'-{os.getpid()}'
    output = ROOT / 'target/native-portability' / stamp
    output.mkdir(parents=True)
    record = {'schema_version': 1, 'id': stamp, 'timestamp': now.isoformat(), 'status': 'incomplete',
              'scope': 'transport development' if args.transport_only else 'native three-target verification and Cinema acceptance',
              'metadata': {'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                           'dirty_worktree': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True)),
                           'target': native_target(), 'platform': platform.platform(), 'machine': platform.machine(), 'cpu': platform.processor() or 'unknown', 'logical_cpus': os.cpu_count(),
                           'content_hash_before': content_hash(), 'python': sys.version, 'binary_hash': None, 'toolchains': {}},
              'baseline': {'status': 'unassessed', 'reason': 'No accepted native portability baseline; existing baselines unchanged'},
              'cache': 'prepared offline dependencies; prebuilt release CLI; existing compiler caches; one cold project and one warm execution',
              'seed': 42, 'generated_cases': 100, 'features': 'all-features', 'checks': [], 'measurements': {}}
    environment = {**os.environ, 'DCTX0_TEST_PYTHON': sys.executable, 'DCTX0_MEMORY_GUARD': '0', 'DCTX0_COMPLETE_MEMORY_GUARDED': '1'}

    def run(label, command, timeout=1800, allowed=(0,)):
        print(f'Native acceptance: {label}', flush=True)
        sample, text = measured(list(map(str, command)), ROOT, timeout=timeout, env=environment)
        record['measurements'][label] = sample
        (output / f'{label}.log').write_text(text, encoding='utf-8')
        record['checks'].append({'label': label, 'exit_code': sample['exit_code'], 'expected_exit_codes': allowed})
        print(f'Native acceptance: {label}: exit {sample["exit_code"]}, {sample["elapsed_ms"]:.1f} ms, peak RSS {sample["peak_rss_bytes"]}', flush=True)
        if sample['exit_code'] not in allowed:
            raise RuntimeError(f'{label}: exit {sample["exit_code"]}; {text[-5000:]}')
        return text

    failure = None
    try:
        for name, command in [('bun', ['bun', '--version']), ('go', ['go', 'version']), ('rust', ['rustc', '-vV'])]:
            if not shutil.which(command[0]):
                raise RuntimeError(f'Required toolchain unavailable: {name}')
            record['metadata']['toolchains'][name] = run('toolchain-' + name, command).strip()
        binary = args.binary.resolve()
        record['metadata']['binary_hash'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        run('portable-transport', ['cargo', 'test', '--locked', '-p', 'dctx0-engine', '--test', 'native_portable', '--test', 'native'])
        run('small-compiled-targets', ['cargo', 'test', '--locked', '-p', 'dctx0-engine', '--test', 'verification_go', '--test', 'verification_rust'])
        run('small-typescript-and-replay', ['bun', ROOT / 'scripts/release_platform.ts', binary])
        run('complete-guards-and-reviews', ['cargo', 'test', '--locked', '--all-features', '-p', 'dctx0-cli', '--test', 'verification_complete'])
        if not args.transport_only:
            run('cinema-reviews', [sys.executable, ROOT / 'scripts/conformance_review_acceptance.py', '--binary', binary, '--report', output / 'cinema-reviews.json'])
            with tempfile.TemporaryDirectory(prefix='cinema-ts-', dir=output) as scratch:
                app = Path(scratch) / 'application with spaces'
                shutil.copytree(ROOT / 'examples/cinema', app, ignore=shutil.ignore_patterns('.dctx0ctl', '.bin', 'cinema.sqlite3*'))
                spec = app / 'dctx0'
                run('cinema-ts-check', [binary, 'check', spec, '--json'])
                source = app / 'src/domain/candidate-order-winner.ts'
                original = source.read_bytes()
                assert original.count(b'if (count > highest)') == 1
                source.write_bytes(original.replace(b'if (count > highest)', b'if (count >= highest)'))
                broken = json.loads(run('cinema-ts-mutation', [binary, 'verify', 'run', spec, '--project', app, '--subject', 'HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner', '--no-adopt', '--json'], allowed=(1,)))
                case = next(case for case in broken['evidence']['run']['cases'] if case['result']['status'] == 'FAIL')
                replay = [binary, 'verify', 'replay', spec, broken['evidence']['id'], '--case', case['id'], '--project', app, '--json']
                assert json.loads(run('cinema-ts-broken-replay', replay, allowed=(1,)))['case_result']['status'] == 'FAIL'
                source.write_bytes(original)
                repaired = json.loads(run('cinema-ts-repaired-replay', replay))
                assert repaired['case_result']['status'] == 'PASS' and repaired['changes']
                assert not repaired['evidence']['adopted_tasks']
            for target in ('go', 'rust'):
                run('cinema-' + target, [sys.executable, ROOT / f'scripts/{target}_cinema_acceptance.py', '--binary', binary, '--report', output / f'cinema-{target}.json'])
        if args.coverage:
            command = ['cargo', 'llvm-cov', '--locked', '--no-report', '-p', 'dctx0-engine', '--test', 'native_portable', '--test', 'verification_go', '--test', 'verification_rust']
            run('native-coverage', command)
            run('native-coverage-export', ['cargo', 'llvm-cov', 'report', '--lcov', '--output-path', output / 'native.lcov'])
            run('native-coverage-raw', ['cargo', 'llvm-cov', 'report', '--json', '--output-path', output / 'llvm.json'])
            record['coverage_scope'] = 'Native executor and compiled-target tests on this platform; no extrapolation from Linux and no replacement of the 97% Linux workspace gate'
        record['status'] = 'partial' if args.transport_only else 'complete'
    except (OSError, ValueError, RuntimeError, TimeoutError, AssertionError, StopIteration, KeyError) as error:
        failure = str(error)
        record.update(status='failed', failure=failure)
    finally:
        # Release build/execution measurements are mandatory, including after a
        # failed functional check. A failure cannot produce a usable baseline.
        try:
            with tempfile.TemporaryDirectory(prefix='measurement-', dir=output) as scratch:
                app = Path(scratch) / 'application with spaces'
                shutil.copytree(ROOT / 'fixtures/conformance/adaptive-typescript/direct', app, ignore=shutil.ignore_patterns('node_modules', '.bin'))
                spec = app / 'spec'
                spec.mkdir()
                shutil.copy(ROOT / 'fixtures/conformance/adaptive-typescript/spec/workspace.dctx0', spec / 'w.dctx0')
                run('release-check', [binary, 'check', spec, '--json'])
                run('release-generation', [binary, 'verify', 'init', spec, '--project', app, '--target', 'typescript', '--subject', 'Adaptive.Counter.next', '--json'])
                for cache in ('first-project', 'warm-project'):
                    run('release-execution-' + cache, [binary, 'verify', 'run', spec, '--project', app, '--no-adopt', '--json'])
        except (OSError, ValueError, RuntimeError, TimeoutError, UnboundLocalError) as error:
            failure = (failure + '\n' if failure else '') + str(error)
            record.update(status='failed', failure=failure)
        record['metadata']['content_hash_after'] = content_hash()
        if record['metadata']['content_hash_before'] != record['metadata']['content_hash_after']:
            failure = (failure + '\n' if failure else '') + 'Source inputs changed during acceptance'
            record.update(status='failed', failure=failure)
        HISTORY.mkdir(parents=True, exist_ok=True)
        with locked(HISTORY.parent / '.native-portability.lock'):
            with tempfile.NamedTemporaryFile(mode='w', dir=HISTORY, delete=False, prefix='.write-', encoding='utf-8') as staged:
                json.dump(record, staged, indent=2); staged.write('\n'); staged.flush(); os.fsync(staged.fileno())
            os.replace(staged.name, HISTORY / (stamp + '.json'))
            for old in sorted(HISTORY.glob('*.json'))[:-20]:
                old.unlink()
        (output / 'report.json').write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'status': record['status'], 'report': str(output / 'report.json'), 'failure': failure}, indent=2))
    return 1 if failure else 0


if __name__ == '__main__':
    raise SystemExit(main())
