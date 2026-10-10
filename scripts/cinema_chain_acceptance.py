#!/usr/bin/env python3
"""Test exactly three Cinema tasks and a small pure-function composition."""
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

import validate

ROOT = Path(__file__).resolve().parents[1]


def main():
    output = ROOT / 'target/cinema-chain-acceptance'
    output.mkdir(parents=True, exist_ok=True)
    app = Path(tempfile.mkdtemp(prefix='app-', dir=output))
    example = ROOT / 'examples/cinema'
    for name in ('AGENTS.md', 'package.json', 'bun.lock', 'tsconfig.json'):
        shutil.copy2(example / name, app / name)
    shutil.copytree(example / 'dctx0', app / 'dctx0', ignore=shutil.ignore_patterns('.dctx0ctl'))
    shutil.copytree(example / 'src', app / 'src')
    binary = ROOT / 'target/release/dctx0'
    report = {'status': 'incomplete', 'scope': 'Cinema: FilmId, VotingRules contract, candidateOrderWinner only',
              'app': str(app), 'commands': [], 'checks': [],
              'memory_scope': '1 ms sampled process-tree sum of VmRSS; short spikes may be missed',
              'input_hashes': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                               for p in sorted((example / 'dctx0').glob('*.dctx0'))}}
    source_paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock',
                    *ROOT.glob('crates/**/*.rs'), *ROOT.glob('crates/**/Cargo.toml'),
                    *ROOT.glob('scripts/**/*.py'), *ROOT.glob('targets/**/*.rs'),
                    *ROOT.glob('targets/**/Cargo.toml'), *ROOT.glob('targets/**/*.md'),
                    *example.glob('src/**/*.ts')]
    report['source_hash'] = validate.digest(source_paths)

    def run(command, label, cwd=ROOT, success=True):
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        timer = time.perf_counter()
        peak = None
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            process = subprocess.Popen(list(map(str, command)), cwd=cwd, stdout=stdout, stderr=stderr)
            while process.poll() is None:
                pending, seen, rss = [process.pid], set(), 0
                while pending:
                    pid = pending.pop()
                    if pid in seen:
                        continue
                    seen.add(pid)
                    try:
                        for line in Path(f'/proc/{pid}/status').read_text().splitlines():
                            if line.startswith('VmRSS:'):
                                rss += int(line.split()[1]) * 1024
                        pending.extend(map(int, Path(f'/proc/{pid}/task/{pid}/children').read_text().split()))
                    except (FileNotFoundError, ProcessLookupError):
                        pass
                if rss:
                    peak = max(peak or 0, rss)
                time.sleep(0.001)
            stdout.seek(0)
            stderr.seek(0)
            result = subprocess.CompletedProcess(command, process.returncode, stdout.read().decode(), stderr.read().decode())
        entry = {'label': label, 'command': list(map(str, command)), 'cwd': str(cwd),
                 'returncode': result.returncode, 'started_at': started,
                 'finished_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 'elapsed_ms': (time.perf_counter() - timer) * 1000, 'sampled_peak_rss_bytes': peak}
        report['commands'].append(entry)
        (output / f'{label}.stdout').write_text(result.stdout)
        (output / f'{label}.stderr').write_text(result.stderr)
        if (result.returncode == 0) != success:
            raise RuntimeError(f'{label}: unexpected exit {result.returncode}: {result.stderr}')
        print(f'{label}: {entry["elapsed_ms"]:.1f} ms, {peak} bytes sampled RSS', flush=True)
        return result, entry

    def task(action, label, identifier=None, options=(), success=True):
        command = [binary, 'tasks', action, app / 'dctx0']
        if identifier is not None:
            command.append(f'T{identifier}')
        result, _ = run([*command, *options, '--json'], label, success=success)
        return json.loads(result.stdout)['body']['data'] if success else result

    def check(label, condition):
        if not condition:
            raise AssertionError(label)
        report['checks'].append(label)

    try:
        run(['cargo', 'build', '--release', '--workspace', '--all-features', '--locked'], 'release-build')
        run([binary, 'profile', 'typescript', 'create', '--project', app], 'profiles')
        run([binary, 'check', app / 'dctx0'], 'check')
        plan = task('plan', 'plan')
        keys = {m['candidate']['key']: m['task_id'] for m in plan['members']}
        subject = 'HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner'
        identifiers = [keys['contract:HomeCinema.MovieNight.FilmId'], keys['contract:' + subject],
                       keys['implementation:' + subject]]
        report['task_ids'] = identifiers
        initial = task('show', 'initial-operation', identifiers[2])
        check('Operation initially names both undelivered prerequisites', not initial['overview']['ready']
              and len(initial['overview']['blockers']) == 2)
        task('start', 'reject-premature-start', identifiers[2], ['--actor', 'cinema-chain'], success=False)
        closure = task('slice', 'dependency-closure', identifiers[2])
        report['slice'] = closure
        check('Dependency closure contains exactly the three selected tasks',
              {item['task_id'] for item in closure['slice']['tasks']} == set(identifiers))
        run(['bun', 'install', '--frozen-lockfile'], 'native-install', cwd=app)
        artifacts = ['film-id.ts', 'voting-rules.ts', 'candidate-order-winner.ts']
        decisions = [
            'FilmId is canonical UUID text; existing primitive string representation, no UUID dependency.',
            'VotingRules uses readonly FilmId arrays and undefined for Option.None; it publishes only the contract.',
            'Count eligible votes, choose first candidate on a tie. Unspecified boundaries: no eligible votes yields None; unknown votes ignored. Member/poll rules remain outside this pure operation.',
        ]
        for index, (identifier, artifact, decision) in enumerate(zip(identifiers, artifacts, decisions)):
            shown = task('show', f'before-{identifier}', identifier)
            check(f'T{identifier} becomes ready after actual prerequisite delivery', shown['overview']['ready'])
            context, _ = run([binary, 'tasks', 'show', app / 'dctx0', f'T{identifier}'], f'context-{identifier}')
            if index == 2:
                check('Consumer receives delivered producer artifact paths', all(path in context.stdout
                      for path in ('src/domain/film-id.ts', 'src/domain/voting-rules.ts')))
                check('Operation context includes the modeled tie/reversal invariant',
                      'reverse(votes)' in context.stdout and 'CandidateOrderTieBreak' in context.stdout)
            receipt = task('start', f'start-{identifier}', identifier, ['--actor', 'cinema-chain'])
            if index == 0:
                result, measured = run(['bun', 'test', 'src/domain/voting-rules.test.ts',
                                        '--test-name-pattern', 'FilmId'], 'native-film-id', cwd=app)
            elif index == 1:
                result, measured = run(['bun', 'run', 'typecheck'], 'native-contract-typecheck', cwd=app)
            else:
                result, measured = run(['bun', 'test', 'src/domain/voting-rules.test.ts'], 'native-winner', cwd=app)
                path = app / 'src/domain/candidate-order-winner.ts'
                original = path.read_text()
                path.write_text(original.replace('count > highest', 'count >= highest'))
                run(['bun', 'test', 'src/domain/voting-rules.test.ts'], 'broken-tie-rule', cwd=app, success=False)
                path.write_text(original)
                check('Native tests reject choosing the last candidate on a tie', True)
            implementation = hashlib.sha256((app / 'src/domain' / artifact).read_bytes()).hexdigest()
            delivered = {'summary': f'Delivered {artifact}', 'artifacts': [f'src/domain/{artifact}'],
                         'decisions': [decision], 'implementation': implementation}
            result_file = output / f'result-{identifier}.json'
            result_file.write_text(json.dumps(delivered, indent=2) + '\n')
            completed = task('complete', f'complete-{identifier}', identifier,
                             ['--actor', 'cinema-chain', '--claim', str(receipt['state']['claim']),
                              '--version', str(receipt['state']['version']), '--file', result_file])
            check(f'T{identifier} completion alone remains Unverified', completed['state']['check'] == 'unverified')
            evidence = {'requirements_hash': shown['definition']['candidate']['requirements_hash'],
                        'implementation': implementation, 'verifier': 'TypeScript' if index == 1 else 'bun:test',
                        'verifier_version': json.loads((app / 'node_modules/typescript/package.json').read_text())['version']
                        if index == 1 else subprocess.check_output(['bun', '--version'], text=True).strip(),
                        'command': ' '.join(map(str, measured['command'])), 'started_at': measured['started_at'],
                        'finished_at': measured['finished_at'], 'report': result.stdout + result.stderr, 'status': 'pass'}
            evidence_file = output / f'check-{identifier}.json'
            evidence_file.write_text(json.dumps(evidence, indent=2) + '\n')
            task('record-check', f'pass-{identifier}', identifier, ['--file', evidence_file])
            if index == 0:
                between = task('show', 'after-type-delivery', identifiers[2])
                check('Type delivery removes exactly one operation blocker', len(between['overview']['blockers']) == 1)
        final = task('show', 'final-operation', identifiers[2])
        check('Only the small operation reaches Done/PASS', final['state']['work'] == 'done' and final['state']['check'] == 'pass')
        listed = task('list', 'final-list', options=['--limit', '250'])
        check('Exactly three tasks complete; integration and every other task remain Open',
              {item['task_id'] for item in listed['items'] if item['work'] == 'done'} == set(identifiers)
              and all(item['work'] == 'open' for item in listed['items'] if item['task_id'] not in identifiers))
        replanned = task('plan', 'replan')
        check('Replanning retains task IDs', {m['candidate']['key']: m['task_id'] for m in replanned['members']} == keys)
        report['status'] = 'pass'
    except Exception as error:
        report['status'] = 'fail'
        report['error'] = str(error)
        raise
    finally:
        # Pair this bounded test with only three representative Cinema release phases.
        samples = {}
        for phase in ('compile', 'tasks-context-json', 'tasks-claim'):
            with tempfile.TemporaryDirectory(prefix='cinema-chain-bench-') as temporary:
                database = Path(temporary) / 'state.sqlite3'
                validate.measure(binary, 'examples/cinema/dctx0', 3, phase, database)
                samples[phase] = [validate.measure(binary, 'examples/cinema/dctx0', 3, phase, database)
                                  for _ in range(3)]
        report['release_samples'] = samples
        report['source_hash_after'] = validate.digest(source_paths)
        if report['source_hash_after'] != report['source_hash']:
            report['status'] = 'fail'
            report['error'] = 'Source contents changed during acceptance'
        report['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        report['toolchain'] = subprocess.check_output(['rustc', '--version'], text=True).strip()
        report['release_memory_scope'] = 'Linux post-exec VmHWM; preparation, IR and SQLite included'
        with validate.locked():
            filename = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-cinema-chain.json'
            validate.atomic(ROOT / 'benchmarks/history' / filename,
                            {'schema_version': 2, 'status': 'complete', 'validation_status': report['status'],
                             'timestamp': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                             'performance_gate': 'unassessed', 'measurements': samples,
                             'provenance': report})
            for old in sorted((ROOT / 'benchmarks/history').glob('*.json'))[:-20]:
                old.unlink()
        report['benchmark_history'] = f'benchmarks/history/{filename}'
        validate.atomic(output / 'report.json', report)
        print(f'Report: {output / "report.json"}', flush=True)
        if report['status'] != 'pass':
            raise RuntimeError(report.get('error', 'Acceptance incomplete'))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
