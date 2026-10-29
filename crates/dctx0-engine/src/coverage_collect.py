"""Experimental Linux Rust/libtest collector. Invoked by the dctx0 CLI.

Each child owns a process group; timeout/cancellation kills and reaps it.
Raw LLVM exports and merged profiles remain in the unique artifact directory.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import signal
import subprocess
import sys
import time
import tempfile
import datetime
import tomllib


def run(argv, cwd, env, timeout, allow_failure=False):
    started = time.monotonic()
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        with subprocess.Popen(argv, cwd=cwd, env=env, stdout=out,
                              stderr=err, start_new_session=True) as child:
            timed_out = False
            try:
                while True:
                    pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                    if pid:
                        child.returncode = os.waitstatus_to_exitcode(status)
                        break
                    if time.monotonic() - started >= timeout:
                        timed_out = True
                        break
                    if out.tell() + err.tell() > 256 * 1024 * 1024:
                        raise RuntimeError('Child output exceeded 256 MiB')
                    time.sleep(0.02)
            finally:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                if child.returncode is None:
                    _, status, usage = os.wait4(child.pid, 0)
                    child.returncode = os.waitstatus_to_exitcode(status)
        if out.tell() + err.tell() > 256 * 1024 * 1024:
            raise RuntimeError('Child output exceeded 256 MiB')
        out.seek(0); err.seek(0)
        stdout, stderr = out.read(), err.read()
    elapsed = (time.monotonic() - started) * 1000
    if not allow_failure and (timed_out or child.returncode):
        raise RuntimeError(f'{argv[0]} failed/timeout: {stderr.decode(errors="replace")[-4000:]}')
    return stdout, stderr, child.returncode, timed_out, elapsed, usage.ru_maxrss * 1024


def collect(options):
    if sys.platform != 'linux':
        raise RuntimeError('Experimental collection currently supports Linux only')
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    root = Path(options['root'])
    artifacts = Path(options['artifacts'])
    artifacts.mkdir(parents=True, exist_ok=options.get("resume", False))
    env = dict(os.environ)
    if options.get('research'):
        env['RUSTUP_AUTO_INSTALL'] = '0'
    env['CARGO_TARGET_DIR'] = str(artifacts / 'target')
    env['RUSTFLAGS'] = env.get('RUSTFLAGS', '') + ' -C instrument-coverage'
    if 'CARGO_ENCODED_RUSTFLAGS' in env:
        env['CARGO_ENCODED_RUSTFLAGS'] += '\x1f-C\x1finstrument-coverage'
    env['LLVM_PROFILE_FILE'] = str(artifacts / 'build-%p-%m.profraw')
    timeout = options['timeout_secs']
    def command(argv, **kwargs):
        return run(argv, root, env, timeout, **kwargs)
    sysroot = command(['rustc', '--print', 'sysroot'])[0].decode().strip()
    host = next(line.split(': ', 1)[1] for line in command(['rustc', '-vV'])[0].decode().splitlines() if line.startswith('host: '))
    llvm = Path(sysroot) / 'lib/rustlib' / host / 'bin'
    for tool in ('llvm-profdata', 'llvm-cov'):
        if not options.get('catalog_only') and not (llvm / tool).is_file():
            raise RuntimeError('Install rustup component llvm-tools-preview')
    git = run(['git', 'rev-parse', 'HEAD'], root, env, timeout, allow_failure=True)
    revision = git[0].decode().strip() if git[2] == 0 else None
    dirty = run(['git', 'status', '--porcelain'], root, env, timeout, allow_failure=True)
    dirty_worktree = bool(dirty[0]) if dirty[2] == 0 else None
    # Build-input fingerprint, not a claim of complete environmental dependency capture.
    metadata = json.loads(command(['cargo', 'metadata', '--format-version=1', '--no-deps', *(['--locked', '--offline'] if options.get('research') else [])])[0])
    def source_inventory():
        source_files = set()
        for package in metadata['packages']:
            folder = Path(package['manifest_path']).parent
            source_files.update(p for p in folder.rglob('*') if p.is_file() and
                                not {'target', '.git', '.dctx0ctl'}.intersection(p.relative_to(folder).parts)
                                and p.suffix in ('.rs', '.toml', '.lock', '.ts', '.go', '.md', '.json', '.py'))
        source_files.update(p for p in (Path(metadata['workspace_root']) / 'Cargo.toml', Path(metadata['workspace_root']) / 'Cargo.lock') if p.is_file())
        return source_files
    def fingerprint():
        digest = hashlib.sha256()
        for path in sorted(source_inventory()):
            digest.update(str(path).encode()); digest.update(b'\0'); digest.update(path.read_bytes())
        return digest.hexdigest()
    before = fingerprint()
    environment_hash = hashlib.sha256(json.dumps(sorted(env.items())).encode()).hexdigest()
    build = ['cargo', 'test', '--no-run', '--message-format=json', '--locked']
    if options.get('package'):
        build += ['-p', options['package']]
    else:
        build += ['--workspace']
    if options.get('offline'):
        build += ['--offline']
    if options.get('all_features'):
        build += ['--all-features']
    if options.get('no_default_features'):
        build += ['--no-default-features']
    if options.get('features'):
        build += ['--features', ','.join(options['features'])]
    output, logs, _, _, build_ms, build_rss = command(build)
    (artifacts / 'build.stderr').write_bytes(logs)
    binaries = {}
    for line in output.splitlines():
        entry = json.loads(line)
        if entry.get('reason') == 'compiler-artifact' and entry.get('profile', {}).get('test') and entry.get('executable'):
            binaries[entry['executable']] = entry
    provenance = {'schema_version': 1, 'project_root': str(root), 'repository_root': options.get('repository_root', str(root)), 'timeout_secs': timeout, 'revision': revision, 'dirty_worktree': dirty_worktree, 'environment_hash': environment_hash, 'source_fingerprint': before,
                  'fingerprint_scope': 'workspace package source/config files; external inputs not captured',
                  'rustc': command(['rustc', '-vV'])[0].decode(), 'platform': platform.platform(),
                  'rustflags': env['RUSTFLAGS'], 'encoded_rustflags': env.get('CARGO_ENCODED_RUSTFLAGS'), 'build_argv': build, 'phase_build_ms': build_ms, 'phase_build_peak_rss_bytes': build_rss,
                  'build_ms': build_ms + options.get('catalog_build_ms', 0), 'build_peak_rss_bytes': max(build_rss, options.get('catalog_build_peak_rss_bytes', 0)),
                  'ignored_tests': 'excluded using libtest --list --ignored; doctests/benchmarks/custom harnesses unsupported',
                  'coverage_scope': 'instrumented Rust objects in this test executable; child/native coverage not included',
                  'artifacts': str(artifacts), 'peak_rss_scope': 'Linux wait4 ru_maxrss for test process including waited descendants; concurrent process RSS is not summed'}
    previous = {}
    if options.get('resume'):
        for record_file in artifacts.glob('execution-*/execution.json'):
            record = json.loads(record_file.read_text())
            p = record['provenance']
            previous[(p['package_name'], p['test_target_kind'], p['test_target_name'], p['test_name'])] = str(record_file)
    records = []
    catalog = []
    packages = {p['id']: Path(p['manifest_path']).parent for p in metadata['packages']}
    package_names = {p['id']: p['name'] for p in metadata['packages']}
    unsupported = []
    supported_targets = []
    for package in metadata['packages']:
        if package['id'] not in metadata['workspace_members'] or (options.get('package') and package['name'] != options['package']):
            continue
        manifest = tomllib.loads(Path(package['manifest_path']).read_text())
        for target in package['targets']:
            kind = target['kind'][0]
            declarations = manifest.get('lib', {}) if kind in ('lib', 'proc-macro') else next((t for t in manifest.get(kind, []) if t.get('name') == target['name']), {})
            if isinstance(declarations, dict) and declarations.get('harness') is False:
                unsupported.append({'package': package['name'], 'kind': kind, 'name': target['name'], 'supported': False, 'reason': 'custom harness unsupported'})
            elif not target.get('test', False) or kind in ('bench', 'custom-build'):
                unsupported.append({'package': package['name'], 'kind': kind, 'name': target['name'], 'supported': False, 'reason': 'not a cargo test libtest target'})
    planned = []
    for binary, entry in sorted(binaries.items(), key=lambda item: (package_names[item[1]["package_id"]], item[1]["target"]["kind"][0], item[1]["target"]["name"])):
        manifest = tomllib.loads((packages[entry['package_id']] / 'Cargo.toml').read_text())
        kind = entry['target']['kind'][0]
        declarations = manifest.get('lib', {}) if kind in ('lib', 'proc-macro') else next((t for t in manifest.get(kind, []) if t.get('name') == entry['target']['name']), {})
        if isinstance(declarations, dict) and declarations.get('harness') is False:
            continue
        supported_targets.append({'package': package_names[entry['package_id']], 'kind': kind, 'name': entry['target']['name'], 'supported': True, 'reason': None})
        env['LLVM_PROFILE_FILE'] = str(artifacts / 'listing-%p-%m.profraw')
        listing = run([binary, '--list', '--format=terse'], packages[entry['package_id']], env, timeout)[0].decode()
        ignored_listing = run([binary, '--list', '--ignored', '--format=terse'], packages[entry['package_id']], env, timeout)[0].decode()
        ignored = {line[:-6] for line in ignored_listing.splitlines() if line.endswith(': test')}
        tests = sorted(line[:-6] for line in listing.splitlines() if line.endswith(': test'))
        for test in tests:
            test_id = f'{entry["package_id"]}::{entry["target"]["kind"][0]}::{entry["target"]["name"]}::{test}'
            filter_id = f'{package_names[entry["package_id"]]}::{entry["target"]["kind"][0]}::{entry["target"]["name"]}::{test}' if options.get('research') else test_id
            if options.get('filter') and options['filter'] not in filter_id:
                continue
            catalog.append({'package': package_names[entry['package_id']], 'target_kind': entry['target']['kind'][0],
                            'target_name': entry['target']['name'], 'test_name': test, 'ignored': test in ignored})
            if test in ignored or options.get('catalog_only'):
                continue
            planned.append((package_names[entry['package_id']], entry['target']['kind'][0], entry['target']['name'], test, binary, entry, test_id))
    inventory = {'catalog': catalog, 'supported_targets': supported_targets, 'unsupported': unsupported,
                 'enumeration_complete': True, 'provenance': provenance}
    inventory_partial = artifacts / 'catalog.partial'
    inventory_partial.write_text(json.dumps(inventory))
    inventory_partial.replace(artifacts / 'catalog.json')
    if options.get('expected_catalog_path'):
        expected = json.loads(Path(options['expected_catalog_path']).read_text())
        if expected['catalog'] != catalog or expected['toolchain'] != provenance['rustc']:
            raise RuntimeError('Catalog changed after publication; refusing test execution')
    if options.get('expected_toolchain') and options['expected_toolchain'] != provenance['rustc']:
        raise RuntimeError('Toolchain changed after catalog publication')
    for _, _, _, test, binary, entry, test_id in sorted(planned, key=lambda item: item[:4]):
        folder = artifacts / f'execution-{len(records):06d}'
        identity = (package_names[entry['package_id']], entry['target']['kind'][0], entry['target']['name'], test)
        if identity in previous:
            records.append(previous[identity])
            continue
        if folder.exists():
            folder = artifacts / f'execution-{len(records):06d}-{time.time_ns()}'
        folder.mkdir()
        env['LLVM_PROFILE_FILE'] = str(folder / '%p-%m.profraw')
        argv = [binary, '--exact', test, '--nocapture']
        started_at = datetime.datetime.now(datetime.timezone.utc).isoformat()
        started_at_unix_ms = time.time_ns() // 1_000_000
        stdout, stderr, code, expired, elapsed, rss = run(argv, packages[entry['package_id']], env, timeout, allow_failure=True)
        ended_at = datetime.datetime.now(datetime.timezone.utc).isoformat()
        ended_at_unix_ms = time.time_ns() // 1_000_000
        (folder / 'stdout').write_bytes(stdout); (folder / 'stderr').write_bytes(stderr)
        profiles = sorted(folder.glob('*.profraw'))
        merged = folder / 'coverage.profdata'
        try:
            if profiles:
                command([str(llvm / 'llvm-profdata'), 'merge', '-sparse', *map(str, profiles), '-o', str(merged)])
                exported = command([str(llvm / 'llvm-cov'), 'export', binary, f'-instr-profile={merged}'])[0]
            else:
                exported = json.dumps({'type': 'llvm.coverage.json.export', 'version': '2.0.1', 'data': []}).encode()
        except RuntimeError as error:
            if not options.get('research'):
                raise
            (folder / 'coverage-error').write_text(str(error))
            profiles = []
            exported = json.dumps({'type': 'llvm.coverage.json.export', 'version': '2.0.1', 'data': []}).encode()
        (folder / 'llvm.json').write_bytes(exported)
        record = {'test_id': test_id, 'outcome': 'timeout' if expired else 'pass' if code == 0 else 'fail',
                        'duration_ms': elapsed, 'peak_rss_bytes': rss,
                        'provenance': {**provenance, 'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                                       'execution_artifacts': str(folder), 'exit_code': code, 'argv': argv,
                                       'started_at': started_at, 'ended_at': ended_at,
                                       'started_at_unix_ms': started_at_unix_ms, 'ended_at_unix_ms': ended_at_unix_ms,
                                       'test_identity_version': 2, 'test_name': test, 'package_name': package_names[entry['package_id']],
                                       'test_target_name': entry['target']['name'], 'test_target_kind': entry['target']['kind'][0],
                                       'coverage_available': bool(profiles),
                                       'profile_base64': base64.b64encode(merged.read_bytes()).decode() if profiles else None},
                        'llvm_export': json.loads(exported)}
        record_file = folder / 'execution.json'
        temporary_record = folder / 'execution.partial'
        temporary_record.write_text(json.dumps(record))
        temporary_record.replace(record_file)
        records.append(str(record_file) if options.get('stream') else record)
    final_revision = run(['git', 'rev-parse', 'HEAD'], root, env, timeout, allow_failure=True)
    final_dirty = run(['git', 'status', '--porcelain'], root, env, timeout, allow_failure=True)
    if revision is not None and (final_revision[0].decode().strip() != revision or (dirty_worktree is False and bool(final_dirty[0]))):
        raise RuntimeError('Git revision/worktree changed during collection; refusing publication')
    if fingerprint() != before:
        raise RuntimeError('Source files changed during collection; refusing publication')
    if not records and not options.get('research'):
        raise RuntimeError('No matching Rust libtest tests found')
    report = {('execution_files' if options.get('stream') else 'executions'): records, 'collector_children_peak_rss_bytes': resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024,
            'measurement_scope': 'Linux cumulative maximum child RSS, includes build; not per-test RSS'}
    if options.get('research'):
        report.update({'catalog': catalog, 'metadata': metadata, 'provenance': provenance, 'unsupported': unsupported, 'supported_targets': supported_targets})
    report_file = artifacts / ('catalog-manifest.json' if options.get('catalog_only') else 'manifest.json')
    report_partial = report_file.with_suffix('.partial')
    report_partial.write_text(json.dumps(report))
    report_partial.replace(report_file)
    return report


if __name__ == '__main__':
    def interrupted(signum, frame):
        raise KeyboardInterrupt()
    signal.signal(signal.SIGTERM, interrupted)
    try:
        options = json.loads(sys.argv[1])
        print(json.dumps(coverage_diff(options) if options.get('mode') == 'diff' else collect(options)))
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
