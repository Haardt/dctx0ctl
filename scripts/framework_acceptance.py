#!/usr/bin/env python3
"""Coupled Gin/Axum acceptance: functional checks first, release speed/RSS last."""
from verification_storage import saved_verification_artifacts
from cinema_expectations import assert_native_cinema
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile

from memory_guard import guard_script
from release_support import locked, measured

ROOT = Path(__file__).resolve().parents[1]
HISTORY = ROOT / 'benchmarks/history/frameworks'
PREFIX = 'HomeCinema.MovieNight.'


def input_hash():
    digest = hashlib.sha256()
    files = []
    for base in (ROOT/'examples/cinema-go', ROOT/'examples/cinema-rust', ROOT/'targets/go/framework-profiles', ROOT/'targets/rust/framework-profiles'):
        files.extend(p for p in base.rglob('*') if p.is_file() and not any(part in ('target','.bin','.dctx0ctl') for part in p.relative_to(base).parts) and not p.name.startswith('cinema.sqlite3'))
    files.extend(ROOT/'scripts'/name for name in ('framework_acceptance.py','conformance_review_acceptance.py','cinema_expectations.py','memory_guard.py','release_support.py'))
    for path in sorted(files):
        digest.update(str(path.relative_to(ROOT)).encode()); digest.update(path.read_bytes())
    return digest.hexdigest()


def run(command, cwd, record, label, allowed=(0,), timeout=900):
    sample, output = measured(list(map(str,command)),cwd=cwd,timeout=timeout,env={**os.environ,'CARGO_BUILD_JOBS':'2','GOFLAGS':'-p=2','DCTX0_MEMORY_GUARD':'0'})
    record['measurements'].setdefault(label,[]).append(sample)
    log = ROOT/'target/frameworks/logs'/f'{label}.log'
    log.parent.mkdir(parents=True,exist_ok=True); log.write_text(output)
    if sample['exit_code'] not in allowed:
        raise RuntimeError(f'{label} failed ({sample["exit_code"]}): {output[-5000:]}')
    return output


def cli(binary, project, record, label, *command, allowed=(0,)):
    output=run([binary,'--json',*command,*(['--full'] if command[:2] in (('verify','run'),('verify','replay')) else [])],ROOT,record,label,allowed=allowed)
    try: return json.loads(output)
    except json.JSONDecodeError as error: raise RuntimeError(f'{label}: invalid CLI JSON: {output[-2000:]}') from error


def project_copy(destination, language, framework):
    source=ROOT/f'examples/cinema-{language}'
    shutil.copytree(source,destination,ignore=shutil.ignore_patterns('.dctx0ctl','.bin','target','cinema.sqlite3*'))
    config=destination/f'.dctx0/testing/{language}/config.json'
    data=json.loads(config.read_text()); data['httpFramework']=framework
    config.write_text(json.dumps(data,indent=2)+'\n')
    # Native mutation runs do not import old author reviews or adopt tasks.
    (destination/f'.dctx0/testing/{language}/inspection-notes.json').unlink()


def native_details(project, data):
    details = {'engine_elapsed_ms': data['evidence']['run']['elapsed_us'] / 1000}
    rows = saved_verification_artifacts(project / "dctx0/.dctx0ctl/state.sqlite3", data['evidence']['id'])
    for media, raw, expected in rows:
        if hashlib.sha256(raw).hexdigest() != expected: raise RuntimeError('Native metric artifact hash mismatch')
        if media in ('application/vnd.dctx0.runtime-metrics+json', 'application/vnd.dctx0.build-audit+json'):
            details[media] = json.loads(raw)
        if media == 'application/vnd.dctx0.go-build-inputs+json':
            receipt = json.loads(raw)
            details['go_compile'] = {key:receipt.get(key) for key in ('build_elapsed_us','build_peak_rss_bytes','go_version','binary_hash')}
        if media == 'application/vnd.dctx0.rust-build-inputs+json':
            receipt = json.loads(raw)
            details['rust_compile'] = {key:receipt.get(key) for key in ('runtime_version','binary_hash')}
    return details


def assert_native(data, label):
    run=data['evidence']['run']
    assert_native_cinema(run)
    if len(run['open_obligations'])!=25:
        raise RuntimeError(f'{label}: manual obligations were lost')
    return {'mandatory_cases':len(run['required_cases']),'recorded_cases':len(run['cases']),'native_open_obligations':25,'incomplete_provenance':run['provenance']['incomplete'],'evidence_id':data['evidence']['id']}


def mutations(binary, language, framework, project, record):
    spec=project/'dctx0'
    cli(binary,project,record,f'{framework}-check','check',spec)
    if language=='go':
        rows=[('routing','ginapi/handler.go','"GET /films/{id}", "getFilm", id','"GET /films/{id}", "getCurrentVoting", id','Catalog.FilmCatalogApi.getFilm'),
              ('status','ginapi/handler.go','status := 404','status := 418','Catalog.FilmCatalogApi.getFilm'),
              ('middleware','ginapi/handler.go','c.Header("X-Cinema-Framework", "gin")','c.Header("X-Cinema-Framework", "broken")','Catalog.FilmCatalogApi.getFilm'),
              ('domain-through-http','application/service.go','result, err := s.ports.Invoke(port, args)','if operation == "getFilm" { return nil, &domain.ContractError{Message: "Intentional business mutation"} }; result, err := s.ports.Invoke(port, args)','Catalog.FilmCatalogApi.getFilm')]
    else:
        rows=[('routing','src/axumapi.rs','.route("/films/{id}", get(invoke))','.route("/broken/{id}", get(invoke))','Catalog.FilmCatalogApi.getFilm'),
              ('status','src/httpapi.rs','_ => 404,','_ => 418,','Catalog.FilmCatalogApi.getFilm'),
              ('middleware','src/axumapi.rs','HeaderValue::from_static("axum")','HeaderValue::from_static("broken")','Catalog.FilmCatalogApi.getFilm'),
              ('domain-through-http','src/application.rs','let result = self.ports.invoke(port, args)?;','if operation == "getFilm" { return Err(Error::Contract("Intentional business mutation".into())); } let result = self.ports.invoke(port, args)?;','Catalog.FilmCatalogApi.getFilm')]
    for name,relative,before,after,subject in rows:
        file=project/relative; original=file.read_text()
        if original.count(before)!=1: raise RuntimeError(f'{framework} {name}: mutation anchor is not unique')
        try:
            file.write_text(original.replace(before,after))
            data=cli(binary,project,record,f'{framework}-mutation-{name}','verify','run',spec,'--project',project,'--subject',PREFIX+subject,'--no-adopt',allowed=(0,1))
            run_data=data['evidence']['run']
            # An unexpected HTTP response is an observed application exception,
            # not automatically a centrally established contract violation.
            # Keep that UNKNOWN and require independent, real HTTP assertions to
            # fail as well. Missing evidence/build errors never satisfy this.
            http_failures=[case for case in run_data['cases'] if
                ('http' in case['id'] or ':operation:' in case['id']) and
                (case['result']['status']=='FAIL' or
                 (case['result']['status']=='UNKNOWN' and
                  case['result'].get('issue',{}).get('code')=='APPLICATION_EXCEPTION'))]
            if not http_failures or run_data['provenance']['incomplete']:
                raise RuntimeError(f'{framework} {name}: no executed HTTP counterexample: {run_data["result"]}; {run_data["provenance"]["incomplete"]}')
            counter=http_failures[0]
            if language=='go':
                http_command=['go','test','-C',project,'./ginapi','-run','^TestActualGinRoutesValidateInputMapFailuresAndRunMiddleware$','-count=1']
                http_exit=1
                assertion='--- FAIL: TestActualGinRoutesValidateInputMapFailuresAndRunMiddleware'
            else:
                http_command=['cargo','test','--manifest-path',project/'Cargo.toml','--all-features','--test','axum','--offline','--locked','--target-dir',ROOT/'target/frameworks/http-mutation-tests/rust']
                http_exit=101
                test='routers_use_real_sqlite_services_and_do_not_share_state_between_sessions' if name=='domain-through-http' else 'actual_router_preserves_route_arguments_rejections_and_middleware'
                assertion=f'test {test} ... FAILED'
            http_label=f'{framework}-mutation-{name}-http-assertions'
            http_output=run(http_command,ROOT,record,http_label,allowed=(http_exit,))
            if assertion not in http_output:
                raise RuntimeError(f'{framework} {name}: no actual HTTP assertion failure; compilation/tool failures do not count')
            row={'variant':framework,'mutation':name,'case_id':counter['id'],'result':counter['result'],'http_assertion_failure':{'command':[str(item) for item in http_command],'exit_code':http_exit,'assertion':assertion,'measurement':http_label},'source':relative,'http_path_basis':'Captured adapter routes explicit HTTP cases and generated real Surface operations through the selected framework'}
            if name=='routing':
                replay=cli(binary,project,record,f'{framework}-broken-replay','verify','replay',spec,data['evidence']['id'],'--case',counter['id'],'--project',project,allowed=(0,1))
                if replay['case_result']!=counter['result']:raise RuntimeError('Broken route replay did not reproduce the exact native result')
                file.write_text(original)
                replay=cli(binary,project,record,f'{framework}-repaired-replay','verify','replay',spec,data['evidence']['id'],'--case',counter['id'],'--project',project)
                if replay['case_result']['status']!='PASS':raise RuntimeError('Repaired route replay did not pass')
                row['replay']={'broken':counter['result']['status'],'repaired':'PASS','changed_implementation_comparison':True}
            record['mutations'].append(row)
        finally:file.write_text(original)


def save(record):
    HISTORY.mkdir(parents=True,exist_ok=True)
    with locked(HISTORY.parent/'.frameworks.lock'):
        with tempfile.NamedTemporaryFile(mode='w',dir=HISTORY,prefix='.write-',delete=False) as file:
            json.dump(record,file,indent=2);file.write('\n');file.flush();os.fsync(file.fileno())
        path=HISTORY/(record['id']+'.json');os.replace(file.name,path)
        for old in sorted(HISTORY.glob('*.json'))[:-20]:old.unlink()
    report=ROOT/'target/frameworks/validation.json';report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(record,indent=2)+'\n')
    return path


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/release/dctx0')
    parser.add_argument('--samples',type=int,default=3)
    parser.add_argument('--skip-performance',action='store_true',help='functional preparation only; result remains incomplete until measurements run')
    parser.add_argument('--performance-only',action='store_true',help='finish a successful functional run with the exact same inputs and binary')
    args=parser.parse_args()
    scratch = ROOT / 'target/frameworks/tmp'
    scratch.mkdir(parents=True,exist_ok=True)
    # Keep captured application/dependency builds on disk, not a RAM-backed /tmp.
    os.environ['TMPDIR'] = str(scratch)
    tempfile.tempdir = str(scratch)
    if args.samples<1:parser.error('At least one sample is required')
    binary=args.binary.resolve()
    if not binary.is_file():parser.error('Build the release CLI first')
    now=datetime.now(timezone.utc)
    current=input_hash(); binary_hash=hashlib.sha256(binary.read_bytes()).hexdigest()
    record={'schema_version':1,'id':now.strftime('%Y%m%dT%H%M%S.%fZ')+f'-{os.getpid()}','timestamp':now.isoformat(),'status':'incomplete',
      'metadata':{'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'dirty_worktree':bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip()),'source_hash':current,'binary_sha256':binary_hash,'platform':platform.platform(),'cpu':next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),platform.processor()) if Path('/proc/cpuinfo').exists() else platform.processor(),'available_memory_at_start':next((line.strip() for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:')),None) if Path('/proc/meminfo').exists() else None,'memory_guard':{'active':os.environ.get('DCTX0_MEMORY_GUARDED')=='1','budget_bytes':os.environ.get('DCTX0_MEMORY_BUDGET_BYTES')},'go':subprocess.check_output(['go','version'],text=True).strip(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip()},
      'versions':{'gin':'1.12.0','axum':'0.8.9','tokio':'1.52.3','tower':'0.5.3'},'seed':42,'generated_cases':100,'measurements':{},'mutations':[],
      'baseline':{'status':'unassessed','reason':'New HTTP framework workloads; accepted baseline unchanged'},'cache':'fresh managed project build; existing module/compiler caches; repeated warm execution',
      'scope':'Linux Cinema Gin and Axum additions; existing paths retained; authored reviews separate from native PASS'}
    failed=None
    try:
        if args.performance_only:
            prior=json.loads((ROOT/'target/frameworks/validation.json').read_text())
            if prior.get('functional_status')!='pass' or prior['metadata']['source_hash']!=current or prior['metadata']['binary_sha256']!=binary_hash:raise RuntimeError('Performance continuation requires matching successful functional evidence')
            record['functional_evidence']=prior['id'];record['mutations']=prior['mutations'];record['review_acceptance']=prior['review_acceptance'];record['functional_measurements']=prior['measurements'];record['functional_status']='pass'
        else:
            checks=[('go-tests',['go','test','-C','examples/cinema-go','-coverprofile='+str(ROOT/'target/frameworks/go.coverage'),'./...']),
              ('go-vet',['go','vet','-C','examples/cinema-go','./...']),
              ('go-race',['go','test','-C','examples/cinema-go','-race','./...']),
              ('rust-tests',['cargo','test','--manifest-path','examples/cinema-rust/Cargo.toml','--all-features','--offline','--locked']),
              ('rust-default-tests',['cargo','test','--manifest-path','examples/cinema-rust/Cargo.toml','--offline','--locked']),
              ('rust-clippy',['cargo','clippy','--manifest-path','examples/cinema-rust/Cargo.toml','--all-features','--all-targets','--offline','--locked','--','-D','warnings']),
              ('rust-adapter-tests',['cargo','test','--manifest-path','examples/cinema-rust/.dctx0/testing/rust/Cargo.toml','--lib','--offline','--locked']),
              ('rust-format',['cargo','fmt','--manifest-path','examples/cinema-rust/Cargo.toml','--','--check']),
              # The worker is materialized only in captured native builds.
              # Check its shipped template and the authored adapter directly;
              # cargo fmt would require an absent generated bin source here.
              ('rust-adapter-format',['rustfmt','--edition','2024','--check','examples/cinema-rust/.dctx0/testing/rust/adapter.rs','targets/rust/harness/worker.rs']),
              ('framework-task-reviews',[sys.executable,'scripts/conformance_review_acceptance.py','--http-frameworks','--binary',binary,'--report',ROOT/'target/frameworks/task-reviews.json']),
              ('reference-task-reviews',[sys.executable,'scripts/conformance_review_acceptance.py','--binary',binary,'--report',ROOT/'target/frameworks/reference-task-reviews.json'])]
            for label,command in checks:run(command,ROOT,record,label)
            record['review_acceptance']={name:json.loads((ROOT/'target/frameworks'/file).read_text()) for name,file in [('frameworks','task-reviews.json'),('references','reference-task-reviews.json')]}
            with tempfile.TemporaryDirectory(prefix='dctx0-framework-mutations-') as temporary:
                for language,framework in [('go','gin'),('rust','axum')]:
                    project=Path(temporary)/framework;project_copy(project,language,framework)
                    mutations(binary,language,framework,project,record)
            record['functional_status']='pass'
        if not args.skip_performance:
            with tempfile.TemporaryDirectory(prefix='dctx0-framework-performance-') as temporary:
                for language,framework in [('go','gin'),('rust','axum')]:
                    project=Path(temporary)/framework;project_copy(project,language,framework)
                    cli(binary,project,record,f'{framework}-release-check','check',project/'dctx0')
                    first=cli(binary,project,record,f'{framework}-first-managed-build','verify','run',project/'dctx0','--project',project,'--no-adopt',allowed=(0,1))
                    record.setdefault('native_plans',{})[framework]=assert_native(first,framework)
                    record.setdefault('native_metrics',{}).setdefault(framework,[]).append(native_details(project,first))
                    for index in range(args.samples):
                        result=cli(binary,project,record,f'{framework}-warm-native','verify','run',project/'dctx0','--project',project,'--no-adopt',allowed=(0,1))
                        assert_native(result,framework)
                        record['native_metrics'][framework].append(native_details(project,result))
                    command=['go','test','-C','examples/cinema-go','./ginapi','-count=1'] if language=='go' else ['cargo','test','--release','--manifest-path','examples/cinema-rust/Cargo.toml','--all-features','--test','axum','--offline','--locked']
                    run(command,ROOT,record,f'{framework}-http-warmup')
                    for index in range(args.samples):run(command,ROOT,record,f'{framework}-http-checks')
            record['summary'] = {}
            for label, samples in record['measurements'].items():
                if label.endswith(('warm-native','http-checks','first-managed-build')):
                    memory=[sample['peak_rss_bytes'] for sample in samples if sample['peak_rss_bytes'] is not None]
                    record['summary'][label]={'samples':len(samples),'median_elapsed_ms':statistics.median(sample['elapsed_ms'] for sample in samples),'peak_rss_bytes':max(memory) if len(memory)==len(samples) else None}
            record['status']='complete' 
        else:record['status']='incomplete';record['remaining']='Release performance deferred explicitly'
        if input_hash()!=current:raise RuntimeError('Relevant sources changed during acceptance')
    except (OSError,RuntimeError,TimeoutError,AssertionError) as error:
        failed=str(error);record['status']='failed';record['failure']=failed
    path=save(record)
    print(json.dumps({'status':record['status'],'report':str(path.relative_to(ROOT)),'mutations':len(record['mutations']),'failure':failed},indent=2))
    return 1 if failed else 0


if __name__=='__main__':
    guard_script()
    raise SystemExit(main())
