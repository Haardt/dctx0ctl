#!/usr/bin/env python3
"""Go Cinema acceptance and actual production mutations; release measurements run separately."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'examples/cinema-go'
PREFIX = 'HomeCinema.MovieNight.'


def invoke(binary, project, *args, expect=None):
    command = [str(binary), '--json', *args]
    result = subprocess.run(command, cwd=ROOT, env={**os.environ, 'DCTX0_MEMORY_GUARD': '0'}, capture_output=True, text=True)
    if expect is not None and result.returncode != expect:
        raise RuntimeError(f'{command}: {result.stderr}\n{result.stdout[:2000]}')
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f'{command}: {result.stderr}\n{result.stdout[:2000]}') from error


def copy_application(destination):
    shutil.copytree(SOURCE, destination, ignore=shutil.ignore_patterns('.dctx0ctl', '.bin', 'cinema.sqlite3*'))


def native(binary, project, subject):
    return invoke(binary, project, 'verify', 'run', str(project / 'dctx0'), '--project', str(project), '--subject', PREFIX + subject, '--no-adopt')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/dctx0')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/go-cinema/acceptance.json')
    args = parser.parse_args()
    binary = args.binary.resolve()
    report = {'schema_version': 1, 'status': 'incomplete', 'scope': 'Complete unchanged Go Cinema; authored review kept separate; targeted production mutations and replay', 'mutations': []}
    with tempfile.TemporaryDirectory(prefix='dctx0-go-acceptance-') as temporary:
        base = Path(temporary)
        project = base / 'baseline'
        copy_application(project)
        complete = invoke(binary, project, 'verify', 'complete', str(project / 'dctx0'), '--project', str(project), expect=0)
        assert complete['result']['status'] == 'PASS'
        assert complete['mandatory_cases'] == 5674 and complete['tasks'] == 105
        assert complete['open_tasks'] == complete['non_pass_tasks'] == 0
        assert complete['adopted_tasks'] and complete['inspected_tasks'], 'Fresh acceptance must deliver real tasks'
        report['complete'] = complete
        specs = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (project / 'dctx0').glob('*.dctx0')}
        reference = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (ROOT / 'examples/cinema/dctx0').glob('*.dctx0')}
        assert specs == reference, 'Cinema spec differs between targets'
        report['spec_hashes'] = specs
        go_plan = invoke(binary, project, 'verify', 'plan', str(project / 'dctx0'), '--project', str(project))
        reference_project = base / 'typescript-reference'
        shutil.copytree(ROOT / 'examples/cinema', reference_project,
                        ignore=shutil.ignore_patterns('.dctx0ctl', 'node_modules', 'cinema.sqlite3*'))
        invoke(binary, reference_project, 'check', str(reference_project / 'dctx0'), expect=0)
        ts_plan = invoke(binary, reference_project, 'verify', 'plan', str(reference_project / 'dctx0'), '--project', str(reference_project))
        def semantic(value):
            if isinstance(value, dict):
                return {key: semantic(item) for key, item in value.items() if key != 'check_occurrence'}
            if isinstance(value, list):
                return [semantic(item) for item in value]
            return value
        assert semantic(go_plan) == semantic(ts_plan), 'Target-neutral Cinema plans differ'
        report['semantic_plan_equal'] = True
        mutations = [
            ('winner', 'domain/contracts.go', 'if count > best {', 'if count >= best {', 'ScreeningPlanning.VotingRules.candidateOrderWinner'),
            ('persistence', 'persistence/store.go', 'ON CONFLICT(id) DO UPDATE SET document=excluded.document', 'ON CONFLICT(id) DO NOTHING', 'Catalog.FilmCatalogStore.save'),
            ('lifecycle', 'persistence/store.go', 'poll["status"] = "Closed"', 'poll["status"] = "Open"', 'ScreeningPollLifecycle'),
            ('http', 'httpapi/handler.go', '"GET /films/{id}", "getFilm", id', '"GET /films/{id}", "getCurrentVoting", id', 'Catalog.FilmCatalogApi.getFilm'),
            ('collaborator-calls', 'application/service.go', 'result, err := s.ports.Invoke(port, args)', 'if operation == "castVote" { _, _ = s.ports.Invoke(port, args) }; result, err := s.ports.Invoke(port, args)', 'ScreeningPlanning.ScreeningPlanningApi.castVote'),
        ]
        for label, relative, before, after, subject in mutations:
            changed = base / label
            copy_application(changed)
            invoke(binary, changed, 'check', str(changed / 'dctx0'), expect=0)
            file = changed / relative
            original = file.read_text()
            if original.count(before) != 1:
                raise RuntimeError(f'Mutation anchor is not unique: {label}')
            file.write_text(original.replace(before, after))
            result = native(binary, changed, subject)
            run = result['evidence']['run']
            failures = [case for case in run['cases'] if case['result']['status'] == 'FAIL']
            assert failures, f'{label} produced no executed FAIL: {run["result"]}; {run["provenance"]["incomplete"]}'
            counter = failures[0]
            row = {'mutation': label, 'subject': PREFIX + subject, 'case_id': counter['id'], 'result': counter['result'], 'mandatory_cases': len(run['required_cases']), 'recorded_cases': len(run['cases'])}
            if label == 'winner':
                original_run = result['evidence']['id']
                replay = invoke(binary, changed, 'verify', 'replay', str(changed / 'dctx0'), original_run, '--case', counter['id'], '--project', str(changed))
                assert replay['case_result']['status'] == 'FAIL'
                file.write_text(original)
                repaired = invoke(binary, changed, 'verify', 'replay', str(changed / 'dctx0'), original_run, '--case', counter['id'], '--project', str(changed), expect=0)
                assert repaired['case_result']['status'] == 'PASS' and repaired['changes']
                row['replay'] = {'broken': replay['case_result'], 'repaired': repaired['case_result'], 'provenance_changes': len(repaired['changes'])}
            report['mutations'].append(row)
    report['status'] = 'PASS'
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Go Cinema PASS: 5674 cases, 105 tasks, {len(report["mutations"])} actual mutations and replay; {args.report}')


if __name__ == '__main__':
    main()
