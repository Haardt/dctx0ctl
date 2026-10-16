#!/usr/bin/env python3
"""Run validation plus release benchmarks; retain 20 runs and explicit baselines."""
import argparse
import datetime
import fcntl
import hashlib
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
STORE = ROOT / 'benchmarks'
WORKLOADS = {'cinema': 'examples/cinema/dctx0'}


def capture(command):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else None


def digest(paths):
    h = hashlib.sha256()
    for path in sorted(paths):
        h.update(str(path.relative_to(ROOT)).encode())
        h.update(path.read_bytes())
    return h.hexdigest()


def validation_sources():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', *ROOT.glob('crates/**/*.rs'), *ROOT.glob('crates/**/Cargo.toml'), *ROOT.glob('scripts/**/*.py'), *ROOT.glob('targets/**/*.rs'), *ROOT.glob('targets/**/Cargo.toml'), *ROOT.glob('targets/**/*.md'), *ROOT.glob('targets/**/*.ts'), *ROOT.glob('examples/cinema/src/**/*.ts'), *ROOT.glob('examples/cinema/.dctx0/testing/typescript/*.ts'), *ROOT.glob('examples/cinema/.dctx0/testing/**/*.json'), ROOT / 'examples/cinema/package.json', ROOT / 'examples/cinema/bun.lock']
    adaptive = ROOT / 'fixtures/conformance/adaptive-typescript'
    paths.extend(adaptive.glob('*/src/**/*.ts'))
    paths.extend(adaptive.glob('*/.dctx0/testing/**/*.ts'))
    paths.extend(adaptive.glob('*/.dctx0/testing/**/*.json'))
    paths.extend(adaptive.glob('spec/*.dctx0'))
    for name in ('package.json', 'bun.lock', 'tsconfig.json'):
        paths.extend(adaptive.glob('*/' + name))
    paths.extend(path for path in ROOT.glob('examples/**/*.dctx0') if path.is_file())
    paths.extend(path for path in ROOT.glob('fixtures/**/*.dctx0') if path.is_file())
    paths.extend(ROOT.glob('fixtures/**/*.json'))
    paths.extend(ROOT.glob('skills/**/*.md'))
    paths.extend(ROOT.glob('skills/**/*.yaml'))
    for pattern in ("**/*.js", "**/*.cjs", "**/*.json", "**/*.scm", "**/*.toml", "**/*.rs", "**/*.c", "**/*.h", "**/*.txt", "**/Cargo.lock"):
        paths.extend(path for path in (ROOT / "editors").glob(pattern) if "target" not in path.relative_to(ROOT / "editors").parts)
    paths.extend(ROOT.glob('targets/**/*.go'))
    paths.extend(ROOT.glob('targets/rust/harness/*.toml'))
    paths.extend(ROOT.glob('examples/cinema-rust/src/**/*.rs'))
    paths.extend(ROOT.glob('examples/cinema-rust/tests/**/*.rs'))
    paths.extend(ROOT.glob('examples/cinema-rust/.dctx0/testing/rust/*.rs'))
    paths.extend(ROOT.glob('examples/cinema-rust/.dctx0/testing/**/*.json'))
    paths.extend(ROOT / 'examples/cinema-rust' / name for name in ('Cargo.toml','Cargo.lock','.dctx0/testing/rust/Cargo.toml','.dctx0/testing/rust/Cargo.lock'))
    paths.extend(ROOT.glob('examples/cinema-go/**/*.go'))
    paths.extend(ROOT.glob('examples/cinema-go/.dctx0/testing/go/*.go'))
    paths.extend(ROOT.glob('examples/cinema-go/.dctx0/testing/**/*.json'))
    paths.extend(ROOT / 'examples/cinema-go' / name for name in ('go.mod', 'go.sum'))
    for project in ROOT.glob('examples/tutorial-step-*'):
        paths.extend(project.glob('src/**/*.ts'))
        paths.extend(project.glob('*.ts'))
        paths.extend(project.glob('.dctx0/testing/typescript/*.ts'))
        paths.extend(project.glob('.dctx0/testing/**/*.json'))
        paths.extend(project / name for name in ('package.json', 'bun.lock', 'tsconfig.json'))
    for project in ROOT.glob('examples/tutorial-variants/*'):
        if not project.is_dir():
            continue
        paths.extend(project.glob('src/**/*.ts'))
        paths.extend(project.glob('.dctx0/testing/**/*.ts'))
        paths.extend(project.glob('.dctx0/testing/**/*.json'))
        paths.extend(project.glob('.dctx0/profiles/**/*.md'))
        paths.extend(project / name for name in ('package.json', 'bun.lock', 'tsconfig.json', 'AGENTS.md'))
    paths.extend((ROOT / 'examples/tutorial').glob('*.ts'))
    paths.extend((ROOT / 'examples/tutorial').glob('*.sh'))
    paths.append(ROOT / 'examples/tutorial/features.json')
    paths.extend(path for path in (ROOT / "ui/site").rglob("*") if path.is_file() and "node_modules" not in path.parts)
    paths.extend(ROOT.glob("dctx0/*.dctx0"))
    paths.extend(ROOT.glob(".dctx0/testing/rust/*.rs"))
    paths.extend(ROOT / ".dctx0/testing/rust" / name for name in ("Cargo.toml", "Cargo.lock"))
    paths.append(adaptive / 'production-frozen.json')
    return paths


def atomic(path, data):
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as handle:
        json.dump(data, handle, indent=2)
        handle.write('\n')
        handle.flush()
        os.fsync(handle.fileno())
        temporary = handle.name
    os.replace(temporary, path)


def locked():
    STORE.mkdir(exist_ok=True)
    handle = (STORE / '.lock').open('a')
    fcntl.flock(handle, fcntl.LOCK_EX)
    return handle


def measure(binary, path, iterations, phase=None, database=None, target=None):
    # wait4 reaps this process. RSS comes from its post-exec /proc/self/status;
    # wait4's ru_maxrss can include the pre-exec Python parent's memory.
    with tempfile.TemporaryFile() as output:
        command = [str(binary), 'bench-parse', path, str(iterations)] if phase is None else [str(binary), 'bench', path, phase, str(iterations)]
        if target is not None:
            command = [str(binary), 'bench-profiles', path, target, phase, str(iterations)]
        if database is not None:
            command += ['--db', str(database)]
        process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, close_fds=False)
        started = time.perf_counter()
        _, status, _ = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
        wall_ms = (time.perf_counter() - started) * 1000
        output.seek(0)
        content = output.read().decode()
    if process.returncode:
        raise RuntimeError(f'Benchmark failed ({process.returncode}): {content}')
    data = json.loads(content)
    if not data['valid']:
        raise RuntimeError('Benchmark input produced parser diagnostics')
    if not data.get('peak_rss_bytes'):
        raise RuntimeError('Peak RSS measurement unavailable')
    result = {'elapsed_ms': data['elapsed_ms'] / iterations, 'process_wall_ms': wall_ms,
              'peak_rss_bytes': data['peak_rss_bytes']}
    if 'application_source_hash' in data:
        result['application_source_hash'] = data['application_source_hash']
    return result


def benchmark(args):
    selected = getattr(args, 'example', 'all')
    workloads = WORKLOADS if selected == 'all' else {selected: WORKLOADS[selected]}
    record = {'schema_version': 1, 'timestamp': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'status': 'incomplete', 'measurements': {}, 'settings': {'iterations': args.iterations, 'samples': args.samples,
              'warmup_runs': 1, 'profile': 'release', 'features': 'all', 'cache': 'warm filesystem; fresh process per sample'},
              'toolchain': capture(['rustc', '--version']), 'platform': platform.platform(),
              'hardware': {'machine': platform.machine(), 'cpu_count': os.cpu_count(),
                           'cpu': next((s.split(':', 1)[1].strip() for s in Path('/proc/cpuinfo').read_text().splitlines() if s.startswith('model name')), 'unknown')},
              'revision': capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': capture(['git', 'status', '--porcelain']),
              'source_hash': digest(validation_sources()),
              'input_hashes': {name: digest((ROOT / path).rglob('*.dctx0')) for name, path in workloads.items()},
              'memory_scope': 'Linux post-exec VmHWM from standalone Rust parser process /proc/self/status; no child processes'}
    try:
        subprocess.run(['cargo', 'build', '--release', '--workspace', '--all-features', '--locked'], cwd=ROOT, check=True)
        binary = ROOT / 'target/release/dctx0-parse-bench'
        for name, path in workloads.items():
            measure(binary, path, args.iterations)
            samples = [measure(binary, path, args.iterations) for _ in range(args.samples)]
            record['measurements'][name] = {'samples': samples,
                'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples)}
        # Phase timings exclude setup; RSS includes the standalone process and
        # its prepared inputs. SQLite runs use disposable databases only.
        phases = ('compile', 'graph', 'snapshot', 'diff', 'diff-changed', 'persist', 'check', 'check-changed', 'tasks-plan', 'tasks-reconcile', 'tasks-publish', 'tasks-list', 'tasks-context-md', 'tasks-context-json', 'tasks-slice', 'tasks-claim', 'verification-evaluate-unbound', 'verification-assess-unbound', 'verification-generate', 'verification-project', 'verification-harness', 'verification-init', 'verification-history-publish', 'verification-history-read', 'verification-discover')
        phase_iterations = min(args.iterations, 3)
        for name, path in workloads.items():
            for phase in phases:
                with tempfile.TemporaryDirectory(prefix='dctx0-benchmark-') as temporary:
                    database = Path(temporary) / 'history.sqlite3'
                    binary = ROOT / 'target/release/dctx0'
                    measure(binary, path, phase_iterations, phase, database)
                    samples = [measure(binary, path, phase_iterations, phase, database) for _ in range(args.samples)]
                    if phase == 'verification-discover' and len({s['application_source_hash'] for s in samples}) != 1:
                        raise RuntimeError('Discovery application sources changed between samples')
                    record['measurements'][f'{name}/{phase}'] = {
                        'samples': samples, 'iterations': phase_iterations,
                        **({'protocol_version': 2} if phase == 'tasks-publish' else {}),
                        **({'application_source_hash': samples[0]['application_source_hash'], 'protocol_version': 1} if phase == 'verification-discover' else {}),
                        'memory_scope': 'Linux post-exec VmHWM; includes phase setup, IR and SQLite; no children',
                        'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                        'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples)}
        for target in ('typescript', 'java', 'go', 'rust'):
            for name, path in workloads.items():
                for phase in ('create', 'load', 'context-md', 'context-json', 'claim'):
                    with tempfile.TemporaryDirectory(prefix='dctx0-profile-benchmark-') as temporary:
                        database = Path(temporary) / 'history.sqlite3'
                        measure(binary, path, phase_iterations, phase, database, target)
                        samples = [measure(binary, path, phase_iterations, phase, database, target) for _ in range(args.samples)]
                        record['measurements'][f'{name}/profiles/{target}/{phase}'] = {
                            'samples': samples, 'iterations': phase_iterations, 'protocol_version': 1,
                            'memory_scope': 'Linux post-exec VmHWM; profile files only; no compiler or SQLite' if phase in ('create', 'load') else 'Linux post-exec VmHWM; includes profile and task setup, IR and SQLite; no children',
                            'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                            'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples)}
        if getattr(args, 'tutorial', False):
            # New identities are unassessed until explicit baseline promotion.
            for step in (1, 8, 12, 16):
                project = next(ROOT.glob(f'examples/tutorial-step-{step}-*'))
                path = str((project / 'dctx0').relative_to(ROOT))
                for phase in ('compile', 'verification-project', 'verification-harness'):
                    with tempfile.TemporaryDirectory(prefix='dctx0-tutorial-benchmark-') as temporary:
                        database = Path(temporary) / 'history.sqlite3'
                        measure(binary, path, phase_iterations, phase, database)
                        samples = [measure(binary, path, phase_iterations, phase, database) for _ in range(args.samples)]
                    record['measurements'][f'tutorial-step-{step}/{phase}'] = {
                        'samples': samples, 'iterations': phase_iterations,
                        'memory_scope': 'Linux post-exec VmHWM; Rust preparation/projection/artifact generation; no native children',
                        'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                        'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples)}
        command = [sys.executable, 'scripts/go_cinema_performance.py', '--binary', str(ROOT / 'target/release/dctx0')]
        subprocess.run(command, cwd=ROOT, check=True)
        native = json.loads((ROOT / 'target/go-cinema/performance.json').read_text())
        record['go_cinema_performance'] = native
        for phase, sample in native['samples'].items():
            record['measurements'][f'cinema-go/{phase}'] = {
                'samples': [sample], 'iterations': 1, 'elapsed_ms': sample['elapsed_ms'],
                'peak_rss_bytes': sample['peak_rss_bytes'],
                'memory_scope': native['memory_scope'], 'protocol_version': 1,
                'workload_settings': {'seed': 42, 'generated_cases': 100, 'cache': native['cache']},
            }
        subprocess.run([sys.executable,'scripts/rust_cinema_performance.py','--binary',str(ROOT / 'target/release/dctx0')],cwd=ROOT,check=True)
        native_rust=json.loads((ROOT / 'target/rust-cinema/performance.json').read_text())
        record['rust_cinema_performance']=native_rust
        for phase,sample in native_rust['samples'].items():
            record['measurements'][f'cinema-rust/{phase}']={'samples':[sample],'iterations':1,'elapsed_ms':sample['elapsed_ms'],'peak_rss_bytes':sample['peak_rss_bytes'],'memory_scope':native_rust['memory_scope'],'protocol_version':1,'workload_settings':{'seed':42,'generated_cases':100,'cache':native_rust['cache']}}
        subprocess.run([sys.executable, 'scripts/editor_acceptance.py', '--binary', str(ROOT / 'target/release/dctx0'), '--performance-only'], cwd=ROOT, check=True)
        editor_performance = json.loads((ROOT / 'target/editors/performance.json').read_text())
        record['editor_performance'] = editor_performance
        for phase, sample in editor_performance['samples'].items():
            record['measurements'][f'cinema-editor/{phase}'] = {'samples': [sample], 'iterations': 1, 'elapsed_ms': sample['elapsed_ms'], 'peak_rss_bytes': sample['peak_rss_bytes'], 'memory_scope': editor_performance['memory_scope'], 'protocol_version': 1, 'workload_settings': {'cache': 'cold server, warm OS cache', 'project': 'cinema'}}
        record['source_hash_after'] = digest(validation_sources())
        if record['source_hash_after'] != record['source_hash']:
            raise RuntimeError('Benchmark sources changed during measurement; rerun on a stable tree')
        record['status'] = 'complete' 
    except (RuntimeError, subprocess.CalledProcessError, OSError, ValueError) as error:
        record['error'] = str(error)
    return record


def compatible(left, right):
    return all(left.get(key) == right.get(key) for key in
               ('schema_version', 'settings', 'toolchain', 'platform', 'hardware', 'input_hashes', 'memory_scope'))


def workload_compatible(accepted, current):
    return accepted is not None and all(accepted.get(key) == current.get(key) for key in ('iterations', 'memory_scope', 'protocol_version', 'application_source_hash'))


def compare(record, baseline, args):
    if not baseline:
        print('BASELINE MISSING: measurements saved; regression assessment unavailable.')
        return True
    if not compatible(record, baseline):
        print('BASELINE INCOMPATIBLE: measurements saved; regression assessment unavailable.')
        return True
    passed = True
    for name, metrics in record['measurements'].items():
        accepted = baseline['measurements'].get(name)
        if not workload_compatible(accepted, metrics):
            print(f'{name}: BASELINE MISSING OR INCOMPATIBLE; phase assessment unavailable.')
            continue
        for metric, tolerance in [('elapsed_ms', args.speed_tolerance), ('peak_rss_bytes', args.ram_tolerance)]:
            before, now = baseline['measurements'][name][metric], metrics[metric]
            percent = (now / before - 1) * 100 if before else None
            regression = now > before * (1 + tolerance / 100)
            passed &= not regression
            delta = f'{percent:+.2f}%' if percent is not None else 'percentage unavailable'
            print(f'{name} {metric}: {now:.3f}; delta {now-before:+.3f} ({delta}); tolerance {tolerance}% ' + ('REGRESSION' if regression else 'OK'))
    return passed


def save(record, args):
    with locked():
        history = STORE / 'history'
        history.mkdir(exist_ok=True)
        baseline_path = STORE / 'baseline.json'
        baseline = json.loads(baseline_path.read_text()) if baseline_path.exists() else None
        gate = record['status'] == 'complete' and compare(record, baseline, args)
        record['performance_gate'] = ('unassessed' if gate and (not baseline or not compatible(record, baseline)) else 'pass' if gate else 'fail')
        record['unassessed_workloads'] = [name for name, metrics in record['measurements'].items() if not baseline or not workload_compatible(baseline.get('measurements', {}).get(name), metrics)]
        if gate and baseline and compatible(record, baseline) and record['unassessed_workloads']:
            record['performance_gate'] = 'partial'
        record['tolerances_percent'] = {'speed': args.speed_tolerance, 'ram': args.ram_tolerance}
        previous = []
        for path in sorted(history.glob('*.json')):
            item = json.loads(path.read_text())
            if item['status'] == 'complete' and compatible(record, item):
                previous.append(item)
        for name, metrics in record['measurements'].items():
            comparable = [r for r in previous if workload_compatible(r['measurements'].get(name), metrics)]
            metrics['recent_comparable_runs'] = len(comparable)
            for metric in ('elapsed_ms', 'peak_rss_bytes'):
                print(f'{name} {metric}: {metrics[metric]:.3f}')
                if comparable:
                    median = statistics.median(r['measurements'][name][metric] for r in comparable)
                    print(f'  history median {median:.3f}; delta {metrics[metric]-median:+.3f}')
        name = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + f'-{os.getpid()}.json'
        atomic(history / name, record)
        for old in sorted(history.glob('*.json'))[:-20]:
            old.unlink()
        print(f'Measurement: benchmarks/history/{name}')
        if record.get('error'):
            print(record['error'], file=sys.stderr)
        return gate


def accept(selected):
    with locked():
        candidates = sorted((STORE / 'history').glob('*.json'))
        if not candidates:
            raise ValueError('No recorded measurements')
        path = candidates[-1] if selected == 'latest' else STORE / 'history' / selected
        if path.parent != STORE / 'history' or path not in candidates:
            raise ValueError('Select a filename from benchmarks/history')
        data = json.loads(path.read_text())
        if data['status'] != 'complete' or data.get('validation_status') != 'pass':
            raise ValueError('Only complete measurements with successful validation can become a baseline')
        data['baseline_source'] = path.name
        atomic(STORE / 'baseline.json', data)
        print(f'Baseline explicitly set from {path.name}')


def main():
    os.environ.setdefault("DCTX0_TEST_PYTHON", sys.executable)
    os.chdir(ROOT)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--accept-baseline', metavar='RUN', help='latest or a history filename; performs no tests')
    parser.add_argument('--example', choices=('all', 'cinema'), default='all', help='select release measurement scope; cinema defers the two-example benchmark conformance test')
    parser.add_argument('--coverage', action='store_true', help='enforce 97%% line/function coverage')
    parser.add_argument('--tutorial', action='store_true', help='validate all 16 TypeScript steps, feature coverage, mutations and replay; measure representative tutorial Rust phases last')
    parser.add_argument('--skip-performance', action='store_true', help='defer release measurements; save check results separately from benchmark history')
    parser.add_argument('--focus', choices=('workspace', 'native-transport', 'native-executor', 'verification', 'editors', 'conformance-reviews'), default='workspace', help='native-transport is a partial development check; release measurements still run')
    parser.add_argument('--samples', type=int, default=5)
    parser.add_argument('--iterations', type=int, default=20)
    parser.add_argument('--speed-tolerance', type=float, default=20)
    parser.add_argument('--ram-tolerance', type=float, default=10)
    args = parser.parse_args()
    if sys.platform != 'linux':
        parser.error('Peak RSS measurement currently supports Linux only')
    if args.samples < 1 or args.iterations < 1 or args.speed_tolerance < 0 or args.ram_tolerance < 0:
        parser.error('Samples/iterations must be positive; tolerances must be non-negative')
    if args.accept_baseline:
        accept(args.accept_baseline)
        return 0
    commands = [[sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests'],
                ['cargo', 'check', '--workspace', '--all-targets', '--all-features', '--locked'],
                ['cargo', 'build', '--workspace', '--locked'],
                ['cargo', 'fmt', '--all', '--', '--check'],
                ['cargo', 'clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings'],
                ['cargo', 'test', '--no-fail-fast', '--workspace', '--all-features', '--locked'],
                ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--no-default-features', '--locked'],
                ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--no-default-features', '--features', 'rust', '--locked'],
                ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--no-default-features', '--features', 'go', '--locked']]
    if args.focus == 'native-transport':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-target-sdk', '-p', 'dctx0-target-typescript', '--all-features', '--locked']]
    if args.focus == 'native-executor':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-engine', '--test', 'native', '--locked']]
    if args.focus == 'verification':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-interpreter', '-p', 'dctx0-verification', '-p', 'dctx0-target-sdk', '-p', 'dctx0-target-typescript', '-p', 'dctx0-tasks', '-p', 'dctx0-store', '-p', 'dctx0-engine', '--all-features', '--locked']]
    if args.focus == 'editors':
        commands = commands[:5] + [['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-lsp', '-p', 'dctx0-engine', '-p', 'dctx0-cli', '--lib', '--test', 'analysis', '--test', 'lsp', '--test', 'editors', '--test', 'skills', '--all-features', '--locked']]
    if args.focus == 'conformance-reviews':
        commands = commands[:5] + [
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-tasks', '-p', 'dctx0-store', '--all-features', '--locked'],
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-engine', '--lib', '--test', 'tasks', '--test', 'profiles', '--all-features', '--locked'],
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--test', 'tasks', '--test', 'verification_complete', '--all-features', '--locked'],
            ['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'],
            [sys.executable, 'scripts/conformance_review_acceptance.py', '--binary', str(ROOT / 'target/release/dctx0')],
        ]
    for command in commands:
        if command[:2] == ['cargo', 'test']:
            command.extend(['--', '--skip', 'verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'])
    if args.focus not in ('editors', 'conformance-reviews'):
        commands.insert(0, ['bun', 'install', '--frozen-lockfile', '--cwd', 'examples/cinema'])
        commands.insert(1, ['bun', 'run', '--cwd', 'examples/cinema', 'typecheck'])
        commands.insert(2, ['bun', 'test', 'examples/cinema/src', './examples/cinema/.dctx0/testing/typescript/http-integration.test.ts', './examples/cinema/.dctx0/testing/typescript/persistence-conformance.test.ts'])
        for index, variant in enumerate(('direct', 'service'), start=3):
            commands.insert(index, ['bun', 'install', '--frozen-lockfile', '--cwd', f'fixtures/conformance/adaptive-typescript/{variant}'])
        commands.extend([
            ['go', 'mod', 'download', '-C', 'examples/cinema-go'],
            ['go', 'test', '-C', 'examples/cinema-go', '-coverprofile=' + str(ROOT / 'target/go-cinema.coverage'), './...'],
            ['go', 'vet', '-C', 'examples/cinema-go', './...'],
            ['go', 'test', '-C', 'examples/cinema-go', '-race', './...'],
            ['cargo', 'build', '-p', 'dctx0-cli', '--all-features', '--locked'],
            [sys.executable, 'scripts/go_cinema_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0')],
            ['cargo','test','--no-fail-fast','--manifest-path','examples/cinema-rust/Cargo.toml','--locked','--offline','--','--skip','verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'],
            ['cargo','clippy','--manifest-path','examples/cinema-rust/Cargo.toml','--all-targets','--locked','--offline','--','-D','warnings'],
            [sys.executable,'scripts/rust_cinema_acceptance.py','--binary',str(ROOT / 'target/debug/dctx0')],
            ['cargo','test','--no-fail-fast','--manifest-path','examples/cinema-rust/Cargo.toml','--all-features','--offline','--locked','--','--skip','verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'],
            ['cargo','test','--no-fail-fast','--manifest-path','examples/cinema-rust/.dctx0/testing/rust/Cargo.toml','--lib','--offline','--locked','--','--skip','verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'],
        ])
    if args.tutorial:
        commands.extend([
            ['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'],
            ['bun', 'examples/tutorial/verify-series.ts'],
            ['bun', 'examples/tutorial/mutation-acceptance.ts'],
            ['bun', 'examples/tutorial/verify-variants.ts', '--skip-performance'],
        ])
    if args.focus != 'conformance-reviews':
        commands.append([sys.executable, 'scripts/editor_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0'), '--functional-only'])
    if args.coverage:
        commands.append([sys.executable, 'scripts/coverage.py'] + (['--skip-performance'] if args.skip_performance else []))
    passed = True
    validation_steps = []
    # Native evidence and instrumented verifier images can temporarily exceed
    # a small RAM-backed /tmp. Use an owned on-disk cache for functional stages;
    # leave the parent's environment unchanged for established release workloads.
    temporary_base = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'dctx0ctl'
    temporary_base.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='validation-', dir=temporary_base) as temporary:
        functional_environment = {**os.environ, 'TMPDIR': temporary}
        # Fresh installs materialize dependency inputs that the provenance
        # inventory deliberately hashes. Prepare them before pinning that
        # inventory, then retain the same source-change guard for every check.
        preparation = [command for command in commands if command[:2] == ['bun', 'install']]
        commands = [command for command in commands if command[:2] != ['bun', 'install']]
        for command in preparation:
            print('+ ' + ' '.join(command), flush=True)
            returncode = subprocess.run(command, cwd=ROOT, env=functional_environment).returncode
            validation_steps.append({'command': command, 'returncode': returncode})
            passed = passed and returncode == 0
        validation_source_hash = digest(validation_sources())
        validation_started_ns = time.time_ns()
        for command in commands:
            print('+ ' + ' '.join(command), flush=True)
            returncode = subprocess.run(command, cwd=ROOT, env=functional_environment).returncode
            validation_steps.append({'command': command, 'returncode': returncode})
            if returncode:
                passed = False
                # Independent stages retain their failures; measurements build
                # separately and cannot certify a stale executable.
    validation_source_hash_after = digest(validation_sources())
    if validation_source_hash_after != validation_source_hash:
        passed = False
        print('Validation source inputs changed during checks; results do not certify the final tree', flush=True)
    if args.skip_performance:
        report = {'schema_version': 1, 'validation_scope': args.focus,
                  'validation_status': 'pass' if passed else 'fail',
                  'validation_steps': validation_steps,
                  'validation_source_hash': validation_source_hash,
                  'validation_source_hash_after': validation_source_hash_after,
                  'performance_status': 'deferred',
                  'deferred_tests': ['verification_core_benchmarks_measure_both_compiled_examples_without_native_claims']}
        destination = ROOT / 'target/validation-without-performance.json'
        destination.parent.mkdir(parents=True, exist_ok=True)
        atomic(destination, report)
        print(f'Performance measurements deferred; checks saved to {destination}', flush=True)
        return 0 if passed else 1
    # Benchmark conformance runs only after all functional/coverage stages.
    conformance = ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--all-features', '--locked',
                   'verification_core_benchmarks_measure_both_compiled_examples_without_native_claims']
    conformance_code = 0
    if args.example == 'all':
        conformance_code = subprocess.run(conformance, cwd=ROOT).returncode
        validation_steps.append({'command': conformance, 'returncode': conformance_code})
    passed = passed and conformance_code == 0
    if args.tutorial:
        command = ['bun', 'examples/tutorial/verify-variants.ts', '--performance-only']
        returncode = subprocess.run(command, cwd=ROOT).returncode
        validation_steps.append({'command': command, 'returncode': returncode})
        passed = passed and returncode == 0
    record = benchmark(args)
    dependency_command = [sys.executable, 'scripts/dependency_resolution_acceptance.py', '--binary', str(ROOT / 'target/release/dctx0')]
    dependency_code = subprocess.run(dependency_command, cwd=ROOT).returncode
    validation_steps.append({'command': dependency_command, 'returncode': dependency_code})
    passed = passed and dependency_code == 0
    record['validation_source_hash'] = validation_source_hash
    record['validation_source_hash_after'] = validation_source_hash_after
    if record.get('source_hash') != validation_source_hash_after:
        passed = False
        print('Measured release inputs differ from the checked source tree', flush=True)
    record['validation_scope'] = args.focus
    record['validation_steps'] = validation_steps
    adaptive_reports = sorted((p for p in (ROOT / 'target/adaptive-conformance').glob('*.json') if p.stat().st_mtime_ns >= validation_started_ns), key=lambda p: p.stat().st_mtime_ns)
    record['adaptive_conformance'] = json.loads(adaptive_reports[-1].read_text()) if passed and adaptive_reports else {'status': 'unavailable', 'reason': 'No fresh successful adaptive report from this validation'}
    executor_reports = sorted((p for p in (ROOT / 'target/native-executor-conformance').glob('*.json') if p.stat().st_mtime_ns >= validation_started_ns), key=lambda p: p.stat().st_mtime_ns)
    record['native_executor_conformance'] = json.loads(executor_reports[-1].read_text()) if passed and executor_reports else {'status': 'unavailable'}
    native_reports = sorted((p for p in (ROOT / 'target/native-harness-conformance').glob('*.json') if p.stat().st_mtime_ns >= validation_started_ns), key=lambda p: p.stat().st_mtime_ns)
    record['native_transport_conformance'] = json.loads(native_reports[-1].read_text()) if passed and native_reports else {'status': 'unavailable', 'reason': 'No successful native transport conformance report from this validation'}
    record['validation_status'] = 'pass' if passed else 'fail'
    gate = save(record, args)
    return 0 if passed and gate else 1


if __name__ == '__main__':
    from memory_guard import guard_script
    guard_script()
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
