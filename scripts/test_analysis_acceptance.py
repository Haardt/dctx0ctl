#!/usr/bin/env python3
"""Couple research checks with six release speed/RSS workloads; never promote a baseline."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import shutil
import subprocess
import sys
import tempfile
import time

import validate

ROOT = validate.ROOT
HISTORY = ROOT / 'benchmarks/history/test-analysis'


def measured(argv, cwd=ROOT):
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        started = time.perf_counter()
        child = subprocess.Popen(list(map(str, argv)), cwd=cwd, stdout=stdout, stderr=stderr)
        _, status, usage = os.wait4(child.pid, 0)
        child.returncode = os.waitstatus_to_exitcode(status)
        elapsed = (time.perf_counter() - started) * 1000
        stdout.seek(0); stderr.seek(0)
        out, err = stdout.read().decode(), stderr.read().decode()
    return {'elapsed_ms': elapsed, 'peak_rss_bytes': usage.ru_maxrss * 1024,
            'returncode': child.returncode}, out, err


def checked(argv, cwd=ROOT):
    sample, out, err = measured(argv, cwd)
    if sample['returncode']:
        raise RuntimeError(f'{argv}: {err[-8000:]} {out[-8000:]}')
    return out


def fixture(root):
    (root / 'src').mkdir(parents=True)
    (root / 'Cargo.toml').write_text('[package]\nname="research-measurement"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
    (root / '.gitignore').write_text('/.dctx0ctl/\n/target/\n')
    source = 'pub fn one()->u32 {1}\npub fn two()->u32 {2}\n#[test] fn first(){assert_eq!(one(),1);}\n#[test] fn second(){assert_eq!(two(),2);}\n'
    (root / 'src/lib.rs').write_text(source)
    checked(['cargo', 'generate-lockfile', '--offline'], root)
    def git(*args):
        return checked(['git', '-c', 'user.name=Research measurement', '-c', 'user.email=research@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', *args], root).strip()
    git('init', '-q'); git('add', '.'); git('commit', '-qm', 'base')
    base = git('rev-parse', 'HEAD')
    (root / 'src/lib.rs').write_text(source.replace('one()->u32 {1}', 'one()->u32 {9}'))
    git('add', '.'); git('commit', '-qm', 'observed failure')
    head = git('rev-parse', 'HEAD')
    state = root / '.dctx0ctl'
    state.mkdir()
    manifest = state / 'experiment.json'
    manifest.write_text(json.dumps({'schema_version': 1, 'scope': {'package': None, 'filter': None, 'all_features': False, 'no_default_features': False, 'features': []}, 'timeout_secs': 60, 'strategy': 'coverage', 'pairs': [{'base': base, 'head': head}]}))
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--measure-only', action='store_true')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    args = parser.parse_args()
    if args.samples < 1:
        parser.error('Samples must be positive')
    os.chdir(ROOT)
    os.environ.setdefault('CARGO_PROFILE_DEV_DEBUG', '0')
    os.environ.setdefault('CARGO_PROFILE_TEST_DEBUG', '0')
    os.environ.setdefault('CARGO_INCREMENTAL', '0')
    os.environ.pop('LLVM_PROFILE_FILE', None)
    os.environ.pop('CARGO_ENCODED_RUSTFLAGS', None)
    os.environ.pop('RUSTFLAGS', None)
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    report = {'schema_version': 1, 'timestamp': stamp, 'status': 'incomplete',
              'source_hash': validate.digest(validate.validation_sources()),
              'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': bool(validate.capture(['git', 'status', '--porcelain'])),
              'platform': platform.platform(), 'hardware': {'machine': platform.machine(), 'cpu_count': os.cpu_count(), 'cpu': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown')},
              'python': sys.version,
              'toolchain': validate.capture(['rustc', '-vV']), 'measurements': {}, 'checks': [],
              'settings': {'samples': args.samples, 'warmup_runs': 1, 'profile': 'release', 'cache': 'warm filesystem; fresh Cargo target directory for catalog/collection/replay', 'fixture': 'two libtest tests; two commits; one observed production failure'},
              'memory_scope': 'Linux wait4 ru_maxrss: maximum child/its waited descendants, includes pre-exec parent footprint; concurrent RSS is not summed; build/test RSS additionally present in exported collection/observation data',
              'baseline_status': 'unassessed here: scripts/validate.py applies the accepted baseline and independent speed/RAM gates',
              'scope': 'partial research validation; not workspace coverage acceptance'}
    logdir = ROOT / 'target/test-analysis-validation' / stamp
    logdir.mkdir(parents=True)
    try:
        commands = [] if args.measure_only else [
            ['cargo', 'fmt', '--all', '--', '--check'],
            ['cargo', 'clippy', '-p', 'dctx0-test-analysis', '-p', 'dctx0-store', '-p', 'dctx0-engine', '-p', 'dctx0-cli', '--all-targets', '--all-features', '--locked', '--offline', '--', '-D', 'warnings'],
            ['cargo', 'test', '-p', 'dctx0-test-analysis', '--locked', '--offline'],
            ['cargo', 'test', '-p', 'dctx0-store', '--test', 'test_analysis', '--locked', '--offline'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'test_analysis', '--locked', '--offline'],
            ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'test_analysis', '--test', 'experimental_coverage', '--locked', '--offline'],
            [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests'],
        ]
        commands.append(['cargo', 'build', '--release', '-p', 'dctx0-cli', '--locked', '--offline'])
        for index, command in enumerate(commands):
            print('+ ' + ' '.join(command), flush=True)
            sample, out, err = measured(command)
            (logdir / f'{index}.stdout').write_text(out); (logdir / f'{index}.stderr').write_text(err)
            report['checks'].append({'command': command, **sample})
        if any(c['returncode'] for c in report['checks']):
            raise RuntimeError('Functional checks or release build failed; see raw logs')
        with tempfile.TemporaryDirectory(prefix='dctx0-research-measure-') as directory:
            directory = Path(directory)
            setup = directory / 'setup'
            manifest = fixture(setup)
            prefix = [args.binary, 'experimental']
            replay = json.loads(checked([*prefix, 'test-analysis', 'replay', setup, '--manifest', manifest]))
            dataset = directory / 'dataset.ndjson'
            checked([*prefix, 'test-analysis', 'export', setup, replay['id'], '--output', dataset])
            for phase in ('catalog', 'collection', 'prediction', 'evaluation', 'replay', 'export'):
                samples = []
                for index in range(args.samples + 1):
                    if phase in ('catalog', 'collection', 'replay'):
                        root = directory / f'{phase}-{index}'
                        manifest = fixture(root)
                        command = [*prefix, 'test-analysis', 'replay', root, '--manifest', manifest] if phase == 'replay' else [*prefix, 'coverage', 'catalog' if phase == 'catalog' else 'collect', root]
                    elif phase in ('prediction', 'evaluation'):
                        command = [*prefix, 'test-analysis', 'recompute', dataset, '--stage', phase]
                    else:
                        command = [*prefix, 'test-analysis', 'export', setup, replay['id'], '--output', directory / f'export-{index}.ndjson']
                    sample, out, err = measured(command)
                    # A fixture collection intentionally contains one ordinary FAIL.
                    expected_failure = phase == 'collection' and json.loads(out).get('completeness', {}).get('reference_evaluable') is True
                    if sample['returncode'] and not expected_failure:
                        raise RuntimeError(f'{phase}: {err} {out}')
                    if phase in ('catalog', 'collection', 'replay'):
                        builds, tests = [], []
                        for raw_manifest in sorted((root / '.dctx0ctl/analysis/runs').glob('*/*manifest.json')):
                            raw = json.loads(raw_manifest.read_text())
                            provenance = raw['provenance']
                            builds.append({'elapsed_ms': provenance.get('phase_build_ms', provenance['build_ms']), 'peak_rss_bytes': provenance.get('phase_build_peak_rss_bytes', provenance['build_peak_rss_bytes'])})
                            for execution_file in raw.get('execution_files', []):
                                execution = json.loads(Path(execution_file).read_text())
                                p = execution['provenance']
                                tests.append({'package': p['package_name'], 'target_kind': p['test_target_kind'], 'target_name': p['test_target_name'], 'test_name': p['test_name'], 'outcome': execution['outcome'], 'exit_code': p['exit_code'], 'elapsed_ms': execution['duration_ms'], 'peak_rss_bytes': execution['peak_rss_bytes']})
                        sample['instrumented_builds'] = builds
                        sample['isolated_test_executions'] = tests
                        sample['artifact_bytes'] = sum(p.stat().st_size for p in (root / '.dctx0ctl/analysis').rglob('*') if p.is_file())
                    if index:
                        samples.append(sample)
                report['measurements'][phase] = {'samples': samples, 'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples), 'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples), 'cost_scope': 'CLI wall time including startup/serialization; Cargo instrumented build/test costs included only for catalog/collection/replay'}
            retained = logdir / 'dataset.ndjson'
            retained.write_bytes(dataset.read_bytes())
            report['dataset'] = str(retained.relative_to(ROOT))
            shutil.copytree(setup / '.dctx0ctl/analysis/artifacts', logdir / 'artifacts')
            report['artifacts'] = str((logdir / 'artifacts').relative_to(ROOT))
        build_variables = {'CARGO_INCREMENTAL', 'CARGO_BUILD_JOBS', 'RUSTUP_TOOLCHAIN', 'RUSTC_BOOTSTRAP'}
        build_variables.update('CARGO_PROFILE_' + profile + '_' + field for profile in ('DEV', 'TEST', 'RELEASE') for field in ('DEBUG', 'OPT_LEVEL', 'DEBUG_ASSERTIONS', 'OVERFLOW_CHECKS', 'CODEGEN_UNITS', 'LTO', 'PANIC'))
        environment = {name: os.environ.get(name) for name in sorted(build_variables)}
        report['build_environment'] = environment
        workload_hash = hashlib.sha256(Path(__file__).read_bytes() + json.dumps(environment, sort_keys=True).encode() + sys.version.encode()).hexdigest()
        for metrics in report['measurements'].values():
            metrics['application_source_hash'] = workload_hash
        report['status'] = 'complete'
    except (OSError, RuntimeError, ValueError) as error:
        report['error'] = str(error)
    report['source_hash_after'] = validate.digest(validate.validation_sources())
    if report['source_hash'] != report['source_hash_after']:
        report['status'] = 'incomplete'
        report['error'] = 'Source changed during validation'
    HISTORY.mkdir(parents=True, exist_ok=True)
    with validate.locked():
        destination = HISTORY / f'{stamp}-{os.getpid()}.json'
        validate.atomic(destination, report)
        for old in sorted(HISTORY.glob('*.json'))[:-20]:
            old.unlink()
    print(destination, flush=True)
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    from memory_guard import guard_script
    guard_script()
    sys.exit(main())
