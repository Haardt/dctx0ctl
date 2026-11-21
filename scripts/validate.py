#!/usr/bin/env python3
"""Run validation plus release benchmarks; retain 20 runs and explicit baselines."""
import history_retention
import argparse
import contextlib
import datetime
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import sqlite3
import subprocess
import sys
import tempfile
import time
import validation_cache
import incremental_benchmark

ROOT = Path(__file__).resolve().parents[1]
STORE = ROOT / 'benchmarks'
WORKLOADS = {'cinema': 'examples/cinema/dctx0'}


def artifact_root():
    return Path(os.environ.get('DCTX0_VALIDATION_ARTIFACT_ROOT', ROOT / 'target'))


def functional_cache_root():
    """Choose a short, private on-disk root without consulting long TMPDIR."""
    configured = os.environ.get('DCTX0_VALIDATION_TEMP_ROOT')
    base = Path(configured) if configured is not None else Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'dctx0ctl'
    if not base.is_absolute():
        raise ValueError('DCTX0_VALIDATION_TEMP_ROOT must be an absolute short on-disk path')
    base = base.resolve()
    # The native engine appends an IPC directory, Linux pid/nonce and /s.
    # Reject early and leave a clear override rather than fail inside a test.
    longest = base / ('v-' + 'x' * 8) / 'dctx0-ipc-ffffffff-ffffffffffffffff' / 's'
    if len(os.fsencode(longest)) > 100:
        raise ValueError('Validation temp root is too long for native sockets; set DCTX0_VALIDATION_TEMP_ROOT to a shorter absolute on-disk path')
    base.mkdir(mode=0o700, parents=True, exist_ok=True)
    return base


def create_held_functional_cache(artifact_directory):
    """Record a unique retained cache before any functional child can run."""
    temporary = Path(tempfile.mkdtemp(prefix='v-', dir=functional_cache_root())).resolve()
    atomic(artifact_directory / 'run.json', {'status': 'incomplete', 'artifact_root': str(artifact_directory.resolve()),
                                             'functional_cache': str(temporary)})
    return str(temporary)


def capture(command):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else None


def digest(paths):
    h = hashlib.sha256()
    for path in sorted(paths):
        h.update(str(path.relative_to(ROOT)).encode())
        h.update(path.read_bytes())
    return h.hexdigest()


def application_go_coverage(profiles):
    """Union statement blocks only when every profile names the identical source map."""
    merged = {}
    expected_blocks = None
    for profile in profiles:
        lines = profile.read_text().splitlines()
        if not lines or lines[0] not in ('mode: set', 'mode: count', 'mode: atomic'):
            raise ValueError('Unknown Go helper coverage format')
        blocks = {}
        for line in lines[1:]:
            location, statements, count = line.rsplit(' ', 2)
            prefix = 'github.com/Haardt/dctx0ctl/applicationtests/go/'
            filename = location.removeprefix(prefix).split(':', 1)[0]
            if not location.startswith(prefix) or '/' in filename or not (ROOT / 'targets/go/applicationtests' / filename).is_file():
                raise ValueError('Go coverage names a foreign helper source')
            if location in blocks or int(statements) < 0 or int(count) < 0:
                raise ValueError('Invalid or duplicate Go helper coverage block')
            blocks[location] = int(statements)
            previous = merged.get(location, (int(statements), False))
            if previous[0] != int(statements):
                raise ValueError('Go helper coverage sources differ')
            merged[location] = (int(statements), previous[1] or int(count) > 0)
        if expected_blocks is not None and blocks != expected_blocks:
            raise ValueError('Go helper coverage source maps differ')
        expected_blocks = blocks
    total = sum(count for count, _ in merged.values())
    covered = sum(count for count, hit in merged.values() if hit)
    if not total:
        raise ValueError('No Go helper coverage statements')
    return {'method': 'union of identical Go statement blocks across unit and actual native pilot profiles; not branch/function coverage',
            'statements': total, 'covered_statements': covered, 'statement_percent': covered * 100 / total,
            'profiles': [str(path.relative_to(ROOT)) for path in profiles],
            'source_hashes': {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in (ROOT / 'targets/go/applicationtests').glob('*.go') if not path.name.endswith('_test.go')}}


def validation_sources():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', *ROOT.glob('crates/**/*.rs'), *ROOT.glob('crates/**/*.py'), *ROOT.glob('crates/**/Cargo.toml'), *ROOT.glob('scripts/**/*.py'), *ROOT.glob('targets/**/*.rs'), *ROOT.glob('targets/**/Cargo.toml'), *ROOT.glob('targets/**/*.md'), *ROOT.glob('targets/**/*.ts'), *ROOT.glob('examples/cinema/src/**/*.ts'), *ROOT.glob('examples/cinema/.dctx0/testing/typescript/*.ts'), *ROOT.glob('examples/cinema/.dctx0/testing/**/*.json'), ROOT / 'examples/cinema/package.json', ROOT / 'examples/cinema/bun.lock']
    paths.extend(ROOT.glob('targets/*/applicationtests/package.json'))
    paths.extend(ROOT.glob('examples/cinema/scripts/*.ts'))
    paths.extend(ROOT.glob('examples/cinema/scripts/*.py'))
    paths.extend(ROOT.glob('examples/cinema/.dctx0/application-tests/**/*.ts'))
    paths.extend(ROOT.glob('examples/cinema/.dctx0/application-tests/library/package.json'))
    paths.extend(ROOT.glob('examples/cinema/.dctx0/application-tests/library-pin.json'))
    paths.extend(ROOT.glob('examples/cinema-go/.dctx0/application-tests/library/go.mod'))
    paths.extend(ROOT.glob('examples/cinema-go/.dctx0/application-tests/library-pin.json'))
    paths.extend(ROOT.glob('examples/cinema-go/.dctx0/application-tests/native-run.json'))
    paths.extend(ROOT.glob('examples/cinema-go/scripts/*.py'))
    paths.extend(ROOT.glob('examples/cinema/.dctx0/application-tests/native-run.json'))
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
    paths.extend(ROOT.glob('targets/**/go.mod'))
    paths.extend(ROOT.glob('targets/**/go.sum'))
    for suffix in ('go','rs','ts'):
        paths.extend(ROOT.glob(f'fixtures/conformance/generator/**/*.{suffix}'))
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
    # Recursive harness inventories also see Cargo fingerprints after their
    # first build. These are outputs, not reviewed compilation inputs. Keep
    # source/configuration JSON (and embedded UI dist assets) in the inventory.
    return [path for path in paths
            if not {'target', 'node_modules'}.intersection(path.relative_to(ROOT).parts)]


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


def run_native_cinema_performance(target, binary):
    """Read only the fresh receipt from this exact native benchmark invocation."""
    destination = artifact_root() / 'native-performance'
    destination.mkdir(parents=True, exist_ok=True)
    run_directory = Path(tempfile.mkdtemp(prefix=f'{target}-', dir=destination))
    report_path = run_directory / 'performance.json'
    expected_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    command = [sys.executable, f'scripts/{target}_cinema_performance.py', '--binary', str(binary), '--report', str(report_path)]
    subprocess.run(command, cwd=ROOT, check=True)
    if not report_path.is_file():
        raise RuntimeError(f'{target} native performance report missing: {report_path}')
    native = json.loads(report_path.read_text())
    if not isinstance(native, dict) or native.get('status') != 'complete' or native.get('target') != target or native.get('binary_hash') != expected_hash:
        raise RuntimeError(f'{target} native performance report is incomplete or not bound to the requested binary')
    if hashlib.sha256(binary.read_bytes()).hexdigest() != expected_hash:
        raise RuntimeError(f'{target} native binary changed during measurement')
    workspace = native.get('native_workspace', {})
    if (not isinstance(workspace, dict) or not isinstance(native.get('samples'), dict) or not native['samples']
            or not all(workspace.get(key) for key in ('path', 'tmpdir', 'filesystem', 'supervision', 'comparability'))
            or not isinstance(workspace.get('retained'), bool)
            or workspace['path'] != workspace['tmpdir'] or not Path(workspace['tmpdir']).is_absolute()
            or workspace['filesystem'] in ('tmpfs', 'ramfs', 'unavailable')
            or not all(key in native for key in ('seed', 'generated_cases', 'cache', 'memory_scope'))):
        raise RuntimeError(f'{target} native performance provenance missing')
    native['report_path'] = str(report_path)
    return native


def native_workload_settings(native):
    workspace = native['native_workspace']
    return {'seed': native['seed'], 'generated_cases': native['generated_cases'], 'cache': native['cache'],
            'filesystem': workspace['filesystem'], 'temp_root': str(Path(workspace['tmpdir']).parent),
            'retained': workspace['retained'], 'supervision': workspace['supervision']}


def benchmark(args):
    if args.focus == 'incremental':
        return incremental_benchmark.benchmark(ROOT, args, digest(validation_sources()))
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
        phases = ('compile', 'graph', 'snapshot', 'diff', 'diff-changed', 'persist', 'check', 'check-changed', 'tasks-plan', 'tasks-reconcile', 'tasks-publish', 'tasks-list', 'tasks-context-md', 'tasks-context-json', 'tasks-slice', 'tasks-claim', 'tasks-package', 'tasks-package-delta', 'tasks-package-apply', 'tasks-review-changes', 'tasks-progress', 'verification-diagnostics-summary', 'verification-diagnostics-cases', 'verification-evaluate-unbound', 'verification-assess-unbound', 'verification-generate', 'verification-project', 'application-test-plan', 'verification-harness', 'verification-codecs', 'verification-bindings-plan', 'verification-init', 'verification-history-publish', 'verification-history-read', 'verification-discover')
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
        for target in ('go', 'rust'):
            native = run_native_cinema_performance(target, ROOT / 'target/release/dctx0')
            record[f'{target}_cinema_performance'] = native
            for phase, sample in native['samples'].items():
                record['measurements'][f'cinema-{target}/{phase}'] = {
                    'samples': [sample], 'iterations': 1, 'elapsed_ms': sample['elapsed_ms'],
                    'peak_rss_bytes': sample['peak_rss_bytes'], 'memory_scope': native['memory_scope'],
                    'protocol_version': 1, 'workload_settings': native_workload_settings(native),
                }
        subprocess.run([sys.executable, 'scripts/editor_acceptance.py', '--binary', str(ROOT / 'target/release/dctx0'), '--performance-only'], cwd=ROOT, check=True)
        editor_performance = json.loads((ROOT / 'target/editors/performance.json').read_text())
        record['editor_performance'] = editor_performance
        for phase, sample in editor_performance['samples'].items():
            record['measurements'][f'cinema-editor/{phase}'] = {'samples': [sample], 'iterations': 1, 'elapsed_ms': sample['elapsed_ms'], 'peak_rss_bytes': sample['peak_rss_bytes'], 'memory_scope': editor_performance['memory_scope'], 'protocol_version': 1, 'workload_settings': {'cache': 'cold server, warm OS cache', 'project': 'cinema'}}
        record['application_test_performance'] = {}
        for target, forms in [('go', ('decoder', 'operation', 'state-change', 'assertion', 'all')), ('typescript', ('constructor', 'operation', 'state-change', 'assertion', 'all'))]:
            for form in forms:
                destination = artifact_root() / f'application-performance/{target}-{form}'
                if target == 'go':
                    command = [sys.executable, 'scripts/application_test_acceptance.py', '--binary', str(ROOT / 'target/release/dctx0'), '--form', form, '--report', str(destination / 'result.json')]
                else:
                    command = [sys.executable, 'examples/cinema/scripts/application-acceptance.py', '--cli', str(ROOT / 'target/release/dctx0'), '--form', form, '--output', str(destination)]
                subprocess.run(command, cwd=ROOT, check=True)
                receipt = json.loads((destination / 'result.json').read_text())
                record['application_test_performance'][f'{target}/{form}'] = receipt
                steps = receipt['steps'] if target == 'go' else {step['name']: step for step in receipt['steps']}
                selected_phases = ('prepare', 'build-native', 'native-tests', 'assessment-service') if target == 'go' else ('prepare', 'native', 'service')
                for phase in selected_phases:
                    sample = steps[phase]
                    rss = sample.get('peak_rss_bytes', sample.get('peak_process_tree_rss_bytes'))
                    if sample.get('elapsed_ms') is None or rss is None:
                        raise RuntimeError(f'Application-test performance unavailable: {target}/{form}/{phase}')
                    record['measurements'][f'cinema-application/{target}/{form}/{phase}'] = {
                        'samples': [sample], 'iterations': 1, 'protocol_version': 2,
                        'elapsed_ms': sample['elapsed_ms'], 'peak_rss_bytes': rss,
                        'memory_scope': 'Linux sampled native command/descendant RSS sum, 50 ms; shared pages counted per process; service duration includes waiting for runner',
                        'workload_settings': {'seed': 42, 'generated_cases': 100, 'cache': 'warm toolchain/module caches; fresh isolated application copy and native test binary', 'scope': 'finite application tests; full suite when form=all; no group adoption authority', 'effective_plan_hash': receipt.get('effective_plan_hash')},
                    }
        record['application_test_managed_performance'] = {}
        for target in ('go', 'typescript'):
            destination = artifact_root() / f'application-performance/{target}-managed'
            subprocess.run([sys.executable, 'scripts/application_test_managed_acceptance.py', '--replay', '--binary', str(ROOT / 'target/release/dctx0'), '--target', target, '--report', str(destination / 'result.json')], cwd=ROOT, check=True)
            receipt = json.loads((destination / 'result.json').read_text())
            record['application_test_managed_performance'][target] = receipt
            sample = receipt['steps']['managed-run']
            if sample.get('peak_rss_bytes') is None:
                raise RuntimeError(f'Managed application-test RSS unavailable: {target}')
            record['measurements'][f'cinema-application/{target}/managed/full'] = {
                'samples': [sample], 'iterations': 1, 'protocol_version': 2,
                'elapsed_ms': sample['elapsed_ms'], 'peak_rss_bytes': sample['peak_rss_bytes'],
                'memory_scope': sample['memory_scope'],
                'workload_settings': {'seed': 42, 'generated_cases': 100, 'scope': 'captured-source full suite, independent native collection, guarded task delivery and source-bound complete acceptance', 'cache': 'warm OS/toolchain/module caches; fresh capture, dependency materialization and native test binary'},
            }
        record['application_test_go_helper_coverage'] = application_go_coverage([
            artifact_root() / 'go-application-library.coverage',
            *[artifact_root() / f'application-performance/go-{form}/helper.coverage' for form in ('decoder', 'operation', 'state-change', 'assertion', 'all')],
            *[artifact_root() / f'application-validation/go-probe-{probe}/helper.coverage' for probe in ('tie-break','wrong-return','missing-write','wrong-key','cleanup','operation-input-swap')],
            *[artifact_root() / f'application-validation/go-all-probe-{probe}/helper.coverage' for probe in ('tie-break','wrong-return','operation-input-swap','missing-write','wrong-key','cleanup','unexpected-collaborator')],
        ])
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
    return accepted is not None and all(accepted.get(key) == current.get(key) for key in ('iterations', 'memory_scope', 'protocol_version', 'application_source_hash', 'workload_settings'))


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
        preserve = history_retention.preserve_history(getattr(args, 'preserve_history', False))
        history_retention.mark(record, preserve)
        atomic(history / name, record)
        history_retention.prune(history, preserve)
        if preserve:
            print('Measurement history preserved: no recorded runs deleted')
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


def validation_status(steps, sources_unchanged=True):
    errors = [step for step in steps if step['returncode']]
    if not sources_unchanged or any(step.get('status') != 'unavailable' for step in errors):
        return 'fail'
    return 'incomplete' if errors else 'pass'


def validation_step(command, returncode):
    step = {'command': command, 'returncode': returncode}
    if command == [sys.executable, 'scripts/go_race.py'] and returncode == 2:
        step.update(status='unavailable', evidence='target/go-cinema/race.json')
    return step


def read_analysis_report(stdout, source_hash):
    lines = stdout.splitlines()
    if not lines:
        raise ValueError('Research measurement did not publish a receipt')
    path = Path(lines[-1])
    if path.parent != ROOT / 'benchmarks/history/test-analysis':
        raise ValueError('Unexpected research measurement receipt path')
    report = json.loads(path.read_text())
    if report.get('schema_version') != 1 or report.get('status') != 'complete' or not source_hash == report.get('source_hash') == report.get('source_hash_after'):
        raise ValueError('Research measurement receipt is incomplete or uses different source inputs')
    return report


def cache_eligible(args):
    return (args.coverage and args.focus == 'workspace' and args.example == 'all'
            and not args.skip_performance and not args.no_reuse)


def finish_validation_cache(args, identity_before, record, passed, gate):
    """Only a complete measured successful validation can seed reuse."""
    if (identity_before is None or not cache_eligible(args) or not passed or not gate
            or record.get('status') != 'complete' or record.get('validation_status') != 'pass'
            or not record.get('source_hash')
            or not (record.get('validation_source_hash') == record.get('validation_source_hash_after') == record['source_hash'])
            or not record.get('validation_steps')
            or any(step['returncode'] for step in record['validation_steps'])):
        return None
    try:
        if validation_cache.identity(ROOT, cache_options(args)) != identity_before:
            print('Validation reuse not recorded: inputs changed during validation', flush=True)
            return None
        evidence = ROOT / 'target/validation-cache/evidence'
        evidence.mkdir(parents=True, exist_ok=True)
        content = json.dumps(record, sort_keys=True).encode()
        path = evidence / (hashlib.sha256(content).hexdigest() + '.json')
        atomic(path, record)
        receipt = validation_cache.record_success(ROOT, identity_before, [
            path, artifact_root() / 'coverage/source-summary.json',
            artifact_root() / 'coverage/workspace.lcov', artifact_root() / 'coverage/llvm.json'])
        print(f"Reusable validation receipt: {receipt['id']} ({receipt['created']})", flush=True)
        return receipt
    except (OSError, ValueError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f'Validation reuse not recorded: {error}', flush=True)
        return None


def cache_options(args):
    return {key: value for key, value in vars(args).items()
            if key not in ('force', 'no_reuse', 'reuse_max_age_seconds')}


def main():
    os.environ.setdefault("DCTX0_TEST_PYTHON", sys.executable)
    # Match CI's bounded QA images. Large symbol-heavy debug executables can
    # exceed native identity capture limits; assertions/overflow checks retain
    # the ordinary dev/test defaults. Explicit caller settings take precedence.
    os.environ.setdefault('CARGO_PROFILE_DEV_DEBUG', '0')
    os.environ.setdefault('CARGO_PROFILE_TEST_DEBUG', '0')
    os.environ.setdefault('CARGO_INCREMENTAL', '0')
    os.chdir(ROOT)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--accept-baseline', metavar='RUN', help='latest or a history filename; performs no tests')
    parser.add_argument('--example', choices=('all', 'cinema'), default='all', help='select release measurement scope; cinema defers the two-example benchmark conformance test')
    parser.add_argument('--coverage', action='store_true', help='enforce Rust line and LLVM function coverage gates')
    parser.add_argument('--tutorial', action='store_true', help='validate all 16 TypeScript steps, feature coverage, mutations and replay; measure representative tutorial Rust phases last')
    parser.add_argument('--skip-performance', action='store_true', help='defer release measurements; save check results separately from benchmark history')
    parser.add_argument('--focus', choices=('workspace', 'native-transport', 'native-executor', 'verification', 'editors', 'conformance-reviews', 'workflow-tools', 'bindings', 'experimental-coverage', 'incremental'), default='workspace', help='non-workspace focuses are partial development checks; release measurements still run')
    parser.add_argument('--preserve-history', action='store_true', help='retain every existing measurement run during an explicit migration; no baseline promotion')
    parser.add_argument('--samples', type=int, default=5)
    parser.add_argument('--iterations', type=int, default=20)
    parser.add_argument('--speed-tolerance', type=float, default=20)
    parser.add_argument('--ram-tolerance', type=float, default=10)
    parser.add_argument('--force', action='store_true', help='execute all checks even when unchanged successful validation exists')
    parser.add_argument('--no-reuse', action='store_true', help='disable validation receipt lookup and publication')
    parser.add_argument('--reuse-max-age-seconds', type=float, help='require reused evidence to be younger than this age')
    args = parser.parse_args()
    if args.preserve_history:
        os.environ[history_retention.ENVIRONMENT_KEY] = '1'
    if sys.platform != 'linux':
        parser.error('Peak RSS measurement currently supports Linux only')
    if args.samples < 1 or args.iterations < 1 or args.speed_tolerance < 0 or args.ram_tolerance < 0:
        parser.error('Samples/iterations must be positive; tolerances must be non-negative')
    if args.accept_baseline:
        accept(args.accept_baseline)
        return 0
    if args.reuse_max_age_seconds is not None and (not math.isfinite(args.reuse_max_age_seconds) or args.reuse_max_age_seconds < 0):
        parser.error('Reuse maximum age must be finite and non-negative')
    cache_identity = None
    if cache_eligible(args):
        try:
            cache_identity = validation_cache.identity(ROOT, cache_options(args))
            receipt = None if args.force else validation_cache.find_receipt(
                ROOT, cache_identity, max_age_seconds=args.reuse_max_age_seconds)
            if receipt is not None and validation_cache.identity(ROOT, cache_options(args)) == cache_identity:
                print(f"REUSED validation receipt {receipt['id']} from {receipt['created']}: unchanged recorded inputs and intact evidence.", flush=True)
                print('No tests or performance measurements executed; this is the original dated result, not a fresh run. Use --force to rerun.', flush=True)
                for artifact in receipt['artifacts']:
                    print(f"Original evidence: {artifact['path']}", flush=True)
                return 0
        except (OSError, ValueError, sqlite3.Error, subprocess.SubprocessError) as error:
            cache_identity = None
            print(f'Validation cache unavailable; executing checks: {error}', flush=True)
    artifact_directory = None
    if history_retention.preserve_history():
        runs = ROOT / 'target/validation-runs'
        runs.mkdir(parents=True, exist_ok=True)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
        artifact_directory = Path(tempfile.mkdtemp(prefix=f'{stamp}-{os.getpid()}-', dir=runs))
        os.environ['DCTX0_VALIDATION_ARTIFACT_ROOT'] = str(artifact_directory)
        retained_functional_cache = create_held_functional_cache(artifact_directory)
    commands = [[sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests'],
                ['cargo', 'check', '--workspace', '--all-targets', '--all-features', '--locked'],
                ['cargo', 'build', '--workspace', '--locked'],
                ['cargo', 'fmt', '--all', '--', '--check'],
                ['cargo', 'clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings'],
                ['cargo', 'test', '--no-fail-fast', '--workspace', '--all-features', '--locked'],
                ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--no-default-features', '--locked'],
                ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--no-default-features', '--features', 'rust', '--locked'],
                ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--no-default-features', '--features', 'go', '--locked']]
    if args.focus == 'experimental-coverage':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [
            ['cargo', 'test', '-p', 'dctx0-test-analysis', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--test', 'test_analysis', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'test_analysis', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'test_analysis', '--all-features', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--lib', 'coverage_affected', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--test', 'experimental_coverage', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'experimental_coverage', '--all-features', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'experimental_coverage', '--no-default-features', '--locked'],
        ]
    if args.focus == 'incremental':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [
            ['cargo', 'test', '-p', 'dctx0-verification', '--lib', 'incremental', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--lib', 'incremental', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--lib', 'tasks::native_proof::tests', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--lib', 'retention_preserves_all_ancestors', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--lib', 'reuse_inputs', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--lib', 'native::environment', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--lib', 'skills::environment_tests', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--lib', 'native_complete_baseline_survives', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-store', '--lib', 'retention_body_lookup', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'incremental_native', '--test', 'incremental_hybrid', '--test', 'decoder_units', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'verification_run', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'hybrid_done', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'verification_incremental', '--test', 'hybrid_run', '--all-features', '--locked'],
            ['cargo', 'check', '-p', 'dctx0-cli', '--no-default-features', '--locked'],
        ]
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
    if args.focus == 'workflow-tools':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [
            ['cargo', 'test', '-p', 'dctx0-target-typescript', '--test', 'codecs', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-engine', '--lib', 'source_bound_reviews_validate_current_requirements_and_keep_latest_judgements', '--all-features', '--locked'],
            ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'workflow_tools', '--all-features', '--locked'],
        ]
    if args.focus == 'bindings':
        if args.coverage:
            parser.error('Coverage acceptance requires workspace focus')
        commands = commands[:5] + [
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-verification', '--test', 'bindings', '--locked'],
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-engine', '--test', 'verification_bindings', '--test', 'verification_rust', '--locked'],
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--test', 'verification_bindings', '--test', 'self_verification', '--all-features', '--locked'],
            ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-target-sdk', '-p', 'dctx0-target-typescript', '-p', 'dctx0-target-go', '-p', 'dctx0-target-rust', '--locked'],
        ]
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
    if args.focus not in ('editors', 'conformance-reviews', 'workflow-tools', 'bindings', 'experimental-coverage', 'incremental'):
        commands.insert(0, ['bun', 'install', '--frozen-lockfile', '--cwd', 'examples/cinema'])
        commands.insert(1, ['bun', 'run', '--cwd', 'examples/cinema', 'typecheck'])
        commands.insert(2, ['bun', 'test', 'examples/cinema/src', './examples/cinema/.dctx0/testing/typescript/http-integration.test.ts', './examples/cinema/.dctx0/testing/typescript/persistence-conformance.test.ts'])
        for index, variant in enumerate(('direct', 'service'), start=3):
            commands.insert(index, ['bun', 'install', '--frozen-lockfile', '--cwd', f'fixtures/conformance/adaptive-typescript/{variant}'])
        commands.extend([
            ['go', 'mod', 'download', '-C', 'examples/cinema-go'],
            ['go', 'test', '-C', 'examples/cinema-go', '-coverprofile=' + str(artifact_root() / 'go-cinema.coverage'), './...'],
            ['go', 'vet', '-C', 'examples/cinema-go', './...'],
            [sys.executable, 'scripts/go_race.py'],
            ['cargo', 'build', '-p', 'dctx0-cli', '--all-features', '--locked'],
            ['go', 'test', '-C', 'targets/go/applicationtests', '-coverprofile=' + str(artifact_root() / 'go-application-library.coverage'), './...'],
            ['go', 'vet', '-C', 'targets/go/applicationtests', './...'],
            ['bun', 'test', '--coverage'] + (['--coverage-dir=' + str(artifact_root() / 'bun-coverage')] if artifact_directory is not None else []) + ['targets/typescript/applicationtests'],
            ['bun', 'examples/cinema/node_modules/typescript/bin/tsc', '--project', 'targets/typescript/applicationtests/tsconfig.json'],
            *[[sys.executable, 'scripts/application_test_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0'), '--form', form, '--report', str(artifact_root() / f'application-validation/go-{form}/result.json')] for form in ('decoder', 'operation', 'state-change', 'assertion', 'all')],
            *[[sys.executable, 'examples/cinema/scripts/application-acceptance.py', '--cli', str(ROOT / 'target/debug/dctx0'), '--form', form, '--output', str(artifact_root() / f'application-validation/typescript-{form}')] for form in ('constructor', 'operation', 'state-change', 'assertion', 'all')],
            *[[sys.executable, 'scripts/application_test_managed_acceptance.py', '--replay', '--binary', str(ROOT / 'target/debug/dctx0'), '--target', target, '--report', str(artifact_root() / f'application-validation/{target}-managed/result.json')] for target in ('go', 'typescript')],
            *[[sys.executable, 'scripts/application_test_managed_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0'), '--target', target, '--probe', probe, '--report', str(artifact_root() / f'application-validation/{target}-managed-probe-{probe}/result.json')] for target in ('go', 'typescript') for probe in ('native-assertion','missing-test','build-failure')],
            *[[sys.executable, 'scripts/application_test_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0'), '--form', form, '--probe', probe, '--report', str(artifact_root() / f'application-validation/go-probe-{probe}/result.json')] for form, probe in (('assertion', 'tie-break'), ('operation', 'wrong-return'), ('operation', 'operation-input-swap'), ('state-change', 'missing-write'), ('state-change', 'wrong-key'), ('state-change', 'cleanup'))],
            *[[sys.executable, 'examples/cinema/scripts/application-acceptance.py', '--cli', str(ROOT / 'target/debug/dctx0'), '--form', form, '--probe', probe, '--output', str(artifact_root() / f'application-validation/typescript-probe-{probe}')] for form, probe in (('constructor', 'catch-contract-failure'), ('constructor', 'codec-swap'), ('operation', 'operation-codec-swap'), ('constructor', 'post-verdict-assertion'), ('operation', 'wrong-return'), ('assertion', 'tie-break'), ('state-change', 'missing-write'), ('state-change', 'extra-write'), ('state-change', 'wrong-key'), ('state-change', 'cleanup'), ('state-change', 'state-scope'), ('state-change', 'missing-state-observer'))],
            *[[sys.executable, 'scripts/application_test_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0'), '--form', 'all', '--probe', probe, '--report', str(artifact_root() / f'application-validation/go-all-probe-{probe}/result.json')] for probe in ('tie-break','wrong-return','operation-input-swap','missing-write','wrong-key','cleanup','unexpected-collaborator')],
            *[[sys.executable, 'examples/cinema/scripts/application-acceptance.py', '--cli', str(ROOT / 'target/debug/dctx0'), '--form', 'all', '--probe', probe, '--output', str(artifact_root() / f'application-validation/typescript-all-probe-{probe}')] for probe in ('missing-write','extra-write','codec-swap','operation-codec-swap','tie-break','post-verdict-assertion','catch-contract-failure','cleanup','state-scope','missing-state-observer','wrong-key','falsy-native-throw','wrong-business-failure','technical-exception','missing-calls','empty-calls','unmodeled-resource','unexpected-mock-port')],
            [sys.executable, 'scripts/go_cinema_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0')],
            ['cargo', 'fetch', '--manifest-path', 'examples/cinema-rust/.dctx0/testing/rust/Cargo.toml', '--locked'],
            ['cargo','test','--no-fail-fast','--manifest-path','examples/cinema-rust/Cargo.toml','--locked','--offline','--','--skip','verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'],
            ['cargo','clippy','--manifest-path','examples/cinema-rust/Cargo.toml','--all-targets','--locked','--offline','--','-D','warnings'],
            [sys.executable,'scripts/rust_cinema_acceptance.py','--binary',str(ROOT / 'target/debug/dctx0')],
            ['cargo','test','--no-fail-fast','--manifest-path','examples/cinema-rust/Cargo.toml','--all-features','--offline','--locked','--','--skip','verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'],
            ['cargo','test','--no-fail-fast','--manifest-path','examples/cinema-rust/.dctx0/testing/rust/Cargo.toml','--lib','--offline','--locked','--','--skip','verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'],
        ])
    if args.focus == 'workspace':
        commands.extend([
            ['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'],
            [sys.executable, 'scripts/generator_acceptance.py', '--skip-build', '--scaffolds-only'],
            [sys.executable, 'scripts/status_acceptance.py', '--skip-build', '--skip-tests'],
        ])
    if args.tutorial:
        commands.extend([
            ['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'],
            ['bun', 'examples/tutorial/verify-series.ts'],
            ['bun', 'examples/tutorial/mutation-acceptance.ts'],
            ['bun', 'examples/tutorial/verify-variants.ts', '--skip-performance'],
        ])
    if args.focus not in ('conformance-reviews', 'workflow-tools', 'bindings', 'experimental-coverage', 'incremental'):
        commands.append([sys.executable, 'scripts/editor_acceptance.py', '--binary', str(ROOT / 'target/debug/dctx0'), '--functional-only'])
    if args.coverage:
        commands.append([sys.executable, 'scripts/coverage.py'] + (['--skip-performance'] if args.skip_performance else []))
    passed = True
    validation_steps = []
    # Native evidence and instrumented verifier images can temporarily exceed
    # a small RAM-backed /tmp. Use an owned on-disk cache for functional stages;
    # leave the parent's environment unchanged for established release workloads.
    if artifact_directory is not None:
        temporary_context = contextlib.nullcontext(retained_functional_cache)
    else:
        temporary_context = tempfile.TemporaryDirectory(prefix='v-', dir=functional_cache_root())
    with temporary_context as temporary:
        functional_environment = {**os.environ, 'TMPDIR': temporary}
        # Fresh installs materialize dependency inputs that the provenance
        # inventory deliberately hashes. Prepare them before pinning that
        # inventory, then retain the same source-change guard for every check.
        def prepares_inputs(command):
            return command[:2] in (['bun', 'install'], ['cargo', 'fetch'])
        preparation = [command for command in commands if prepares_inputs(command)]
        commands = [command for command in commands if not prepares_inputs(command)]
        for command in preparation:
            print('+ ' + ' '.join(command), flush=True)
            returncode = subprocess.run(command, cwd=ROOT, env=functional_environment).returncode
            validation_steps.append(validation_step(command, returncode))
            passed = passed and returncode == 0
        if cache_identity is not None:
            try:
                cache_identity = validation_cache.identity(ROOT, cache_options(args))
            except (OSError, ValueError, sqlite3.Error, subprocess.SubprocessError) as error:
                cache_identity = None
                print(f'Validation cache unavailable after dependency preparation: {error}', flush=True)
        validation_source_hash = digest(validation_sources())
        validation_started_ns = time.time_ns()
        for command in commands:
            print('+ ' + ' '.join(command), flush=True)
            returncode = subprocess.run(command, cwd=ROOT, env=functional_environment).returncode
            validation_steps.append(validation_step(command, returncode))
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
                  'validation_status': validation_status(validation_steps, validation_source_hash == validation_source_hash_after),
                  'validation_steps': validation_steps,
                  'validation_source_hash': validation_source_hash,
                  'validation_source_hash_after': validation_source_hash_after,
                  'performance_status': 'deferred',
                  'deferred_tests': ['verification_core_benchmarks_measure_both_compiled_examples_without_native_claims']}
        if artifact_directory is not None:
            report.update(artifact_root=str(artifact_directory), functional_cache=temporary, status='incomplete')
        destination = artifact_root() / 'validation-without-performance.json'
        destination.parent.mkdir(parents=True, exist_ok=True)
        atomic(destination, report)
        print(f'Performance measurements deferred; checks saved to {destination}', flush=True)
        return 0 if passed else 1
    # Benchmark conformance runs only after all functional/coverage stages.
    conformance = ['cargo', 'test', '--no-fail-fast', '-p', 'dctx0-cli', '--all-features', '--locked',
                   'verification_core_benchmarks_measure_both_compiled_examples_without_native_claims']
    conformance_code = 0
    if args.example == 'all' and args.focus != 'incremental':
        conformance_code = subprocess.run(conformance, cwd=ROOT).returncode
        validation_steps.append({'command': conformance, 'returncode': conformance_code})
    passed = passed and conformance_code == 0
    if args.tutorial:
        command = ['bun', 'examples/tutorial/verify-variants.ts', '--performance-only']
        returncode = subprocess.run(command, cwd=ROOT).returncode
        validation_steps.append({'command': command, 'returncode': returncode})
        passed = passed and returncode == 0
    record = benchmark(args)
    if artifact_directory is not None:
        record.update(artifact_root=str(artifact_directory), functional_cache=temporary)
    if args.focus != 'incremental':
        analysis_command = [sys.executable, 'scripts/test_analysis_acceptance.py', '--measure-only', '--samples', str(args.samples)]
        analysis_result = subprocess.run(analysis_command, cwd=ROOT, stdout=subprocess.PIPE, text=True)
        print(analysis_result.stdout, end='', flush=True)
        analysis_code = analysis_result.returncode
        try:
            analysis_report = read_analysis_report(analysis_result.stdout, record.get('source_hash'))
            record['test_analysis'] = analysis_report
            for name, metrics in analysis_report['measurements'].items():
                record['measurements']['test-analysis/' + name] = {
                    **metrics, 'iterations': 1, 'protocol_version': 1,
                    'memory_scope': analysis_report['memory_scope']}
        except (OSError, ValueError) as error:
            analysis_code = 1
            record['test_analysis'] = {'status': 'unavailable', 'error': str(error)}
        validation_steps.append({'command': analysis_command, 'returncode': analysis_code})
        passed = passed and analysis_code == 0
        query_command = [sys.executable, 'scripts/query_acceptance.py', '--measure-only', '--binary', str(ROOT / 'target/release/dctx0'), '--samples', str(args.samples)]
        query_code = subprocess.run(query_command, cwd=ROOT).returncode
        validation_steps.append({'command': query_command, 'returncode': query_code})
        passed = passed and query_code == 0
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
    record['validation_build_environment'] = {
        name: os.environ.get(name) for name in (
            'CARGO_PROFILE_DEV_DEBUG', 'CARGO_PROFILE_TEST_DEBUG', 'CARGO_INCREMENTAL',
            'CARGO_PROFILE_DEV_OPT_LEVEL', 'CARGO_PROFILE_TEST_OPT_LEVEL',
            'CARGO_PROFILE_DEV_DEBUG_ASSERTIONS', 'CARGO_PROFILE_TEST_DEBUG_ASSERTIONS',
            'CARGO_PROFILE_DEV_OVERFLOW_CHECKS', 'CARGO_PROFILE_TEST_OVERFLOW_CHECKS',
            'CARGO_BUILD_JOBS', 'RUST_TEST_THREADS')}
    record['validation_steps'] = validation_steps
    adaptive_reports = sorted((p for p in (ROOT / 'target/adaptive-conformance').glob('*.json') if p.stat().st_mtime_ns >= validation_started_ns), key=lambda p: p.stat().st_mtime_ns)
    record['adaptive_conformance'] = json.loads(adaptive_reports[-1].read_text()) if passed and adaptive_reports else {'status': 'unavailable', 'reason': 'No fresh successful adaptive report from this validation'}
    executor_reports = sorted((p for p in (ROOT / 'target/native-executor-conformance').glob('*.json') if p.stat().st_mtime_ns >= validation_started_ns), key=lambda p: p.stat().st_mtime_ns)
    record['native_executor_conformance'] = json.loads(executor_reports[-1].read_text()) if passed and executor_reports else {'status': 'unavailable'}
    native_reports = sorted((p for p in (ROOT / 'target/native-harness-conformance').glob('*.json') if p.stat().st_mtime_ns >= validation_started_ns), key=lambda p: p.stat().st_mtime_ns)
    record['native_transport_conformance'] = json.loads(native_reports[-1].read_text()) if passed and native_reports else {'status': 'unavailable', 'reason': 'No successful native transport conformance report from this validation'}
    record['validation_status'] = validation_status(validation_steps, validation_source_hash == validation_source_hash_after == record.get('source_hash'))
    if artifact_directory is not None:
        record['artifact_status'] = 'complete' if passed and record.get('status') == 'complete' else 'incomplete'
    gate = save(record, args)
    if artifact_directory is not None:
        atomic(artifact_directory / 'run.json', {'status': 'complete' if passed and gate else 'incomplete',
                                                  'artifact_root': str(artifact_directory), 'functional_cache': temporary})
    finish_validation_cache(args, cache_identity, record, passed, gate)
    return 0 if passed and gate else 1


if __name__ == '__main__':
    from memory_guard import guard_script
    guard_script()
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
