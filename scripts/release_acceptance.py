#!/usr/bin/env python3
"""Couple release/installer checks with release packaging and CLI speed/RSS."""
import argparse
from datetime import datetime, timezone
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

from release_support import measured, locked
from memory_guard import guarded_run, systemd_available
from release import ROOT, TARGETS, workspace_version, native_target, package

HISTORY = ROOT / 'benchmarks/history/release'


def run(command, label, record, env=None):
    started = time.perf_counter()
    try:
        sample, output = measured(command, cwd=ROOT, timeout=300, env={**os.environ, **(env or {})})
    except (OSError, TimeoutError) as error:
        record['measurements'].setdefault(label, []).append({
            'elapsed_ms': (time.perf_counter() - started) * 1000,
            'peak_rss_bytes': None, 'exit_code': None,
            'memory_status': 'unavailable: command did not complete', 'failure': str(error)})
        raise RuntimeError(f'{label} failed: {error}') from error
    record['measurements'].setdefault(label, []).append(sample)
    if sample['exit_code']:
        raise RuntimeError(f'{label} failed:\n{output[-12000:]}')
    return output


def save(record):
    HISTORY.mkdir(parents=True, exist_ok=True)
    with locked(HISTORY.parent / '.release.lock'):
        with tempfile.NamedTemporaryFile(mode='w', dir=HISTORY, prefix='.write-', delete=False) as staged:
            json.dump(record, staged, indent=2)
            staged.write('\n')
            staged.flush()
            os.fsync(staged.fileno())
        path = HISTORY / (record['id'] + '.json')
        os.replace(staged.name, path)
        for old in sorted(HISTORY.glob('*.json'))[:-20]:
            old.unlink()
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release' / ('dctx0.exe' if os.name == 'nt' else 'dctx0'))
    parser.add_argument('--target', choices=TARGETS)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--platform-tests', action='store_true', help='run actual standalone CLI/LSP, Bun transport and process cancellation checks')
    parser.add_argument('--cargo-tests', action='store_true', help='also run the CLI integration suite and formatting check')
    args = parser.parse_args()
    if args.samples < 1:
        parser.error('At least one sample is required')
    binary = args.binary.resolve()
    target = args.target or native_target()
    if not binary.is_file():
        parser.error('Build the release CLI first')
    digest = hashlib.sha256()
    inputs = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'install.sh', ROOT / 'install.ps1', ROOT / 'scripts/release_platform.ts', ROOT / '.github/workflows/release.yml']
    inputs += [ROOT / 'scripts' / name for name in ('release.py', 'release_acceptance.py', 'release_support.py', 'memory_guard.py')]
    inputs += sorted(path for path in (ROOT / 'scripts/release_tests').glob('*') if path.is_file())
    for path in inputs:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(path.read_bytes())
    now = datetime.now(timezone.utc)
    record = {'schema_version': 1, 'id': now.strftime('%Y%m%dT%H%M%S.%fZ') + f'-{os.getpid()}',
              'timestamp': now.isoformat(), 'workload': 'release-distribution', 'status': 'incomplete',
              'metadata': {'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                           'dirty_worktree': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip()),
                           'content_hash': digest.hexdigest(), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                           'target': target, 'platform': platform.platform(), 'machine': platform.machine(),
                           'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
                           'cpu': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown') if Path('/proc/cpuinfo').exists() else 'unknown',
                           'memory_guard': {'active': os.environ.get('DCTX0_MEMORY_GUARDED') == '1', 'budget_bytes': int(os.environ['DCTX0_MEMORY_BUDGET_BYTES']) if os.environ.get('DCTX0_MEMORY_BUDGET_BYTES') else None}},
              'features': ['typescript', 'java', 'go', 'rust'], 'seed': None,
              'cache': 'prebuilt uninstrumented release CLI, one warm-up per phase, fresh process per sample, warm filesystem cache',
              'memory_scope': 'Linux/macOS wait4 peak child/reaped descendants; Windows polled child PeakWorkingSetSize; unsupported OS queries are null. See per-command scope; not collective simultaneous RSS',
              'baseline_comparison': {'status': 'unassessed', 'reason': 'No accepted distribution baseline; existing core baseline unchanged'},
              'measurements': {}}
    failure = None
    try:
        if os.name != 'nt':
            run(['bash', '-n', str(ROOT / 'install.sh')], 'shell-syntax', record)
        if args.cargo_tests:
            run(['cargo', 'fmt', '--all', '--', '--check'], 'format', record)
            run(['cargo', 'test', '-p', 'dctx0-cli', '--test', 'cli', '--all-features', '--locked'], 'cli-integration', record)
        version = run([str(binary), '--version'], 'binary-version', record).strip()
        if version != 'dctx0 ' + workspace_version():
            raise RuntimeError('Release binary does not match the workspace version')
        output = run([sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/release_tests', '-v'],
                     'installer-and-release-tests', record, {'DCTX0_RELEASE_BINARY': str(binary)})
        if os.name == 'nt':
            with tempfile.TemporaryDirectory(prefix='windows-install-', dir=ROOT / 'target') as assets:
                package(binary, target, 'v' + workspace_version(), Path(assets))
                native_output = run(['pwsh', '-NoProfile', '-File', str(ROOT / 'scripts/release_tests/test_windows_install.ps1'),
                                     '-Binary', str(binary), '-Assets', assets, '-Tag', 'v' + workspace_version()],
                                    'powershell-installer', record)
                record['windows_installer'] = native_output
        if args.platform_tests:
            run(['bun', str(ROOT / 'scripts/release_platform.ts'), str(binary)], 'platform-cli-lsp-harness-cancellation', record)
            record['platform_checks'] = json.loads((ROOT / 'target/release-platform.json').read_text())
        record['validation'] = {'status': 'pass', 'test_output': output, 'network_scope': 'deterministic curl fixtures; actual binary, archive, checksum, install and compiler execution'}
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        failure = str(error)
    try:
        with tempfile.TemporaryDirectory(prefix='release-measurements-', dir=ROOT / 'target') as directory:
            arguments = [sys.executable, str(ROOT / 'scripts/release.py'), 'package', '--binary', str(binary),
                         '--target', target, '--tag', 'v' + workspace_version(), '--output', directory]
            run(arguments, 'package-warmup', record)
            run([str(binary), '--version'], 'startup-warmup', record)
            for _ in range(args.samples):
                run(arguments, 'package-release', record)
                run([str(binary), '--version'], 'startup-release', record)
        record['summary'] = {}
        for phase in ('package-release', 'startup-release'):
            samples = record['measurements'][phase]
            memory = [sample['peak_rss_bytes'] for sample in samples if sample['peak_rss_bytes'] is not None]
            record['summary'][phase] = {'samples': len(samples), 'median_elapsed_ms': statistics.median(sample['elapsed_ms'] for sample in samples),
                                        'peak_rss_bytes': max(memory) if len(memory) == len(samples) else None}
        record['status'] = 'failed' if failure else 'complete'
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        failure = (failure + '\n' if failure else '') + str(error)
        record['status'] = 'failed'
    if failure:
        record['failure'] = failure
    result = save(record)
    print(json.dumps({'status': record['status'], 'report': str(result.relative_to(ROOT)), 'summary': record.get('summary'), 'failure': failure}, indent=2))
    return 1 if failure else 0


if __name__ == '__main__':
    if os.environ.get('DCTX0_MEMORY_GUARDED') != '1':
        if os.environ.get('DCTX0_MEMORY_GUARD') != '0' and systemd_available():
            raise SystemExit(guarded_run([sys.executable, *sys.argv]))
        print('Release memory guard disabled or unavailable; no collective RAM cap.', file=sys.stderr)
    ROOT.joinpath('target').mkdir(exist_ok=True)
    with locked(ROOT / 'target/release-acceptance.lock'):
        raise SystemExit(main())
