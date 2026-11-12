#!/usr/bin/env python3
"""Run the diagnostic constructor pilot, keeping native/time/RSS evidence."""
import argparse
import gzip
import hashlib
import datetime
import platform
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from application_reference import effective_plan_hash, reference_comparison, validate_current_assessment
SOURCE_APP = Path(__file__).resolve().parents[1]
APP = SOURCE_APP
parser = argparse.ArgumentParser()
parser.add_argument('--cli', default=str(ROOT / 'target/debug/dctx0'))
parser.add_argument('--form', choices=['constructor', 'operation', 'state-change', 'assertion', 'scenario', 'lifecycle', 'all'], default='all')
parser.add_argument('--probe', choices=['release-year', 'catch-contract-failure', 'post-verdict-assertion', 'missing-write', 'wrong-return', 'cleanup', 'state-scope', 'missing-state-observer', 'tie-break', 'wrong-key', 'extra-write', 'falsy-native-throw', 'codec-swap', 'operation-codec-swap', 'wrong-business-failure', 'technical-exception', 'missing-calls', 'empty-calls', 'unmodeled-resource', 'unexpected-mock-port'])
parser.add_argument('--output', default=str(ROOT / 'target/typescript-application-tests/central'))
parser.add_argument('--require-reference-match', action='store_true', help='Require compatibility with the frozen historical finite plan')
args = parser.parse_args()
out = Path(args.output).resolve()
out.mkdir(parents=True, exist_ok=True)
plan = '.dctx0/application-tests/plan.json'
report = '.dctx0/application-tests/report.json'
steps = []
cli_path = Path(args.cli) if Path(args.cli).is_absolute() else ROOT / args.cli
cli_hash = hashlib.sha256(cli_path.read_bytes()).hexdigest()
source_hashes = {}
test_name = {'constructor':'CinemaRuntimeContracts', 'operation':'CinemaWinner', 'state-change':'CinemaFilmSave', 'assertion':'CinemaWinnerAssertions','scenario':'CinemaSuite','lifecycle':'CinemaSuite','all':'CinemaSuite'}[args.form]
native_file = 'runtime-contracts.test.ts' if args.form == 'constructor' else 'cinema.test.ts' if args.form in ('scenario','lifecycle','all') else 'operations.test.ts'
extra = [] if args.form == 'constructor' else ['--form', args.form] if args.form in ('assertion','scenario','lifecycle','all') else ['--form', args.form, '--subject', 'HomeCinema.MovieNight.' + ('ScreeningPlanning.VotingRules.candidateOrderWinner' if args.form == 'operation' else 'Catalog.FilmCatalogStore.save')]

def guarded(name, command, env=None, cwd=ROOT):
    process = subprocess.Popen(['python3', str(ROOT / 'scripts/memory_guard.py'), *command], cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
    try:
        output, _ = process.communicate(timeout=180)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            output, _ = process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate(timeout=10)
        output += '\nAcceptance command timed out; run incomplete.\n'
    result = subprocess.CompletedProcess(command, process.returncode, output)
    (out / (name + '.log')).write_text(result.stdout)
    metric = re.search(r'elapsed_ms=([0-9.]+); peak_process_tree_rss_bytes=(\d+)', result.stdout)
    rss = int(metric[2]) if metric else None
    steps.append(dict(name=name, command=command, exit_code=result.returncode, elapsed_ms=float(metric[1]) if metric else None, peak_process_tree_rss_bytes=rss or None, memory_method='sampled Linux process-tree RSS sum, 50ms; null if no sample'))
    return result.returncode

code = guarded('publish', ['bun', 'run', str(APP / 'scripts/publish-application-library.ts')])
if not code:
    workspace = tempfile.TemporaryDirectory(prefix='dctx0-ts-app-')
    APP = Path(workspace.name) / 'cinema'
    shutil.copytree(SOURCE_APP, APP, ignore=shutil.ignore_patterns('.dctx0ctl', 'node_modules', '__pycache__', 'cinema.sqlite3', 'plan.json', 'report.json'))
    source_hashes = {str(path.relative_to(APP)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(APP.rglob('*')) if path.is_file()}
    if args.form in ('scenario','lifecycle','all'):
        code = guarded('native-dependencies', ['bun', 'install', '--cwd', str(APP), '--frozen-lockfile', '--offline'])
    if not code:
        code = guarded('prepare', [args.cli, 'verify', 'application-tests', 'prepare', str(APP / 'dctx0'), '--project', str(APP), '--plan', plan, '--group', 'cinema-typescript', '--test-name', test_name, '--boundary', 'value-constructor', '--group-seconds', '300', *extra])
if not code and args.form in ('scenario','lifecycle','all'):
    code = guarded('facade-generation', ['bun', str(APP / 'scripts/generate-application-facade.ts'), str(APP / plan)])
    if not code:
        code = guarded('native-typecheck', ['bun', 'run', '--cwd', str(APP), 'typecheck'])
    source_hashes = {str(path.relative_to(APP)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(APP.rglob('*')) if path.is_file() and not set(path.relative_to(APP).parts).intersection({'node_modules','.dctx0ctl','__pycache__'}) and path.name not in ('plan.json','report.json')}
if not code and args.form in ('scenario','lifecycle','all'):
    code = guarded('native-catalog', ['bun', '.dctx0/application-tests/library/catalog.ts', '.dctx0/application-tests/' + native_file], cwd=APP)
if not code:
    mutations = []
    constructor_binding = '.dctx0/application-tests/cinema.test.ts' if args.form == 'all' else '.dctx0/application-tests/runtime-contracts.test.ts'
    fixture_binding = '.dctx0/application-tests/cinema-fixture.ts' if args.form == 'all' else '.dctx0/application-tests/operation-fixtures.ts'

    def mutate(relative, before, after):
        source = APP / relative
        text = source.read_text()
        if before not in text:
            raise RuntimeError('Mutation no longer matches its reviewed source: ' + relative)
        changed = text.replace(before, after)
        mutations.append(dict(path=relative, before_sha256=hashlib.sha256(text.encode()).hexdigest(), after_sha256=hashlib.sha256(changed.encode()).hexdigest(), replacement_count=text.count(before)))
        source.write_text(changed)

    if args.probe in ('release-year', 'catch-contract-failure'):
        mutate('src/domain/release-year.ts', 'input < 1888', 'input < 1800')
    if args.probe == 'catch-contract-failure':
        if args.form == 'all':
            mutate(constructor_binding, 'catch(error){failures.push(error);}', 'catch{}')
        else:
            mutate(constructor_binding, 'failed=true;failure=error;', 'failed=false;failure=undefined;')
    if args.probe == 'post-verdict-assertion':
        mutate('.dctx0/application-tests/session.ts' if args.form == 'all' else constructor_binding, 'if(failed)throw failure;', "if(failed)throw failure; throw new Error('native assertion after central finish');")
    if args.probe == 'falsy-native-throw':
        if args.form == 'all':
            mutate(constructor_binding, 'if(failures.length)throw new AggregateError', 'throw undefined; if(failures.length)throw new AggregateError')
        else:
            marker = '},(actual,ty)=>projectValue(actual,ty,bundle.model.data),(native,expected,ty)=>projectInput(native,expected,ty,bundle.model.data));'
            mutate(constructor_binding, marker, marker + ' throw undefined;')
    if args.probe == 'codec-swap':
        data = 'model.data' if args.form == 'all' else 'bundle.model.data'
        marker = '(value,ty)=>inputValue(value,ty,' + data + ')'
        mutate(constructor_binding, marker, "(value,ty)=>{const actual=inputValue(value,ty," + data + ");return ty.symbol_id==='HomeCinema.MovieNight.ReleaseYear'&&actual===1887?1800:actual;}")
    if args.probe == 'operation-codec-swap':
        if args.form == 'all':
            mutate(fixture_binding, 'const original=invocation.arguments[index],native=inputValue(original,parameter.ty,model.data);', "const original=invocation.arguments[index],converted=inputValue(original,parameter.ty,model.data),native=invocation.subject.endsWith('.candidateOrderWinner')&&index===0?[]:converted;")
        else:
            mutate(fixture_binding, 'const native=inputValue(value,operation.parameters[index]!.ty,model.data);', "const converted=inputValue(value,operation.parameters[index]!.ty,model.data);const native=invocation.subject.endsWith('.candidateOrderWinner')&&index===0?[]:converted;")
    if args.probe == 'missing-write':
        mutate('src/persistence/sqlite.ts', 'this.putFilm(film);return this.details(film);', 'return this.details(film);')
    if args.probe == 'extra-write':
        mutate('src/persistence/sqlite.ts', 'this.putFilm(film);return this.details(film);', "this.putFilm(film);this.putFilm({...film,id:'00000000-0000-4000-8000-000000000099'});return this.details(film);")
    if args.probe == 'wrong-key':
        mutate('src/persistence/sqlite.ts', '.run(film.id,JSON.stringify(film))', ".run('wrong-key',JSON.stringify(film))")
    if args.probe == 'wrong-return':
        mutate('src/domain/candidate-order-winner.ts', 'winner = candidate;', "winner = 'broken' as FilmId;")
    if args.probe == 'tie-break':
        mutate('src/domain/candidate-order-winner.ts', 'count > highest', 'count >= highest')
    if args.probe == 'wrong-business-failure':
        mutate('src/application/movie-night.ts', "if (!details) throw new ApplicationFailure('FilmNotFound',{filmId:id});", "if (!details) throw new ApplicationFailure('PollNotFound',{pollId:id});")
    if args.probe == 'unexpected-mock-port':
        mutate('src/application/movie-night.ts', 'searchFilms(filter: FilmSearch) { return this.ports.catalog.search(filter); }', "async searchFilms(filter: FilmSearch) { await this.ports.presence.exists('00000000-0000-4000-8000-000000000001'); return this.ports.catalog.search(filter); }")
    if args.probe == 'technical-exception':
        mutate('src/application/movie-night.ts', "if (!details) throw new ApplicationFailure('FilmNotFound',{filmId:id});", "if (!details) throw new Error('technical exception instead of business rejection');")
    if args.probe in ('missing-calls', 'empty-calls'):
        mutate(fixture_binding, 'return {observation,...trace};', 'return {observation,...trace,calls:' + ('null' if args.probe == 'missing-calls' else '[]') + '};')
    if args.probe == 'unmodeled-resource':
        mutate('src/persistence/sqlite.ts', 'INSERT INTO films VALUES', '/* observed unsupported SQL syntax */ INSERT INTO films VALUES')
    if args.probe == 'cleanup':
        if args.form == 'all':
            mutate(fixture_binding, 'try{restoreSql?.();await session.close();}finally', "try{restoreSql?.();await session.close();throw new Error('native cleanup failed');}finally")
        else:
            mutate(fixture_binding, 'try{database.close();}finally', "try{database.close();throw new Error('native cleanup failed');}finally")
    if args.probe == 'state-scope':
        mutate(fixture_binding, 'values[entity]=', "values['wrong-scope']=")
    if args.probe == 'missing-state-observer':
        mutate(fixture_binding, 'entities:values,complete:true', 'entities:values,complete:false')
    with tempfile.TemporaryDirectory(prefix='dctx0-ts-') as temp:
        socket = str(Path(temp) / 's')
        service_command = [args.cli, 'verify', 'application-tests', 'serve', str(APP / 'dctx0'), '--project', str(APP), '--plan', plan, '--group', 'cinema-typescript', '--socket', socket, '--report', report]
        with (out / 'service.log').open('w') as log:
            service = subprocess.Popen(['python3', str(ROOT / 'scripts/memory_guard.py'), *service_command], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                deadline = time.monotonic() + 60
                while not Path(socket).exists() and service.poll() is None and time.monotonic() < deadline:
                    time.sleep(.05)
                if not Path(socket).exists():
                    raise RuntimeError('Assessment service did not open socket; inspect service.log')
                env = dict(os.environ, DCTX0_APPLICATION_SOCKET=socket, DCTX0_APPLICATION_PLAN=str(APP / plan))
                code = guarded('native', ['bun', 'test', '--reporter', 'junit', '--reporter-outfile', str(out / 'native.junit.xml'), '--test-name-pattern', '^' + test_name + '$', str(APP / ('.dctx0/application-tests/' + native_file))], env)
                service_code = service.wait(timeout=10)
                if not code:
                    code = service_code
            finally:
                if service.poll() is None:
                    os.killpg(service.pid, signal.SIGTERM)
                    try:
                        service.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(service.pid, signal.SIGKILL)
                        service.wait(timeout=10)
        metric = re.search(r'elapsed_ms=([0-9.]+); peak_process_tree_rss_bytes=(\d+)', (out / 'service.log').read_text())
        steps.append(dict(name='service', command=service_command, exit_code=service.returncode, elapsed_ms=float(metric[1]) if metric else None, peak_process_tree_rss_bytes=int(metric[2]) or None if metric else None))
native_toolchain=subprocess.run(['bun','--version'],cwd=ROOT,capture_output=True,text=True,check=True).stdout.strip()
summary = dict(source_mutations=mutations if 'mutations' in globals() else [], native_toolchain=native_toolchain, timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(), platform=platform.platform(), cli_sha256=cli_hash, cli_unchanged=cli_hash == hashlib.sha256(cli_path.read_bytes()).hexdigest(), source_hashes=source_hashes, schema_version=1, scope=f'diagnostic Cinema {args.form} group, no collector/build authority/adoption', steps=steps, exit_code=code, benchmark_comparison='unassessed development run; accepted baseline untouched')
if not summary['cli_unchanged']:
    code = code or 1
    summary['exit_code'] = code
junit = out / 'native.junit.xml'
if junit.exists():
    collected = [dict(name=case.attrib.get('name'), skipped=case.find('skipped') is not None, failed=case.find('failure') is not None or case.find('error') is not None) for case in ET.parse(junit).iter('testcase')]
    selected = [case for case in collected if case['name'] == test_name]
    summary['native_collection'] = dict(expected=test_name, tests=collected, selected_present_once=len(selected) == 1, selected_skipped=selected[0]['skipped'] if len(selected) == 1 else None)
    if len(selected) != 1 or selected[0]['skipped']:
        code = code or 1
        summary['exit_code'] = code
elif any(step['name'] == 'native' for step in steps):
    code = code or 1
    summary['exit_code'] = code
    summary['native_collection'] = dict(error='missing JUnit runner output')
if (APP / report).exists():
    assessment = json.loads((APP / report).read_text())
    bundle = json.loads((APP / plan).read_text())
    summary['effective_plan_hash'] = effective_plan_hash(bundle, args.form)
    with gzip.open(out / 'prepared-bundle.json.gz', 'wt') as stream:
        json.dump(bundle, stream)
    with gzip.open(out / 'assessment.json.gz', 'wt') as stream:
        json.dump(assessment, stream)
    original = json.loads((ROOT / 'benchmarks/application-tests/before-20261108T172542Z/case-manifest.json').read_text())
    summary['comparison'] = dict(original_required_cases=len(original['required_cases']), current_required_cases=len(bundle['plan']['manifest']['required_cases']), identical_required_cases=original['required_cases'] == bundle['plan']['manifest']['required_cases'], native_required_cases=len(assessment['required_cases']), native_completed_cases=len(assessment['cases']), native_result=assessment['result'], remaining_legacy_cases=sum(len(group['area']['cases']) for group in bundle['plan']['configuration']['groups'] if group['backend'] == 'adapter'))
    representation_cases = []
    for contract in bundle['plan']['program']['contracts']:
        ty = contract['ty']
        while ty.get('symbol_id') and bundle['model']['data'][ty['symbol_id']]['alias']:
            ty = bundle['model']['data'][ty['symbol_id']]['alias']
        if ty['name'] not in ('Option', 'Dctx0Option'):
            for index, value in enumerate(contract['rejection_corpus']['values']):
                if value == {'kind':'option', 'value':{'kind':'unit'}}:
                    case_id = contract['subject'] + ':rejection:' + str(index)
                    if case_id in assessment['required_cases']:
                        representation_cases.append(case_id)
    summary['declared_native_representation_limits'] = dict(boundary='non-Option constructor malformed-input representation', mapping='Some(Unit) and None both map to native undefined; original form metadata retained without claiming native distinguishability', cases=representation_cases, tagged_options_required_for_option_unit_contracts=True)
    before = json.load(gzip.open(ROOT / 'benchmarks/application-tests/typescript-before-20261108T172542Z/native-whole-plan.json.gz'))['evidence']['run']
    old_plan = json.load(gzip.open(ROOT / 'benchmarks/application-tests/before-20261108T172542Z/finite-plan.json.gz'))
    historical = reference_comparison(old_plan, bundle['plan'], assessment['cases'], before['cases'])
    summary['historical_reference'] = historical
    summary['comparison'].update(identical_budgets=before['budgets'] == bundle['plan']['program']['budgets'], mismatched_case_results=historical['different_results'], mismatched_case_inputs=historical['different_inputs'], mismatched_case_arguments=historical['different_arguments'], historical_status=historical['status'])
    if (args.require_reference_match and not historical['compatible']) or (historical['compatible'] and any(historical[key] for key in ('missing_reference_cases', 'different_results', 'different_inputs', 'different_arguments'))):
        code = code or 1
    if not args.probe:
        try:
            validate_current_assessment(bundle['plan'], 'cinema-typescript', assessment)
        except (AssertionError, KeyError, TypeError) as error:
            summary['current_assessment_error'] = str(error)
            code = code or 1
    summary['exit_code'] = code
elif not args.probe:
    code = code or 1
    summary['exit_code'] = code
    summary['current_assessment_error'] = 'No current assessment report'
if args.probe and 'comparison' in summary:
    native = next(step for step in steps if step['name'] == 'native')
    expected_status = 'PASS' if args.probe in ('post-verdict-assertion', 'falsy-native-throw') else 'UNKNOWN' if args.probe in ('cleanup', 'missing-state-observer', 'wrong-key', 'technical-exception', 'missing-calls', 'unmodeled-resource') else 'FAIL'
    detected = summary['cli_unchanged'] and (not args.require_reference_match or summary['historical_reference']['compatible']) and native['exit_code'] != 0 and summary['comparison']['native_result']['status'] == expected_status and summary.get('native_collection', {}).get('selected_present_once') is True and summary['native_collection']['selected_skipped'] is False
    if args.probe == 'codec-swap':
        detected = detected and any(case['id'] == 'HomeCinema.MovieNight.ReleaseYear:rejection:16' and case['result']['status'] != 'PASS' for case in assessment['cases'])
    if args.probe == 'operation-codec-swap':
        detected = detected and summary['comparison']['native_result'].get('issue', {}).get('code') == 'APPLICATION_INPUT_CHANGED'
    mock_case = None
    if args.probe == 'unexpected-mock-port':
        mock_case = next((case for case in assessment['cases'] if case['id'] == 'HomeCinema.MovieNight.Catalog.search and filter the film overview'), None)
        central_rejection = mock_case is not None and mock_case['result'].get('issue', {}).get('message', '').startswith('Central collaborator did not return a representable outcome:')
        detected = detected and central_rejection
    summary['probe'] = dict(name=args.probe, detected=detected, expected_central_status=expected_status, native_exit_code=native['exit_code'])
    if mock_case is not None:
        summary['probe']['unexpected_mock_call_case'] = dict(id=mock_case['id'], result=mock_case['result'], centrally_rejected=central_rejection)
    code = 0 if detected else 1
    summary['exit_code'] = code
if 'workspace' in globals():
    workspace.cleanup()
(out / 'result.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
raise SystemExit(code)
