"""Focused release measurements for incremental verification acceptance paths."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import tempfile

from release_support import measured

MEMORY_SCOPE = ('Linux sampled process-tree RSS sum, 50 ms, including test runner and native '
                'compilers/harnesses; shared pages counted per process')
WORKLOADS = (
    ('legacy-native', 'incremental_native', None),
    ('hybrid-native', 'incremental_hybrid', None),
    ('application-native', 'hybrid_done', 'application_incremental'),
    ('cli', 'verification_incremental', None),
    ('verification-core', 'dctx0_verification', 'incremental'),
)
BUILDS = (
    ('engine', ['cargo', 'test', '--release', '--locked', '--all-features', '-p', 'dctx0-engine', '--test', 'incremental_native', '--test', 'incremental_hybrid', '--test', 'hybrid_done', '--no-run', '--message-format=json']),
    ('cli', ['cargo', 'test', '--release', '--locked', '--all-features', '-p', 'dctx0-cli', '--test', 'verification_incremental', '--no-run', '--message-format=json']),
    ('core', ['cargo', 'test', '--release', '--locked', '--all-features', '-p', 'dctx0-verification', '--lib', '--no-run', '--message-format=json']),
)
INPUTS = ('crates/dctx0-engine/tests/incremental_native.rs',
          'crates/dctx0-engine/tests/incremental_hybrid.rs',
          'crates/dctx0-engine/tests/hybrid_done.rs',
          'crates/dctx0-cli/tests/verification_incremental.rs',
          'crates/dctx0-verification/src/incremental.rs')


def artifacts(output, expected):
    """Select exactly one cargo-built test executable for each requested target."""
    found = {}
    for line in output.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if item.get('reason') != 'compiler-artifact' or not item.get('profile', {}).get('test'):
            continue
        name = item.get('target', {}).get('name')
        executable = item.get('executable')
        if name not in expected or not executable:
            continue
        if name in found and found[name] != executable:
            raise ValueError(f'Ambiguous release test executable: {name}')
        found[name] = executable
    missing = set(expected) - found.keys()
    if missing:
        raise ValueError(f'Missing release test executables: {sorted(missing)}')
    return found


def test_command(executable, test_filter):
    return [str(executable), *([test_filter] if test_filter else []), '--test-threads=1']


def guard_metrics(output):
    matches = re.findall(r'Memory guard: elapsed_ms=([0-9.]+); peak_process_tree_rss_bytes=([0-9]+);', output)
    if len(matches) != 1:
        raise ValueError('Missing or ambiguous process-tree memory measurement')
    elapsed, rss = float(matches[0][0]), int(matches[0][1])
    if elapsed <= 0 or rss <= 0:
        raise ValueError('Invalid process-tree timing or RSS')
    return {'elapsed_ms': elapsed, 'peak_rss_bytes': rss}


def run(root, directory, name, command, timeout, require_tests=False):
    env = os.environ.copy()
    env['DCTX0_MEMORY_GUARD'] = '1'
    result, output = measured([sys.executable, str(root / 'scripts/memory_guard.py'), *map(str, command)], root, timeout=timeout, env=env)
    (directory / f'{name}.log').write_text(output)
    if result['exit_code'] != 0:
        raise RuntimeError(f'{name} failed with exit code {result["exit_code"]}; see {directory / (name + ".log")}')
    sample = guard_metrics(output)
    if require_tests:
        match = re.search(r'test result: ok\. ([0-9]+) passed;', output)
        if not match or int(match.group(1)) == 0:
            raise ValueError(f'{name} executed no successful tests')
        sample['passed_tests'] = int(match.group(1))
    sample.update(exit_code=0, memory_scope=MEMORY_SCOPE)
    (directory / f'{name}.json').write_text(json.dumps(sample, indent=2) + '\n')
    return sample, output


def capture(root, command):
    result = subprocess.run(command, cwd=root, capture_output=True, text=True, check=False)
    return result.stdout.strip() if result.returncode == 0 else None


def benchmark(root, args, source_hash):
    root = Path(root)
    destination = root / 'target/incremental-performance'
    destination.mkdir(parents=True, exist_ok=True, mode=0o700)
    directory = Path(tempfile.mkdtemp(prefix='run-', dir=destination))
    record = {'schema_version': 1, 'timestamp': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'status': 'incomplete', 'measurements': {}, 'builds': {},
              'settings': {'iterations': 1, 'samples': args.samples, 'warmup_runs': 1, 'profile': 'release',
                           'features': 'all', 'cache': 'warm filesystem and native toolchain caches; fresh process and isolated fixture per sample'},
              'toolchain': capture(root, ['rustc', '--version']), 'platform': platform.platform(),
              'hardware': {'machine': platform.machine(), 'cpu_count': os.cpu_count(),
                           'cpu': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown') if Path('/proc/cpuinfo').is_file() else 'unknown'},
              'revision': capture(root, ['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': capture(root, ['git', 'status', '--porcelain']), 'source_hash': source_hash,
              'input_hashes': {}, 'memory_scope': MEMORY_SCOPE, 'artifact_directory': str(directory),
              'baseline_scope': 'focused incremental workloads; no accepted baseline comparison'}
    try:
        for name in INPUTS:
            record['input_hashes'][name] = hashlib.sha256((root / name).read_bytes()).hexdigest()
        expected = {target for _, target, _ in WORKLOADS}
        executables = {}
        for name, command in BUILDS:
            sample, output = run(root, directory, f'build-{name}', command, 1800)
            record['builds'][name] = sample
            # Cargo may report unrelated artifacts, so require only this command's named targets.
            wanted = {'engine': {'incremental_native', 'incremental_hybrid', 'hybrid_done'},
                      'cli': {'verification_incremental'}, 'core': {'dctx0_verification'}}[name]
            executables.update(artifacts(output, wanted))
        if executables.keys() != expected:
            raise ValueError('Built test executables do not match workloads')
        for name, target, test_filter in WORKLOADS:
            command = test_command(executables[target], test_filter)
            warmup, _ = run(root, directory, f'{name}-warmup', command, 1800, True)
            samples = [run(root, directory, f'{name}-sample-{i + 1}', command, 1800, True)[0] for i in range(args.samples)]
            if any(sample['passed_tests'] != warmup['passed_tests'] for sample in samples):
                raise ValueError(f'{name} test count changed between runs')
            record['measurements'][name] = {
                'iterations': 1, 'protocol_version': 1, 'samples': samples, 'warmup': warmup,
                'elapsed_ms': statistics.median(sample['elapsed_ms'] for sample in samples),
                'peak_rss_bytes': statistics.median(sample['peak_rss_bytes'] for sample in samples),
                'memory_scope': MEMORY_SCOPE,
                'workload_settings': {'target': target, 'filter': test_filter, 'test_threads': 1,
                                      'cache': record['settings']['cache'], 'passed_tests': warmup['passed_tests']}}
        # Validation's source inventory includes these test files and benchmark scripts.
        import validate
        if root.resolve() != validate.ROOT:
            raise ValueError('Focused benchmark must run in its validation workspace')
        record['source_hash_after'] = validate.digest(validate.validation_sources())
        if record['source_hash_after'] != source_hash:
            raise ValueError('Benchmark sources changed during measurement')
        record['status'] = 'complete'
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        record['error'] = str(error)
    (directory / 'record.json').write_text(json.dumps(record, indent=2) + '\n')
    return record


if __name__ == '__main__':
    import validate
    parser = argparse.ArgumentParser()
    parser.add_argument('--samples', type=int, default=1)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error('--samples must be positive')
    result = benchmark(validate.ROOT, args, validate.digest(validate.validation_sources()))
    print(json.dumps(result, indent=2))
    sys.exit(result['status'] != 'complete')
