#!/usr/bin/env python3
"""Native Go Cinema application tests + retained before/input comparison.

Measures build, native tests and assessment separately. No adoption or baseline
promotion. Full mixed backend acceptance is a later gate, explicitly not inferred.
"""
import argparse
import datetime
import platform
import re
import sys
import gzip
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
from go_cinema_acceptance import copy_application
from release_support import measured
from application_reference import effective_plan_hash, reference_comparison, validate_current_assessment

ROOT = Path(__file__).resolve().parents[1]
BEFORE = ROOT / 'benchmarks/application-tests/before-20261108T172542Z'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/dctx0')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/go-application-tests/result.json')
    parser.add_argument('--form', choices=('decoder', 'operation', 'state-change', 'assertion', 'all', 'scenario', 'lifecycle'), default='all')
    parser.add_argument('--subject')
    parser.add_argument('--test-name')
    parser.add_argument('--probe', choices=('tie-break','missing-write','wrong-key','cleanup','wrong-return','operation-input-swap','unexpected-collaborator'))
    parser.add_argument('--require-reference-match', action='store_true', help='Require compatibility with the frozen historical finite plan')
    args = parser.parse_args()
    test_name = args.test_name or {'all':'TestCinemaSuite','scenario':'TestCinemaScenarios','lifecycle':'TestCinemaLifecycles','assertion': 'TestCinemaWinnerAssertions', 'decoder': 'TestCinemaRuntimeContracts', 'operation': 'TestCinemaWinner', 'state-change': 'TestCinemaFilmSave'}[args.form]
    subject = args.subject or {'all':None,'scenario':None,'lifecycle':None,'assertion': None, 'decoder': None, 'operation': 'HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner', 'state-change': 'HomeCinema.MovieNight.Catalog.FilmCatalogStore.save'}[args.form]
    binary = args.binary.resolve()
    native_version = subprocess.run(['go', 'version'], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    output = args.report.resolve().parent
    output.mkdir(parents=True, exist_ok=True)
    report = dict(timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(), platform=platform.platform(), native_toolchain=native_version, revision=subprocess.run(['git','rev-parse','HEAD'],cwd=ROOT,capture_output=True,text=True,check=True).stdout.strip(), cache='warm toolchain and module caches; fresh application and test binary', samples=1, repetitions=1, schema_version=1, status='incomplete', scope=f'diagnostic Go {args.form} group; no mixed adoption authority', binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), steps={}, global_baseline_promoted=False, performance_gate='unassessed development run; incomplete backend migration')
    def run(label, command, env=None, allow_failure=False):
        sample, log = measured([sys.executable, str(ROOT / 'scripts/memory_guard.py'), *map(str, command)], ROOT, timeout=420, env=env)
        metric = re.search(r'elapsed_ms=([0-9.]+); peak_process_tree_rss_bytes=(\d+)', log)
        sample['wait4_peak_rss_bytes'] = sample['peak_rss_bytes']
        sample['peak_rss_bytes'] = int(metric[2]) or None if metric else None
        sample['guard_elapsed_ms'] = float(metric[1]) if metric else None
        sample['memory_scope'] = 'Linux sampled command/descendant RSS sum, 50 ms, shared pages counted per process; unavailable if command completed between samples'
        sample['memory_status'] = 'measured' if sample['peak_rss_bytes'] is not None else 'unavailable: no RSS sample'
        (output / (label + '.log')).write_text(log)
        report['steps'][label] = sample
        if sample['exit_code'] and not allow_failure:
            raise RuntimeError(f'{label} failed; see {output / (label + ".log")}')
        return log
    try:
        with tempfile.TemporaryDirectory(prefix='dctx0-app-go-') as temp:
            project = Path(temp) / 'app'
            source = ROOT / 'examples/cinema-go'
            if any(path.is_symlink() for path in source.rglob('*')):
                raise RuntimeError('Application pilot source contains symlinks; captured source closure is not supported by this preview')
            copy_application(project)
            production_hashes = {str(path.relative_to(project)): hashlib.sha256(path.read_bytes()).hexdigest() for path in project.rglob('*') if path.is_file() and (path.suffix == '.go' or path.name in ('go.mod', 'go.sum', 'library-pin.json'))}
            report['application_inputs'] = production_hashes
            pin = json.loads((project / '.dctx0/application-tests/library-pin.json').read_text())
            assert pin['version'] == '0.2.0' and pin['protocol_version'] == 2
            assert all(hashlib.sha256((project / '.dctx0/application-tests/library' / name).read_bytes()).hexdigest() == value for name, value in pin['sha256'].items()), 'Library copy differs from its pin'
            mutations = {
                'operation-input-swap': ('contracttests/operations_test.go', 'actual, err := codec.encode(item, operation.Parameters[i].Type)', 'if i == 0 { item = []any{} }; actual, err := codec.encode(item, operation.Parameters[i].Type)'),
                'tie-break': ('domain/contracts.go', 'if count > best {', 'if count >= best {'),
                'missing-write': ('persistence/store.go', 'ON CONFLICT(id) DO UPDATE SET document=excluded.document', 'ON CONFLICT(id) DO NOTHING'),
                'wrong-key': ('persistence/store.go', 'record["id"], string(data)', '"wrong-key", string(data)'),
                'cleanup': ('contracttests/operations_test.go', 'Cleanup: store.Close', 'Cleanup: func()error{store.Close();return fmt.Errorf("cleanup failed")}'),
                'wrong-return': ('domain/contracts.go', 'winner = candidate', 'winner = "broken"'),
                'unexpected-collaborator': ('contracttests/application_fixture_test.go', 'actual, productionError := fixture.production(invocation, arguments)', 'if len(context.Mocks)>0 && strings.HasSuffix(invocation.Subject, ".saveFilm") { _, _ = fixture.service.Invoke("getFilm", []any{arguments[0].(domain.Record)["id"]}) }; actual, productionError := fixture.production(invocation, arguments)'),
            }
            if args.probe:
                if args.form == 'all' and args.probe == 'cleanup':
                    mutations['cleanup'] = ('contracttests/application_fixture_test.go', 'if !reflect.DeepEqual(before, after) {', 'if !reflect.DeepEqual(before, after) || true {')
                relative, before, after = mutations[args.probe]
                path = project / relative
                content = path.read_text()
                assert content.count(before) == 1, 'Mutation anchor changed'
                path.write_text(content.replace(before, after))
                report['probe'] = dict(name=args.probe, file=relative, source_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
            plan = '.dctx0/application-tests/plan.json'
            evidence = '.dctx0/application-tests/report.json'
            run('prepare', [binary, 'verify', 'application-tests', 'prepare', project / 'dctx0', '--project', project, '--plan', plan, '--group', 'cinema-go', '--test-name', test_name, '--form', args.form, '--group-seconds', '300', *(['--subject', subject] if subject else [])])
            bundle = json.loads((project / plan).read_text())
            report['effective_plan_hash'] = effective_plan_hash(bundle, args.form)
            old = json.load(gzip.open(BEFORE / 'finite-plan.json.gz'))
            initial_comparison = reference_comparison(old, bundle['plan'], [], [])
            report['historical_reference'] = initial_comparison
            if args.require_reference_match and not initial_comparison['compatible']:
                raise RuntimeError('Current plan is incompatible with the required historical reference')
            run('generate-facade', [sys.executable, project / 'scripts/generate_application_facade.py', '--project', project, '--bundle', project / plan])
            test_binary = Path(temp) / 'contracts.test'
            run('build-native', ['go', 'test', '-C', project, '-c', '-buildvcs=false', '-coverpkg=github.com/Haardt/dctx0ctl/applicationtests/go', '-covermode=atomic', '-o', test_binary, './contracttests'])
            catalog = run('native-catalog', [test_binary, '-test.list=.']).splitlines()
            assert catalog.count(test_name) == 1, 'Expected native parent test is absent or duplicated'
            socket = str(Path(temp) / 's')
            service_command = [str(binary), 'verify', 'application-tests', 'serve', str(project / 'dctx0'), '--project', str(project), '--plan', plan, '--group', 'cinema-go', '--socket', socket, '--report', evidence]
            with (output / 'service.log').open('w') as log:
                started = time.perf_counter()
                service = subprocess.Popen([sys.executable, str(ROOT / 'scripts/memory_guard.py'), *service_command], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    deadline = time.monotonic() + 60
                    while not Path(socket).exists() and service.poll() is None and time.monotonic() < deadline:
                        time.sleep(.05)
                    if not Path(socket).exists():
                        raise RuntimeError('Assessment service did not open its socket')
                    env = dict(os.environ, DCTX0_APPLICATION_SOCKET=socket, DCTX0_APPLICATION_PLAN=str(project / plan))
                    native_log = run('native-tests', ['go', 'tool', 'test2json', '-p', 'cinema/contracttests', test_binary, '-test.v=test2json', f'-test.run=^{test_name}$', '-test.coverprofile=' + str(output / 'helper.coverage')], env, allow_failure=True)
                    # wait4 reports this server's own maximum RSS; no child build is hidden here.
                    _, status, usage = os.wait4(service.pid, 0)
                    service.returncode = os.waitstatus_to_exitcode(status)
                    report['steps']['assessment-service'] = dict(elapsed_ms=(time.perf_counter()-started)*1000, peak_rss_bytes=usage.ru_maxrss*1024, memory_method='Linux wait4 direct server ru_maxrss; may include inherited memory before exec; includes waiting for the native runner', exit_code=service.returncode)
                finally:
                    if service.poll() is None:
                        os.killpg(service.pid, signal.SIGTERM)
                        try:
                            service.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(service.pid, signal.SIGKILL)
                            service.wait()
            service_log = (output / 'service.log').read_text()
            metric = re.search(r'elapsed_ms=([0-9.]+); peak_process_tree_rss_bytes=(\d+)', service_log)
            service_sample = report['steps']['assessment-service']
            service_sample['wait4_peak_rss_bytes'] = service_sample['peak_rss_bytes']
            service_sample['peak_rss_bytes'] = int(metric[2]) or None if metric else None
            service_sample['memory_method'] = 'Linux sampled Rust assessment process/descendant RSS sum, 50 ms; includes waiting for native runner; shared pages counted per process'
            assessment = json.loads((project / evidence).read_text())
            with gzip.open(output / 'prepared-bundle.json.gz', 'wt') as stream:
                json.dump(bundle, stream)
            with gzip.open(output / 'assessment.json.gz', 'wt') as stream:
                json.dump(assessment, stream)
            if args.form in ('decoder','all'):
                collapsed = [case['id'] for case in assessment['cases'] if case['bindings'].get('value') == {'kind': 'option', 'value': {'kind': 'unit'}}]
                report['declared_native_representation_limits'] = dict(boundary='default JSON decoder encoding for malformed non-Option constructor inputs', mapping='Some(Unit), None and Unit encode to JSON null; original canonical form metadata is not independently native-observable', cases=collapsed, tagged_options_required_for_option_unit_contracts=True)
            expected_status = 'UNKNOWN' if args.probe in ('wrong-key','cleanup') else 'FAIL' if args.probe else 'PASS'
            reference = json.load(gzip.open(ROOT / 'benchmarks/application-tests/typescript-before-20261108T172542Z/native-whole-plan.json.gz'))['evidence']['run']
            comparison = reference_comparison(old, bundle['plan'], assessment['cases'], reference['cases'])
            report['historical_reference'] = comparison
            report['actual_input_comparison'] = dict(reference='common original Cinema native input contract; TypeScript wire observations', status=comparison['status'], different_invocations=comparison['different_invocations'], different_arguments=comparison['different_arguments'], missing_reference_cases=comparison['missing_reference_cases'])
            report['assessment'] = dict(status=assessment['result']['status'], completed=len(assessment['cases']), required=len(assessment['required_cases']), failures=[{'id': case['id'], 'result': case['result']} for case in assessment['cases'] if case['result']['status'] != 'PASS'])
            events = [json.loads(line) for line in native_log.splitlines() if line.startswith('{')]
            actual = {event['Test'] for event in events if event['Action'] == 'pass' and 'Test' in event}
            registrations = bundle['plan']['registrations']
            case_tests = {case: registration['test'] for registration in registrations for case in registration['cases']}
            expected = {case_tests[case] + '/case_' + hashlib.sha256(case.encode()).hexdigest() for case in assessment['required_cases']}
            expected_parents = {registration['test'] for registration in registrations} | {test_name}
            if args.probe:
                assert assessment['result']['status'] == expected_status and report['steps']['native-tests']['exit_code'] != 0, 'Mutation was not detected'
                assert any(event['Action'] == 'fail' and event.get('Test') == test_name for event in events), 'Native parent did not retain failure'
                report['probe']['detected'] = True
                report['status'] = 'pass'
                return
            if comparison['compatible']:
                assert not comparison['missing_reference_cases'] and not comparison['different_invocations'] and not comparison['different_arguments'], 'Actual invocation inputs changed'
            assert actual == expected | expected_parents, 'Missing or unexpected native subtest results'
            assert len([event for event in events if event['Action'] == 'pass' and 'Test' in event]) == len(actual), 'Duplicated native results'
            assert not any(event['Action'] in ('fail', 'skip') for event in events), 'Failed/skipped native tests'
            validate_current_assessment(bundle['plan'], 'cinema-go', assessment)
            report['comparison'] = dict(identical_finite_plan=comparison['identical_finite_plan'], identical_manifest=comparison['identical_manifest'], historical_status=comparison['status'], required_cases=len(bundle['plan']['manifest']['required_cases']), native_cases=len(assessment['cases']), remaining_legacy_cases=len(bundle['plan']['manifest']['required_cases'])-len(assessment['cases']), native_subtests=len(expected), result='PASS', scope='Current finite plan; full suite when form=all, otherwise explicit adapter remainder; no task adoption authority')
            report['status'] = 'pass'
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        report['binary_unchanged'] = report['binary_sha256'] == hashlib.sha256(binary.read_bytes()).hexdigest()
        if not report['binary_unchanged']:
            report['status'] = 'incomplete'
            report['error'] = 'CLI binary changed during application tests'
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2)+'\n')
        print(json.dumps(report, indent=2))
        if not report['binary_unchanged']:
            raise RuntimeError(report['error'])


if __name__ == '__main__':
    main()
