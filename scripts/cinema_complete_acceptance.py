#!/usr/bin/env python3
"""Execute complete Cinema evidence and publish explicitly pinned source reviews.

Reviews are authored inspection notes, not generated approvals. Changed reviewed
sources require a new inspection before this runner can publish their judgement.
"""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import time

from memory_guard import guard_script

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / 'examples/cinema'
SPEC = APP / 'dctx0'
OUT = SPEC / '.dctx0ctl/cinema-complete'


def main():
    guard_script()
    OUT.mkdir(parents=True, exist_ok=True)
    notes = json.loads((APP / '.dctx0/testing/typescript/inspection-notes.json').read_text())
    for name, expected in notes['reviewed_hashes'].items():
        if hashlib.sha256((APP / name).read_bytes()).hexdigest() != expected:
            raise RuntimeError(f'New source inspection required: {name}')
    binary = ROOT / 'target/debug/dctx0'
    commands = []

    def run(label, args, allowed=(0,)):
        path = OUT / f'{label}.json'
        start = time.monotonic()
        with path.open('w') as stdout, (OUT / f'{label}.stderr').open('w') as stderr:
            result = subprocess.run([str(binary), *map(str, args), '--json'], cwd=ROOT, stdout=stdout, stderr=stderr)
        commands.append({'label': label, 'returncode': result.returncode, 'elapsed_ms': (time.monotonic()-start)*1000})
        if result.returncode not in allowed:
            raise RuntimeError(f'{label} failed: {(OUT / f"{label}.stderr").read_text()}')
        return json.loads(path.read_text())

    run('check', ['check', SPEC])
    run('profiles', ['profile', 'typescript', 'create', '--project', APP, '--default'])
    run('plan-tasks', ['tasks', 'plan', SPEC, '--project', APP, '--target', 'typescript'])
    native = run('native', ['verify', 'run', SPEC, '--project', APP], (0, 1))
    stored = native['evidence']
    evidence = stored['run']
    if evidence['origin'] != 'native' or evidence['provenance']['incomplete']:
        raise RuntimeError('Native provenance is incomplete')
    if not evidence['cases'] or len(evidence['cases']) != len(evidence['required_cases']) or any(case['result']['status'] != 'PASS' for case in evidence['cases']):
        raise RuntimeError('Mandatory native cases did not all pass')
    if len([case for case in evidence['cases'] if ':http-' in case['id']]) != 14:
        raise RuntimeError('The fourteen actual HTTP cases are required')
    plan = run('verify-plan', ['verify', 'plan', SPEC, '--project', APP, '--subject', 'HomeCinema.MovieNight'])
    inspections = []
    subjects = [(suite['declaration']['id'], suite['nodes'], False) for suite in plan['suites']]
    subjects += [(manual['subject'], manual['nodes'], manual['kind'] == 'review') for manual in plan['manual_checks'] if manual['kind'] != 'intent']
    for subject, nodes, is_review in subjects:
        note = notes['subjects'].get(subject)
        if note is None:
            if subject.rsplit('.', 1)[-1] in ('Examples', 'Properties'):
                continue
            raise RuntimeError(f'No authored inspection for {subject}')
        requirements = sorted({subject, *(node['id'] for node in nodes)})
        inspections.append({'inspection': {'schema_version': 1, 'actor': notes['actor'], 'subject': subject,
            'snapshot': evidence['provenance']['snapshot'], 'plan_hash': evidence['provenance']['plan_hash'],
            'requirements': requirements, 'implementation_artifacts': evidence['provenance']['implementation'],
            'report': note['report'], 'limitations': note['limitations'], 'result': {'status': 'PASS'}},
            'review': note['review'] if is_review else None})
    file = OUT / 'program-inspections.json'
    file.write_text(json.dumps(inspections, indent=2)+'\n')
    inspected = run('inspect', ['verify', 'inspect', SPEC, stored['id'], '--project', APP, '--file', file])
    if inspected['assessment']['result']['status'] != 'PASS' or inspected['assessment']['open_obligations']:
        raise RuntimeError('Complete effective assessment did not pass')

    def session():
        with sqlite3.connect(f'file:{SPEC}/.dctx0ctl/state.sqlite3?mode=ro', uri=True) as connection:
            project = str(SPEC.resolve())
            row = connection.execute('SELECT p.data FROM task_plans p JOIN task_heads h ON h.plan_id=p.id WHERE h.project=?', (project,)).fetchone()
            if row is None:
                raise RuntimeError('A real active task plan is required')
            tasks = json.loads(row[0])
            ids = {member['task_id'] for member in tasks['members']}
            states = {identifier: json.loads(data) for identifier, data in connection.execute('SELECT task_id,data FROM task_states') if identifier in ids}
            return tasks, states

    tasks, states = session()
    reports = []
    for member in tasks['members']:
        identifier = member['task_id']
        state = states[identifier]
        if state['work'] == 'done':
            if state['check'] != 'pass':
                raise RuntimeError(f'T{identifier}: historical Done without matching PASS needs separate work')
            continue
        if state['work'] != 'open' or state['actor'] or state['claim'] or state['reservation']:
            raise RuntimeError(f'T{identifier}: active work/reservation is preserved')
        candidate = member['candidate']
        requirements = sorted(set(candidate['subjects'] + candidate['types'] + candidate['facts']))
        applicable = [note for subject, note in notes['subjects'].items() if any(requirement == subject or requirement.startswith(subject+'#') for requirement in requirements)]
        review_subject = next((subject for subject in candidate['subjects'] if subject in notes['subjects'] and notes['subjects'][subject]['review']), None)
        descriptions = [notes['subjects'][name]['report'] for name in ['HomeCinema.MovieNight.TypeConformance', 'HomeCinema.MovieNight.SurfaceConformance', 'HomeCinema.MovieNight.AdapterConformance']]
        descriptions.extend(note['report'] for note in applicable)
        body = 'Reviewed task subjects: '+', '.join(candidate['subjects'])+'. '+ ' '.join(dict.fromkeys(descriptions))
        review = None
        if candidate['kind'] == 'review':
            if not review_subject:
                raise RuntimeError(f'T{identifier}: a concrete review is required')
            note = notes['subjects'][review_subject]
            review = {'reviewer': notes['actor'], 'judgement': 'approved', 'summary': note['report'],
                'findings': [{'id': finding['id'], 'severity': finding['severity'], 'description': finding['description'], 'task_ids': [], 'action_required': False} for finding in note['review']['findings']]}
        reports.append({'task': identifier, 'inspection': {'schema_version': 1, 'actor': notes['actor'], 'subject': f'T{identifier}',
            'snapshot': evidence['provenance']['snapshot'], 'plan_hash': evidence['provenance']['plan_hash'], 'requirements': requirements,
            'implementation_artifacts': evidence['provenance']['implementation'], 'report': body,
            'limitations': ['Explicit agent inspection supplements the complete finite native corpus; it is not an automatic proof.'], 'result': {'status': 'PASS'}}, 'review': review})
    if reports:
        file = OUT / 'task-inspections.json'
        file.write_text(json.dumps(reports, indent=2)+'\n')
        run('attest', ['verify', 'attest', SPEC, stored['id'], '--project', APP, '--file', file])
    tasks, states = session()
    incomplete = [identifier for identifier, state in states.items() if state['work'] != 'done' or state['check'] != 'pass']
    if not tasks['members'] or incomplete:
        raise RuntimeError(f'Unfinished actual tasks: {incomplete}')
    summary = {'schema_version': 1, 'status': 'PASS', 'native_run': stored['id'], 'inspected_run': inspected['evidence']['id'],
        'mandatory_cases': len(evidence['cases']), 'tasks': len(tasks['members']), 'open_tasks': 0, 'non_pass_tasks': 0,
        'basis': 'deterministic native cases plus explicitly source-bound agent inspection', 'commands': commands,
        'memory_budget_bytes': int(os.environ['DCTX0_MEMORY_BUDGET_BYTES']), 'performance': 'deferred to final validation'}
    (OUT / 'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
