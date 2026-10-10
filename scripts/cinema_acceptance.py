#!/usr/bin/env python3
"""Exercise Cinema as an LLM consumer, with native tests and release speed/RAM."""
import argparse
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/cinema-acceptance')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    app = Path(tempfile.mkdtemp(prefix='app-', dir=output))
    example = ROOT / 'examples/cinema'
    for name in ('AGENTS.md', 'package.json', 'bun.lock', 'tsconfig.json'):
        shutil.copy2(example / name, app / name)
    shutil.copytree(example / 'dctx0', app / 'dctx0', ignore=shutil.ignore_patterns('.dctx0ctl'))
    shutil.copytree(example / 'src', app / 'src')
    binary = ROOT / 'target/release/dctx0'
    commands = []
    checks = []

    def run(command, label, cwd=ROOT, success=True):
        index = len(commands)
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        timer = time.perf_counter()
        sampled_peak = None
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            process = subprocess.Popen(list(map(str, command)), cwd=cwd, stdout=stdout, stderr=stderr)
            while process.poll() is None:
                pending = [process.pid]
                seen = set()
                resident = 0
                observed = False
                while pending:
                    pid = pending.pop()
                    if pid in seen:
                        continue
                    seen.add(pid)
                    try:
                        status = Path(f'/proc/{pid}/status').read_text()
                        for line in status.splitlines():
                            if line.startswith('VmRSS:'):
                                resident += int(line.split()[1]) * 1024
                                observed = True
                        pending.extend(map(int, Path(f'/proc/{pid}/task/{pid}/children').read_text().split()))
                    except (FileNotFoundError, ProcessLookupError):
                        pass
                if observed:
                    sampled_peak = max(sampled_peak or 0, resident)
                time.sleep(0.001)
            stdout.seek(0)
            stderr.seek(0)
            result = subprocess.CompletedProcess(command, process.returncode,
                                                 stdout.read().decode(), stderr.read().decode())
        entry = {'label': label, 'command': list(map(str, command)), 'cwd': str(cwd),
                 'started_at': started,
                 'finished_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 'elapsed_ms': (time.perf_counter() - timer) * 1000,
                 'sampled_peak_rss_bytes': sampled_peak,
                 'returncode': result.returncode}
        commands.append(entry)
        (output / f'{index:03d}-{label}.stdout').write_text(result.stdout)
        (output / f'{index:03d}-{label}.stderr').write_text(result.stderr)
        if (result.returncode == 0) != success:
            raise RuntimeError(f'{label}: unexpected exit {result.returncode}: {result.stderr}')
        print(f'{label}: exit {result.returncode}, {entry["elapsed_ms"]:.1f} ms, '
              f'{sampled_peak} bytes sampled RSS', flush=True)
        return result, entry

    def cli(parts, label, success=True):
        result, entry = run([binary, *parts], label, success=success)
        return (json.loads(result.stdout) if success else result), entry

    def task(action, label, task_id=None, options=(), success=True):
        parts = ['tasks', action, str(app / 'dctx0')]
        if task_id is not None:
            parts.append(f'T{task_id}')
        response, entry = cli([*parts, *options, '--json'], label, success)
        return (response['body']['data'] if success else response), entry

    def passed(label, condition):
        if not condition:
            raise AssertionError(label)
        checks.append(label)

    report = {'schema_version': 1, 'status': 'incomplete', 'app': str(app),
              'scope': 'Cinema only; actual CLI and TypeScript execution',
              'commands': commands, 'checks': checks,
              'memory_scope': 'Linux sampled process-tree sum of live VmRSS, 1 ms polling; '
                              'sampled peak may miss short spikes, null means unavailable; native costs separate',
              'input_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                               for p in sorted((example / 'dctx0').glob('*.dctx0'))}}
    try:
        run(['cargo', 'build', '--release', '--workspace', '--all-features', '--locked'], 'release-build')
        original_agents = (app / 'AGENTS.md').read_text()
        cli(['profile', 'typescript', 'create', '--project', str(app), '--agents', '--dry-run', '--json'], 'create-dry-run')
        passed('Dry-run writes neither profiles nor AGENTS changes',
               not (app / '.dctx0').exists() and (app / 'AGENTS.md').read_text() == original_agents)
        cli(['profile', 'typescript', 'create', '--project', str(app), '--agents', '--json'], 'create')
        profile = app / '.dctx0/profiles/typescript/implement/value.md'
        profile.write_text(profile.read_text() + '\nCinema acceptance revision A: keep validated values as numbers.\n')
        revision_a = profile.read_text()
        agents_a = (app / 'AGENTS.md').read_text()
        cli(['profile', 'typescript', 'create', '--project', str(app), '--agents', '--json'], 'create-repeat')
        passed('Repeated creation preserves edits and exact existing AGENTS text',
               profile.read_text() == revision_a and (app / 'AGENTS.md').read_text() == agents_a
               and agents_a.startswith(original_agents))
        config = json.loads((app / '.dctx0/profiles/config.json').read_text())
        report['profile_config'] = config
        cli(['check', str(app / 'dctx0'), '--json'], 'check')
        plan, _ = task('plan', 'plan')
        members = plan['members']
        ids = {m['candidate']['key']: m['task_id'] for m in members}
        year = ids['contract:HomeCinema.MovieNight.ReleaseYear']
        runtime = ids['contract:HomeCinema.MovieNight.RuntimeMinutes']
        film = ids['contract:HomeCinema.MovieNight.Film']
        film_id = ids['contract:HomeCinema.MovieNight.FilmId']
        operation = ids['implementation:HomeCinema.MovieNight.Catalog.FilmCatalogApi.searchFilms']
        report['task_ids'] = {'release_year': year, 'runtime': runtime, 'film': film,
                              'film_id': film_id, 'search_films': operation}
        for identifier, label in [(year, 'release-year'), (film_id, 'film-id'),
                                   (film, 'entity'), (operation, 'operation')]:
            markdown, _ = run([binary, 'tasks', 'show', app / 'dctx0', f'T{identifier}'], label)
            if label == 'release-year':
                passed('Value context gives exact integer range and Implement profile',
                       '1888..2100' in markdown.stdout and 'implement/value.md' in markdown.stdout)
            elif label == 'entity':
                passed('Entity context includes full fields, key, type closure and Use roles',
                       all(text in markdown.stdout for text in
                           ('key id', 'genres: List<Genre>', 'runtimeMinutes: RuntimeMinutes',
                            'implement/entity.md', 'use/value.md', 'use/type.md', 'range 1888..2100')))
            elif label == 'operation':
                passed('Operation context includes constraints, examples and actual boundary roles',
                       all(text in markdown.stdout for text in
                           ('case-insensitive', 'logical AND', 'implement/operation.md',
                            'use/port.md', 'use/surface.md', 'use/example.md')))
        page, _ = task('context', 'profile-page', year,
                       ['--section', 'implementation', '--budget', '1'])
        cursor = page['page']['next_cursor']
        passed('Oversized profile is explicit and implementation context remains paginated',
               page['oversized_item'] and cursor is not None)
        cursor = json.dumps(cursor, separators=(',', ':'))
        continuation, _ = task('context', 'profile-page-next', year,
                               ['--section', 'implementation', '--budget', '1', '--cursor', cursor])
        passed('Continuation delivers the complete edited profile text',
               any('Cinema acceptance revision A' in item['text'] for item in continuation['page']['items']))
        profile.write_text(revision_a + '\nTemporary cursor test edit.\n')
        task('context', 'changed-cursor-rejected', year,
             ['--section', 'implementation', '--budget', '1', '--cursor', cursor], success=False)
        profile.write_text(revision_a)
        passed('Unbound cursor rejects edits to a used profile', True)
        before, _ = task('show', 'before-claim', year)
        receipt, _ = task('start', 'claim', year, ['--actor', 'cinema-llm'])
        frozen = receipt['state']['execution_bundle']['hash']
        profile.write_text(revision_a + '\nCinema acceptance revision B: provide a releaseYear boundary factory.\n')
        revision_b = profile.read_text()
        shown, _ = task('show', 'changed-after-claim', year)
        passed('Running claim retains revision A after profile edit',
               shown['state']['execution_bundle']['hash'] == frozen)
        profile.unlink()
        shown, _ = task('show', 'deleted-after-claim', year)
        passed('Running claim survives deletion of required local profile',
               shown['state']['execution_bundle']['hash'] == frozen)
        failure, _ = task('start', 'missing-blocks-new-claim', runtime,
                          ['--actor', 'cinema-other'], success=False)
        passed('New claim names the missing file', 'implement/value.md' in failure.stderr)
        missing, _ = task('show', 'missing-current-context', runtime)
        passed('Missing mandatory guidance stays visible and failed claim leaves work Open',
               any('implement/value.md' in path for path in missing['context']['implementation']['missing_files'])
               and missing['state']['work'] == 'open' and missing['state']['version'] == 0)
        profile.write_text(revision_b)
        task('release', 'release', year, ['--actor', 'cinema-llm', '--claim', str(receipt['state']['claim']),
                                        '--version', str(receipt['state']['version']), '--reason', 'Read current revision'])
        receipt, _ = task('start', 'restart', year, ['--actor', 'cinema-llm'])
        current = receipt['state']['execution_bundle']['hash']
        passed('New attempt receives current revision B', current != frozen)

        # Reservation freezes a separate ready task without starting it.
        reserved, _ = task('claim-slice', 'reserve-slice', runtime, ['--actor', 'cinema-slice'])
        reserved_show, _ = task('show', 'reserved-task', runtime)
        reserved_hash = reserved_show['state']['execution_bundle']['hash']
        passed('Slice reserves Open work without starting it', reserved_show['state']['work'] == 'open')
        profile.write_text(revision_b + '\nCinema acceptance revision C: preserve current constraints.\n')
        reservation = reserved_show['state']['reservation']
        started, _ = task('start', 'start-reserved', runtime,
                          ['--actor', 'cinema-slice', '--reservation', str(reservation)])
        passed('Starting reserved work uses its stored revision',
               started['state']['execution_bundle']['hash'] == reserved_hash)
        task('release', 'release-reserved', runtime,
             ['--actor', 'cinema-slice', '--claim', str(started['state']['claim']),
              '--version', str(started['state']['version']), '--reason', 'Acceptance finished'])
        profile.write_text(revision_b)

        run(['bun', 'install', '--frozen-lockfile'], 'native-install', cwd=app)
        run(['bun', 'run', 'typecheck'], 'native-typecheck', cwd=app)
        test_result, test_measurement = run(['bun', 'test', 'src/domain/release-year.test.ts'], 'native-tests', cwd=app)
        implementation_path = app / 'src/domain/release-year.ts'
        original_implementation = implementation_path.read_text()
        for name, old, new in [('upper-bound', 'input > 2100', 'input >= 2100'),
                               ('integer', '!Number.isInteger(input)', '!Number.isFinite(input)')]:
            implementation_path.write_text(original_implementation.replace(old, new))
            run(['bun', 'test', 'src/domain/release-year.test.ts'], f'mutation-{name}', cwd=app, success=False)
        implementation_path.write_text(original_implementation)
        passed('Native tests reject broken inclusive bound and broken integer validation', True)
        implementation = hashlib.sha256(implementation_path.read_bytes()).hexdigest()
        report['implementation_sha256'] = implementation
        completion, _ = task('complete', 'complete', year,
                             ['--actor', 'cinema-llm', '--claim', str(receipt['state']['claim']),
                              '--version', str(receipt['state']['version']),
                              '--summary', 'Publish ReleaseYear as a number with validated inclusive integer boundaries',
                              '--artifact', 'src/domain/release-year.ts', '--artifact', 'src/domain/release-year.test.ts',
                              '--implementation', implementation])
        passed('Manual completion remains Unverified', completion['state']['check'] == 'unverified')
        evidence = {'requirements_hash': before['definition']['candidate']['requirements_hash'],
                    'implementation': implementation, 'verifier': 'bun:test',
                    'verifier_version': subprocess.check_output(['bun', '--version'], text=True).strip(),
                    'command': 'bun test src/domain/release-year.test.ts',
                    'started_at': test_measurement['started_at'], 'finished_at': test_measurement['finished_at'],
                    'report': test_result.stderr + test_result.stdout, 'status': 'pass'}
        evidence_path = output / 'executed-check.json'
        evidence_path.write_text(json.dumps(evidence, indent=2) + '\n')
        task('record-check', 'record-executed-check', year, ['--file', str(evidence_path)])
        shown, _ = task('show', 'done-pass', year)
        passed('PASS follows actual native check import', shown['state']['check'] == 'pass')
        profile.unlink()
        shown, _ = task('show', 'deleted-after-completion', year)
        passed('Done/PASS retains its bundle after deletion',
               shown['state']['execution_bundle']['hash'] == current and shown['state']['check'] == 'pass')
        profile.write_text(revision_b + '\nCinema acceptance revision D: document boundary validator exports.\n')
        no_selection, _ = task('profile-changes', 'preview-no-selection')
        passed('Drift preview without selection requests no replacement', not no_selection['selected_tasks'])
        report['unselected_preview'] = no_selection
        selected, _ = task('profile-changes', 'preview-selected', options=['--task', f'T{year}'])
        passed('Explicit preview selects exactly the completed ReleaseYear task', selected['selected_tasks'] == [year])
        report['selected_preview'] = selected
        preview_file = output / 'selected-preview.json'
        preview_file.write_text(json.dumps(selected, indent=2) + '\n')
        update, _ = task('profile-update', 'apply-update', options=['--file', str(preview_file),
                                                                  '--reason', 'Document exported ReleaseYear boundary validator'])
        report['update'] = update
        repeated, _ = task('profile-update', 'repeat-update', options=['--file', str(preview_file),
                                                                     '--reason', 'Document exported ReleaseYear boundary validator'])
        passed('Repeated explicit update is idempotent', update == repeated)
        active, _ = task('plan', 'replan')
        old_definition = next(m for m in members if m['task_id'] == year)['candidate']
        replacement = next(m for m in active['members'] if m['candidate']['key'] == old_definition['key'])
        passed('Replanning preserves new adaptation ID and requirement hash',
               replacement['task_id'] != year and replacement['candidate']['requirements_hash'] == old_definition['requirements_hash'])
        new_show, _ = task('show', 'adaptation', replacement['task_id'])
        passed('Adaptation is Open/Unverified with typed reason', new_show['state']['work'] == 'open'
               and new_show['state']['check'] == 'unverified'
               and new_show['definition']['adjustment']['kind'] == 'profile')
        old_show, _ = task('show', 'old-history', year)
        passed('Historical implementation remains Done/PASS', old_show['state']['work'] == 'done'
               and old_show['state']['check'] == 'pass')
        unchanged = {m['candidate']['key']: m['task_id'] for m in active['members']}
        passed('Other implementation task IDs are unchanged',
               all(unchanged[key] == identifier for key, identifier in ids.items()
                   if key != old_definition['key'] and not key.startswith(('integration:', 'review:'))))
        assembly_keys = {m['candidate']['key'] for m in members
                         if m['task_id'] in selected['revised_assemblies']}
        passed('Exactly previewed integration/review IDs change; other assemblies remain intact',
               all((unchanged[key] != identifier) == (key in assembly_keys)
                   for key, identifier in ids.items() if key.startswith(('integration:', 'review:'))))
        report['revised_assembly_keys'] = sorted(assembly_keys)
        report['status'] = 'pass'
        report['replacement_task_id'] = replacement['task_id']
    except Exception as error:
        report['status'] = 'fail'
        report['error'] = str(error)
        raise
    finally:
        # The mandatory paired runner is explicitly restricted to Cinema.
        validate.WORKLOADS = {'cinema': 'examples/cinema/dctx0'}
        bench_args = argparse.Namespace(iterations=3, samples=3, speed_tolerance=20,
                                        ram_tolerance=10, accept_baseline=None)
        measured = validate.benchmark(bench_args)
        measured['validation_status'] = report['status']
        measured['validation_command'] = 'python3 scripts/cinema_acceptance.py'
        validate.save(measured, bench_args)
        report['release_measurements'] = measured
        report_path = output / 'report.json'
        report_path.write_text(json.dumps(report, indent=2) + '\n')
        print(f'Acceptance report: {report_path}', flush=True)
        if measured['status'] != 'complete':
            raise RuntimeError('Paired release measurements are incomplete')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
