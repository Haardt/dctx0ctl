"""Freeze only a complete, source-stable application-test release validation; never promote baseline."""
import datetime,gzip,hashlib,json,shutil,subprocess,sys,tarfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
import validate

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,value): path.write_text(json.dumps(value,indent=2)+'\n')
before_root=ROOT/'benchmarks/application-tests/before-20261108T172542Z'
before=json.loads((before_root/'validation-and-measurements.json').read_text())
before_ref=json.loads((before_root/'reference.json').read_text())
current=validate.digest(validate.validation_sources())
records=[]
for path in sorted((ROOT/'benchmarks/history').glob('*.json')):
 data=json.loads(path.read_text())
 if data.get('source_hash')==current and data.get('status')=='complete' and data.get('validation_status')=='pass': records.append((path,data))
assert records,'No complete validated post-change measurements for the current source hash'
path,after=records[-1]
assert after['validation_source_hash']==after['validation_source_hash_after']==after['source_hash']
assert after.get('validation_scope')=='workspace'
assert before_ref['global_baseline_sha256']==sha(ROOT/'benchmarks/baseline.json')
coverage=json.loads((ROOT/'target/coverage/source-summary.json').read_text())
assert coverage['source_hash_before']==coverage['source_hash_after'] and coverage['instrumented_exit_code']==0
assert coverage['line_percent']>=97 and coverage['llvm_totals']['functions']['percent']>=97
files={line[3:] for line in (ROOT/'target/coverage/workspace.lcov').read_text().splitlines() if line.startswith('SF:')}
required=['crates/dctx0-verification/src/application_plan.rs','crates/dctx0-verification/src/application_session.rs','crates/dctx0-verification/src/application_session/decoder.rs','crates/dctx0-verification/src/application_session/operation.rs','crates/dctx0-verification/src/application_session/assertion.rs','crates/dctx0-target-sdk/src/application_protocol.rs','crates/dctx0-engine/src/application_testing.rs','crates/dctx0-engine/src/application_testing_service.rs','crates/dctx0-cli/src/application_testing.rs','crates/dctx0-engine/src/application_library.rs','crates/dctx0-engine/src/application_test_junit.rs','crates/dctx0-engine/src/verification_run/application_run.rs','crates/dctx0-engine/src/verification_run/application_run/replay.rs','crates/dctx0-engine/src/verification_run/application_run/publication.rs','crates/dctx0-verification/src/application_evidence.rs','crates/dctx0-verification/src/application_session/scenario.rs','crates/dctx0-verification/src/application_session/lifecycle.rs']
assert all(str(ROOT/name) in files for name in required),'Missing application production file in LCOV'
name='after-cinema-native-'+path.stem.rsplit('-',1)[0]
dest=ROOT/'benchmarks/application-tests'/name
dest.mkdir(exist_ok=False)
shutil.copyfile(path,dest/'validation-and-measurements.json')
shutil.copyfile(ROOT/'target/application-test-after-validation.log',dest/'validation.log')
for name in ['source-summary.json','llvm.json','workspace.lcov']:
 (dest/(name+'.gz')).write_bytes(gzip.compress((ROOT/'target/coverage'/name).read_bytes(),mtime=0))
with (dest/'source.patch').open('wb') as stream:
 subprocess.run(['git','diff','--binary','--','.',':(exclude)benchmarks/history',':(exclude)benchmarks/application-tests'],cwd=ROOT,stdout=stream,check=True)
new=subprocess.check_output(['git','ls-files','--others','--exclude-standard','-z'],cwd=ROOT).decode().split('\0')
new_sources=sorted(name for name in new if name and (name.startswith(('crates/','targets/','examples/','scripts/')) and Path(name).suffix in ('.rs','.go','.ts','.py','.json','.md','.toml','.mod') or name.startswith('docs/application-test') and Path(name).suffix in ('.json','.md')))
with tarfile.open(dest/'new-source-files.tar.gz','w:gz') as archive:
 for name in new_sources: archive.add(ROOT/name,arcname=name,recursive=False)
write(dest/'source-inputs.json',{'algorithm':'scripts/validate.py digest(validation_sources()), ordered paths including duplicates','source_hash':current,'files':[{'path':str(p.relative_to(ROOT)),'sha256':sha(p)} for p in sorted(validate.validation_sources())],'new_source_files':new_sources})
all_example_files=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z','--','examples/cinema','examples/cinema-go'],cwd=ROOT).decode().split('\0')
with tarfile.open(dest/'cinema-source.tar.gz','w:gz') as archive:
 for name in sorted(set(p for p in all_example_files if p)): archive.add(ROOT/name,arcname=name,recursive=False)
artifacts=dest/'native';artifacts.mkdir()
for target in ['go','rust']:
 for kind in ['acceptance','performance']:
  shutil.copyfile(ROOT/f'target/{target}-cinema/{kind}.json',artifacts/f'{target}-{kind}.json')
shutil.copyfile(ROOT/'target/go-cinema/race.json',artifacts/'go-race.json')
scopes={}
for target,forms in [('go',['decoder','operation','state-change','assertion']),('typescript',['constructor','operation','state-change','assertion'])]:
 union=set()
 for form in forms:
  source=ROOT/f'target/application-performance/{target}-{form}'
  folder=artifacts/f'{target}-{form}';folder.mkdir()
  for item in source.iterdir():
   if item.is_file(): shutil.copyfile(item,folder/item.name)
  report=json.loads((source/'result.json').read_text())
  assert report.get('status', 'pass' if report.get('exit_code')==0 else 'fail')=='pass'
  bundle=json.load(gzip.open(source/'prepared-bundle.json.gz'))
  assessment=json.load(gzip.open(source/'assessment.json.gz'))
  assert assessment['result']['status']=='PASS' and assessment['finalized']
  assert not assessment['missing_cases'] and not assessment['missing_tests']
  ids=set(assessment['required_cases']);assert len(ids)==len(assessment['cases']) and not union.intersection(ids)
  assert set(bundle['plan']['manifest']['required_cases'])==set(json.loads((before_root/'case-manifest.json').read_text())['required_cases'])
  union.update(ids)
 assert len(union)==4165
 scopes[target]=sorted(union)
assert scopes['go']==scopes['typescript']
full_scopes={}
for target in ['go','typescript']:
 for kind in ['all','managed']:
  source=ROOT/f'target/application-performance/{target}-{kind}'
  folder=artifacts/f'{target}-{kind}';folder.mkdir()
  for item in source.iterdir():
   if item.is_file():shutil.copyfile(item,folder/item.name)
  receipt=json.loads((source/'result.json').read_text())
  assert receipt.get('status','pass' if receipt.get('exit_code')==0 else 'fail')=='pass'
  if kind=='all':
   assessment=json.load(gzip.open(source/'assessment.json.gz'))
   full_scopes[target]=set(assessment['required_cases'])
   assert len(full_scopes[target])==5766 and all(case['result']['status']=='PASS' for case in assessment['cases'])
  else:
   assert receipt['completed_cases']==5766 and receipt['replay']['production_mutation']=='FAIL' and receipt['replay']['repaired']=='PASS'
   assert receipt['adoption_authority'] and receipt['task_completion']['done']==105 and receipt['task_completion']['pass']==105 and receipt['task_completion']['review_done_unverified']==0 and receipt['task_completion']['source_bound_reviews']==4
   assert set(receipt['connected_replay'])=={'lifecycle','entity-identity'}
   assert all(replay['result']['status']=='PASS' and not replay['adoption_authority'] for replay in receipt['connected_replay'].values())
assert full_scopes['go']==full_scopes['typescript']

# Preserve actual negative native evidence and both different-input probes.
for source in sorted((ROOT/'target/application-validation').glob('*probe-*')):
 if not source.is_dir():continue
 receipt=json.loads((source/'result.json').read_text())
 assert receipt.get('probe_detected') is True or isinstance(receipt.get('probe'),dict) and receipt['probe'].get('detected') is True
 folder=artifacts/source.name;folder.mkdir()
 for item in source.iterdir():
  if item.is_file():shutil.copyfile(item,folder/item.name)
write(dest/'native-cases.json',{'cases':sorted(full_scopes['go']),'bounded_pilot_cases':scopes['go'],'native_cases_per_language':5766,'full_required_cases':5766,'remaining_legacy_cases':[],'scope':'All original finite Cinema cases in each native suite; separate transactional mixed-group adoption remains outside this increment'})
common={}
environment=validate.compatible(after,before)
for name,metrics in after['measurements'].items():
 old=before['measurements'].get(name)
 if old is None:continue
 comparable=environment and validate.workload_compatible(old,metrics)
 row={'comparable':comparable,'reason':'same recorded environment, settings and measurement scope; single sample only' if comparable else 'measurement environment/settings/scope incompatible'}
 for metric in ['elapsed_ms','peak_rss_bytes']:
  a,b=old[metric],metrics[metric]
  row[metric]={'before':a,'after':b,'absolute_delta':b-a,'percent_delta':(b/a-1)*100 if a else None}
 common[name]=row
write(dest/'comparison.json',{'schema_version':1,'before_reference':str(before_root.relative_to(ROOT)),'after_measurements':str(path.relative_to(ROOT)),'global_baseline_promoted':False,'accepted_baseline_unchanged':True,'comparable_environment':environment,'scope':'Informational before/after comparison of equal existing workloads; new native phases have no equivalent before workload value. One sample/iteration is not a statistical no-regression claim.','common_workloads':common,'new_workloads':sorted(set(after['measurements'])-set(before['measurements'])),'native_cases_per_language':5766,'remaining_legacy_cases':0})
glue_sets={'go':sorted(path for path in (ROOT/'examples/cinema-go/contracttests').glob('*.go') if not path.name.startswith('generated_')),'typescript':[ROOT/'examples/cinema/.dctx0/application-tests'/name for name in ['runtime-contracts.test.ts','operations.test.ts','operation-fixtures.ts','cinema.test.ts','cinema-fixture.ts','http-bindings.ts','session.ts']]}
glue={}
for language,paths in glue_sets.items():
 rows=[{'path':str(p.relative_to(ROOT)),'physical_lines':len(p.read_text().splitlines()),'bytes':p.stat().st_size,'sha256':sha(p)} for p in paths]
 glue[language]={'files':rows,'physical_lines':sum(r['physical_lines'] for r in rows),'bytes':sum(r['bytes'] for r in rows)}
write(dest/'integration-footprint.json',{'scope':'Authored/reused project test connections, excludes library copies and generated plans. Physical lines reflect different formatting styles and are not comparable elapsed development effort. Old adapters cover a larger scope; no labor reduction is inferred.','project_glue':glue,'native_artifact_bytes':{str(folder.relative_to(artifacts)):sum(p.stat().st_size for p in folder.iterdir() if p.is_file()) for folder in sorted(artifacts.iterdir()) if folder.is_dir()},'artifact_scope':'Retained diagnostic plans/reports/logs/coverage per isolated run; duplicated whole plans are counted. No transactional mixed-backend adoption is claimed.'})
binary_cache=ROOT/'target/application-test-reference'/dest.name;binary_cache.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/'target/release/dctx0',binary_cache/'dctx0')
assert validate.digest(validate.validation_sources())==current
reference={'schema_version':1,'status':'validated-cinema-native-suites','timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat(),'cinema_finite_case_migration_complete':True,'transactional_group_adoption_complete':True,'mixed_backend_cohort_adoption_complete':False,'full_required_cases':5766,'native_cases_per_language':5766,'remaining_legacy_cases':0,'scope':'Complete Cinema finite-case native suites, captured builds, independent collection and mutation/repair replay. Guarded native delivery and authored source-bound inspections complete all105 Cinema tasks Done/PASS, including4 explicitly inspected and source-bound reviews. Old adapter support retained; mixed-backend cohort adoption remains separate.','global_baseline_promoted':False,'global_baseline_sha256':sha(ROOT/'benchmarks/baseline.json'),'source_hash':current,'validation_status':'pass','coverage':{'lines':coverage['lines'],'covered_lines':coverage['covered_lines'],'line_percent':coverage['line_percent'],'llvm_totals':coverage['llvm_totals'],'application_production_files_included':True},'validation_steps':len(after['validation_steps']),'workloads':len(after['measurements']),'performance_gate':after['performance_gate'],'measurement_record':str(path.relative_to(ROOT)),'before_reference':str((before_root/'reference.json').relative_to(ROOT)),'source_reconstruction':['source.patch','new-source-files.tar.gz'],'local_frozen_cli':str((binary_cache/'dctx0').relative_to(ROOT)),'frozen_cli_sha256':sha(binary_cache/'dctx0'),'files':{str(p.relative_to(dest)):sha(p) for p in sorted(dest.rglob('*')) if p.is_file() and p.name!='reference.json'}}
write(dest/'reference.json',reference)
print(json.dumps({'reference':str(dest/'reference.json'),'status':reference['status'],'coverage':reference['coverage'],'steps':reference['validation_steps'],'workloads':reference['workloads'],'comparable_common':sum(r['comparable'] for r in common.values())},indent=2))
