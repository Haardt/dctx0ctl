#!/usr/bin/env python3
"""Exercise the agent review workflow on unchanged Cinema using authored fixture notes.

This is workflow acceptance, not a newly generated architectural approval.
Performance measurements are run separately at the end by validate.py.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/dctx0')
    parser.add_argument('--http-frameworks', action='store_true', help='exercise Gin and Axum with reviewed additional Cinema routers')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/conformance-reviews/acceptance.json')
    args = parser.parse_args()
    binary = args.binary.resolve()
    report = {'schema_version': 1, 'status': 'incomplete', 'basis': 'Workflow test reusing source-validated authored Cinema inspection fixtures; not generated architectural approvals.', 'targets': {}}
    with tempfile.TemporaryDirectory(prefix='dctx0-conformance-review-') as temporary:
        base = Path(temporary)
        variants = [('gin', 'go', 'cinema-go'), ('axum', 'rust', 'cinema-rust')] if args.http_frameworks else [('typescript', 'typescript', 'cinema'), ('go', 'go', 'cinema-go'), ('rust', 'rust', 'cinema-rust')]
        for variant, target, directory in variants:
            project = base / variant
            shutil.copytree(ROOT / 'examples' / directory, project, ignore=shutil.ignore_patterns('.dctx0ctl', '.bin', 'target', 'cinema.sqlite3*'))
            spec = project / 'dctx0'
            notes_path = project / '.dctx0/testing' / target / 'inspection-notes.json'
            notes = json.loads(notes_path.read_text())
            for name, expected in notes['reviewed_hashes'].items():
                assert hashlib.sha256((project / name).read_bytes()).hexdigest() == expected
            if args.http_frameworks:
                configuration = project / '.dctx0/testing' / target / 'config.json'
                config = json.loads(configuration.read_text())
                assert 'httpFramework' not in config, 'Reference configuration must retain the original default'
                config['httpFramework'] = variant
                configuration.write_text(json.dumps(config, indent=2) + '\n')
                # Only the explicitly inspected framework selection changes; the entire
                # code/dependency fixture was hash-checked above, not silently rebound.
                notes['reviewed_hashes'][str(configuration.relative_to(project))] = hashlib.sha256(configuration.read_bytes()).hexdigest()
                notes['actor'] = 'codex-framework-source-review'
                notes['scope'] += '; explicitly selected and inspected ' + variant + ' production router.'
                notes_path.write_text(json.dumps(notes, indent=2) + '\n')


            def invoke(*command, success=True):
                result = subprocess.run([str(binary), '--json', *map(str, command)], cwd=ROOT, env={**os.environ, 'DCTX0_MEMORY_GUARD': '0'}, capture_output=True, text=True, timeout=240)
                if success and result.returncode:
                    raise RuntimeError(f'{target}: {command}: {result.stderr}\n{result.stdout[:2000]}')
                return json.loads(result.stdout)

            invoke('check', spec)
            plan = invoke('tasks', 'plan', spec)['body']['data']
            # Claiming the first task publishes editable profiles before source binding.
            invoke('profile', target, 'create', '--project', project, '--default')
            native = invoke('verify', 'run', spec, '--project', project, success=False)
            run = native['evidence']['run']
            assert len(run['cases']) == len(run['required_cases']) == 5674, f'{variant}: {len(run["cases"])}/{len(run["required_cases"])} cases; {run["result"]}; {run["provenance"]["incomplete"]}'
            assert all(case['result']['status'] == 'PASS' for case in run['cases']), f'{variant}: failing cases: {[(case["id"], case["result"]) for case in run["cases"] if case["result"]["status"] != "PASS"][:5]}'
            assert len(run['open_obligations']) == 25
            reviews = [member for member in plan['members'] if member['candidate']['kind'] == 'review']
            assert len(reviews) == 4
            assigned = [subject for member in reviews for subject in member['candidate']['review_obligations']]
            assert len(assigned) == len(set(assigned)) == 25
            # Existing authored fixture inspections already cover these implementation
            # tasks. Explicitly import them, rather than inventing new approvals.
            attestations = []
            for member in plan['members']:
                task = member['task_id']
                if member['candidate']['kind'] == 'review' or task in native['adopted_tasks']:
                    continue
                candidate = member['candidate']
                attestations.append({'task': task, 'review': None, 'inspection': {
                    'schema_version': 1, 'actor': notes['actor'], 'subject': f'T{task}',
                    'snapshot': run['provenance']['snapshot'], 'plan_hash': run['provenance']['plan_hash'],
                    'requirements': sorted(set(candidate['subjects'] + candidate['types'] + candidate['facts'])),
                    'implementation_artifacts': run['provenance']['implementation'],
                    'report': 'Retained authored Cinema implementation inspection: ' + '\n'.join(note['report'] for note in notes['subjects'].values()),
                    'limitations': ['Existing explicit inspection fixture; finite native corpus, not an exhaustive proof.'], 'result': {'status': 'PASS'},
                }})
            imports = base / f'{target}-attestations.json'
            # Captured toolchain paths vary by runner. Preserve every inspection
            # while respecting the CLI's 1-MiB import limit on all platforms.
            batch = []
            for attestation in attestations:
                encoded = json.dumps([*batch, attestation], separators=(',', ':')).encode()
                if len(encoded) > 1_048_576 and batch:
                    imports.write_text(json.dumps(batch, separators=(',', ':')))
                    invoke('verify', 'attest', spec, native['evidence']['id'], '--project', project, '--file', imports)
                    batch = []
                assert len(json.dumps([attestation], separators=(',', ':')).encode()) <= 1_048_576, 'A single inspection exceeds the import limit'
                batch.append(attestation)
            if batch:
                imports.write_text(json.dumps(batch, separators=(',', ':')))
                invoke('verify', 'attest', spec, native['evidence']['id'], '--project', project, '--file', imports)
            # Removing the legacy notes proves completion uses persisted task reviews.
            notes_path.unlink()
            for member in sorted(reviews, key=lambda item: item['candidate']['wave']):
                task = member['task_id']
                state = invoke('tasks', 'start', spec, f'T{task}', '--project', project, '--actor', notes['actor'])['body']['data']['state']
                review = invoke('verify', 'review-template', spec, '--project', project, '--task', f'T{task}')
                review.update(reviewer=notes['actor'], judgement='approved', summary='Reviewed all assigned requirements against the unchanged authored Cinema fixture.')
                findings = []
                for subject, value in review['conformance']['subjects'].items():
                    if subject.endswith('.Release'):
                        value.update(report='Reviewed the declared Release gates: native Examples/Properties, conformance outcomes and scoped review judgements/high findings. Rust must evaluate these gates; this review cannot override their result.', limitations=['Modeled policy gates only; no additional prose assertions.'])
                    else:
                        value.update(report=notes['subjects'][subject]['report'], limitations=notes['subjects'][subject]['limitations'])
                        authored = notes['subjects'][subject].get('review')
                        if authored:
                            assert authored['approved']
                            findings.extend({**finding, 'task_ids': [], 'action_required': False} for finding in authored['findings'])
                review['findings'] = findings
                review_file = base / f'{target}-T{task}.json'
                review_file.write_text(json.dumps(review))
                state = invoke('tasks', 'record-review', spec, f'T{task}', '--project', project, '--actor', notes['actor'], '--claim', state['claim'], '--version', state['version'], '--file', review_file)['body']['data']['state']
                state = invoke('tasks', 'complete', spec, f'T{task}', '--project', project, '--actor', notes['actor'], '--claim', state['claim'], '--version', state['version'], '--summary', 'Assigned conformance obligations reviewed with source-bound evidence.')['body']['data']['state']
                assert state['work'] == 'done' and state['check'] == 'unverified'
            complete = invoke('verify', 'complete', spec, '--project', project)
            assert complete['tasks'] == 105 and complete['open_tasks'] == 0
            assert complete['non_pass_tasks'] == len(complete['reviewed_tasks']) == 4
            assert complete['mandatory_cases'] == 5674 and complete['result']['status'] == 'PASS'
            report['targets'][variant] = complete
    report['status'] = 'pass'
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Cinema task-review PASS: {list(report["targets"])}; 25 assigned obligations, 4 Done/Unverified review tasks per variant. {args.report}')


if __name__ == '__main__':
    main()
