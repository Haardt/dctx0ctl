#!/usr/bin/env python3
"""Real captured-source native execution, independent collection and source guards."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
from contextlib import closing
import subprocess
import sys
import history_retention
from native_performance_workspace import native_workspace, filesystem_type
from release_support import measured
ROOT=Path(__file__).resolve().parents[1]


def load_replay_receipt(app, relative, phase):
    path = app / relative
    if not path.is_file():
        raise RuntimeError(f'{phase} produced no managed evidence; inspect {phase}.log for the original failure')
    return json.loads(path.read_text())


def task_snapshot(app):
    """Logical task state, plans, events, links and reviews from this isolated DB."""
    with closing(sqlite3.connect(f'file:{app}/dctx0/.dctx0ctl/state.sqlite3?mode=ro',uri=True)) as database:
        names=[row[0] for row in database.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name") if row[0].startswith('task_') or row[0] in ('tasks','verification_task_links','verification_adoptions') or 'review' in row[0]]
        tables={}
        for name in names:
            rows=sorted(repr(row) for row in database.execute('SELECT * FROM "'+name+'"'))
            tables[name]={'count':len(rows),'sha256':hashlib.sha256('\n'.join(rows).encode()).hexdigest()}
        states=[json.loads(row[0]) for row in database.execute('SELECT data FROM task_states ORDER BY task_id')]
        events=dict(database.execute('SELECT kind,count(*) FROM task_events GROUP BY kind'))
        links=database.execute('SELECT count(*) FROM verification_task_links').fetchone()[0]
        adoptions=database.execute('SELECT count(*) FROM verification_adoptions').fetchone()[0]
    return {'tables':tables,'states':states,'events':events,'links':links,'adoptions':adoptions}


def assert_unpublished_task_plan(before, after):
    assert not before['states'] and not before['events'] and before['links']==0 and before['adoptions']==0, 'Expected fresh unplanned task state'
    assert len(after['states'])==105 and after['events']=={'created':105,'plan':1} and after['links']==0 and after['adoptions']==0, 'Failed native run changed task delivery'
    assert all(state['work']=='open' and state['check']=='unverified' and state['result'] is None and not state['evidence'] and not state['reviews'] and state['actor'] is None and state['claim'] is None and state['reservation'] is None for state in after['states']), 'Native failure changed task results, claims or reviews'


def assert_native_probe(probe, target, evidence):
    assert evidence and evidence['result']['status']=='UNKNOWN' and not evidence['adoption_authority'] and not evidence['adopted_tasks'], 'Native failure has no incomplete managed evidence'
    issues=evidence['issues'];phases=evidence['phases'];phase_names=[phase['phase'] for phase in phases]
    assert len(issues)==1 and issues[0]['code']=='APPLICATION_NATIVE_RUN', 'Unexpected failure layer'
    message=issues[0]['message']
    if probe=='missing-test':
        expected='Native test catalog does not contain its root exactly once' if target=='go' else 'Bun registration catalog differs from its required native root'
        assert expected in message and 'catalog' in phase_names and 'native-tests' not in phase_names, 'Missing-root probe did not reach native catalog'
        catalog=next(phase for phase in phases if phase['phase']=='catalog')
        assert catalog['audit']['exit_code']==0, 'Native catalog did not run successfully'
        if target=='go':
            assert 'TestRemovedCinemaSuite' in catalog['audit']['stdout'] and 'TestCinemaSuite' not in catalog['audit']['stdout'].splitlines()
        else:
            registered=json.loads(catalog['audit']['stdout'])['tests']
            assert any(test['name']=='RemovedCinemaSuite' for test in registered) and not any(test['name']=='CinemaSuite' for test in registered)
    elif probe=='build-failure':
        failed=[phase for phase in phases if phase['phase'].startswith('build-') and phase['audit']['exit_code']!=0]
        assert len(failed)==1 and ('Application test '+failed[0]['phase']+' exited') in message and 'catalog' not in phase_names and 'native-tests' not in phase_names, 'Build probe did not fail in native build'
    else:
        failed=[phase for phase in phases if phase['phase']=='native-tests' and phase['audit']['exit_code']!=0]
        assessment=evidence['assessment']
        assert len(failed)==1 and 'Application test native-tests exited' in message and assessment['result']['status']=='PASS', 'Late native assertion was not executed after central assessment'
        assert len(assessment['cases'])==5766 and not assessment['missing_cases'] and not assessment['missing_tests'] and not assessment['issues'] and all(case['result']['status']=='PASS' for case in assessment['cases']), 'Late assertion did not follow complete central assessment'

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/dctx0')
    parser.add_argument('--target',choices=('go','typescript'),required=True)
    parser.add_argument('--report',type=Path,required=True)
    parser.add_argument('--replay',action='store_true')
    parser.add_argument('--probe',choices=('native-assertion','missing-test','build-failure','source-drift'))
    args=parser.parse_args()
    if args.probe=='source-drift' and args.target!='typescript':parser.error('source-drift tests the reviewed TypeScript recipe')
    binary=args.binary.resolve();destination=args.report.resolve().parent;destination.mkdir(parents=True,exist_ok=True)
    result={'schema_version':1,'status':'incomplete','target':args.target,'probe':args.probe,'steps':{},'adoption_authority':False,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
    def run(name,command,allow_failure=False):
        sample,log=measured([sys.executable,str(ROOT/'scripts/memory_guard.py'),*map(str,command)],ROOT,timeout=420)
        match=re.search(r'elapsed_ms=([0-9.]+); peak_process_tree_rss_bytes=(\d+)',log)
        sample['wait4_peak_rss_bytes']=sample['peak_rss_bytes'];sample['peak_rss_bytes']=int(match[2]) or None if match else None
        sample['memory_scope']='sampled Linux process-tree RSS sum, 50ms; shared pages counted per process'
        if sample['peak_rss_bytes'] is None:sample['memory_status']='unavailable-no-process-tree-sample'
        result['steps'][name]=sample;(destination/(name+'.log')).write_text(log)
        if sample['exit_code'] and not allow_failure:raise RuntimeError(name+' failed; inspect retained log')
        return sample
    try:
        with native_workspace(history_retention.preserve_history()) as directory:
            result['native_workspace']={'path':str(directory),'retained':history_retention.preserve_history(),'filesystem':filesystem_type(directory)}
            app=directory/'app';source=ROOT/('examples/cinema-go' if args.target=='go' else 'examples/cinema')
            shutil.copytree(source,app,ignore=shutil.ignore_patterns('.dctx0ctl','node_modules','.bin','vendor','cinema.sqlite3*'))
            run('publish-library',[binary,'verify','application-tests','library','--project',app,'--target',args.target])
            root_test='TestCinemaSuite' if args.target=='go'else'CinemaSuite'
            plan='.dctx0/application-tests/managed-plan.json';recipe='.dctx0/application-tests/native-run.json';receipt='.dctx0ctl/application-tests/receipt.json'
            run('prepare',[binary,'verify','application-tests','prepare',app/'dctx0','--project',app,'--plan',plan,'--group','cinema-native','--test-name',root_test,'--form','all','--group-seconds','300','--boundary','json-decoder'if args.target=='go'else'value-constructor'])
            bundle=json.loads((app/plan).read_text())
            before=task_snapshot(app) if args.probe else None
            if args.probe:
                if args.probe=='native-assertion':
                    file=app/('contracttests/suite_test.go'if args.target=='go'else'.dctx0/application-tests/cinema.test.ts')
                    text=file.read_text();needle='client := clientFor(t)'if args.target=='go'else"const failures:unknown[]=[];"
                    replacement=needle+'\n defer t.Error("late native assertion")'if args.target=='go'else needle+"failures.push(new Error('late native assertion'));"
                    assert text.count(needle)==1;file.write_text(text.replace(needle,replacement))
                elif args.probe=='source-drift':
                    file=app/recipe;value=json.loads(file.read_text());value['root_test']='MissingNativeRoot';file.write_text(json.dumps(value))
                else:
                    file=app/('contracttests/suite_test.go'if args.target=='go'else'.dctx0/application-tests/cinema.test.ts')
                    text=file.read_text()
                    if args.probe=='missing-test':
                        needle='func TestCinemaSuite(' if args.target=='go' else "test('CinemaSuite',"
                        replacement='func TestRemovedCinemaSuite(' if args.target=='go' else "test('RemovedCinemaSuite',"
                        assert text.count(needle)==1;file.write_text(text.replace(needle,replacement))
                    else:
                        file.write_text(text+('\nfunc (\n' if args.target=='go' else '\nconst __dctx0_invalid_build: =\n'))
            sample=run('managed-run',[binary,'--json','verify','application-tests','complete','--reuse','off',app/'dctx0','--project',app,'--plan',plan,'--group','cinema-native','--recipe',recipe,'--report',receipt],True)
            if (app/receipt).exists():
                evidence=json.loads((app/receipt).read_text());(destination/'managed-evidence.json.gz').write_bytes(gzip.compress(json.dumps(evidence).encode()))
            else:evidence=None
            if args.probe:
                assert sample['exit_code']!=0,'Native negative probe incorrectly passed'
                after=task_snapshot(app)
                if args.probe=='source-drift':
                    assert not evidence, 'Source drift must be rejected before native evidence'
                    assert 'New source inspection required: .dctx0/application-tests/native-run.json' in (destination/'managed-run.log').read_text(), 'Expected authored source-inspection guard'
                    assert before==after and not after['states'] and not after['events'] and after['links']==0 and after['adoptions']==0, 'Preflight rejection changed tasks or reviews'
                    result['failure_layer']='source-inspection'
                else:
                    assert_native_probe(args.probe,args.target,evidence)
                    assert_unpublished_task_plan(before,after)
                    result['failure_layer']='native-catalog' if args.probe=='missing-test' else 'native-build' if args.probe=='build-failure' else 'native-tests'
                result['probe_detected']=True
                result['task_states_after_failure']={'open':len(after['states'])} if after['states'] else {}
                result['task_delivery_links_after_failure']=after['links']
                result['task_adoptions_after_failure']=after['adoptions']
                result['task_history_before']=before['tables'];result['task_history_after']=after['tables']
            else:
                assert sample['exit_code']==0 and evidence,'Managed native run failed'
                assessment=evidence['assessment'];assert evidence['result']['status']=='PASS',evidence['issues']
                assert len(assessment['cases'])==5766 and not assessment['missing_cases']and not assessment['missing_tests']
                assert all(case['result']['status']=='PASS'for case in assessment['cases'])
                assert set(assessment['required_cases'])==set(bundle['plan']['manifest']['required_cases'])
                assert evidence['adoption_authority'] and evidence['adopted_tasks'] and evidence['source_inputs'] and evidence['build_inputs'] and evidence['materialized_inputs']
                output=(destination/'managed-run.log').read_text();start=output.index('\n{')+1
                complete=json.JSONDecoder().raw_decode(output[start:])[0]
                (destination/'complete-result.json').write_text(json.dumps(complete,indent=2)+'\n')
                assert complete['result']['status']=='PASS' and complete['tasks']==105 and complete['open_tasks']==0
                assert complete['non_pass_tasks']==0 and len(complete['reviewed_tasks'])==4
                result['adoption_authority']=True
                result['task_completion']={'tasks':complete['tasks'],'done':complete['tasks']-complete['open_tasks'],'pass':complete['tasks']-complete['non_pass_tasks'],'review_done_unverified':complete['non_pass_tasks'],'source_bound_reviews':len(complete['reviewed_tasks']),'adopted_native':len(evidence['adopted_tasks']),'effective_result':complete['result']}
                result['native_run']=evidence['evidence']['id']
                with closing(sqlite3.connect(f'file:{app}/dctx0/.dctx0ctl/state.sqlite3?mode=ro',uri=True)) as database:
                    assert dict(database.execute('SELECT lower(work),count(*) FROM task_states GROUP BY work'))=={'done':105}
                    result['native_task_delivery_links']=database.execute('SELECT count(*) FROM verification_task_links WHERE run_id=?',(evidence['evidence']['id'],)).fetchone()[0]
                    assert result['native_task_delivery_links']==len(evidence['adopted_tasks']), result['native_task_delivery_links']
                result['completed_cases']=len(assessment['cases']);result['native_phases']=[{'phase':phase['phase'],'elapsed_us':phase['elapsed_us'],'runtime_hash':phase['runtime']['executable_hash']}for phase in evidence['phases']]
                if args.replay:
                    case='HomeCinema.MovieNight.ScreeningPlanning.CandidateOrderTieBreak#assertion:2:0:0'
                    assert any(record['id']==case for record in assessment['cases'])
                    replay_receipt='.dctx0ctl/application-tests/replay-original.json'
                    command=[binary,'verify','application-tests','replay',app/'dctx0','--project',app,'--plan',plan,'--group','cinema-native','--recipe',recipe,'--evidence',receipt,'--case',case,'--report',replay_receipt]
                    original_replay=run('replay-original',command,True)
                    replayed=load_replay_receipt(app,replay_receipt,'replay-original')
                    (destination/'replay-original-evidence.json.gz').write_bytes(gzip.compress(json.dumps(replayed).encode()))
                    assert original_replay['exit_code']==0, replayed['issues']
                    assert replayed['requested_case']==case and replayed['replay_of'] and replayed['result']['status']=='PASS'
                    assert len(replayed['assessment']['cases'])==1 and not replayed['adoption_authority'] and not replayed['adopted_tasks'] and replayed.get('evidence') is None
                    production=app/('domain/contracts.go' if args.target=='go' else 'src/domain/candidate-order-winner.ts')
                    text=production.read_text();before='if count > best {'if args.target=='go'else'count > highest';after='if count >= best {'if args.target=='go'else'count >= highest'
                    assert text.count(before)==1;production.write_text(text.replace(before,after))
                    replay_receipt='.dctx0ctl/application-tests/replay-mutated.json'
                    command[command.index('--report')+1]=replay_receipt
                    failed=run('replay-mutated',command,True)
                    replayed=load_replay_receipt(app,replay_receipt,'replay-mutated')
                    (destination/'replay-mutated-evidence.json.gz').write_bytes(gzip.compress(json.dumps(replayed).encode()))
                    assert failed['exit_code']!=0 and replayed['result']['status']=='FAIL','Historical PASS incorrectly survived changed production'
                    assert replayed['assessment']['cases'][0]['result']['issue']['code']=='ASSERTION_FAILED'
                    production.write_text(text)
                    replay_receipt='.dctx0ctl/application-tests/replay-repaired.json'
                    command[command.index('--report')+1]=replay_receipt
                    run('replay-repaired',command)
                    repaired=load_replay_receipt(app,replay_receipt,'replay-repaired')
                    assert repaired['result']['status']=='PASS'
                    assert not repaired['adoption_authority']
                    result['replay']={'case':case,'original':'PASS','production_mutation':'FAIL','repaired':'PASS','historical_pass_reused':False}
                    result['connected_replay']={}
                    for kind in ('lifecycle','entity-identity'):
                        selected=next(record for record in assessment['cases'] if record['kind']==kind)
                        connected_command=command.copy();connected_command[connected_command.index('--case')+1]=selected['id']
                        replay_receipt='.dctx0ctl/application-tests/replay-'+kind+'.json'
                        connected_command[connected_command.index('--report')+1]=replay_receipt
                        run('replay-'+kind,connected_command)
                        connected=load_replay_receipt(app,replay_receipt,'replay-'+kind)
                        assert connected['result']['status']=='PASS' and not connected['adoption_authority'] and connected.get('evidence') is None
                        replay_ids={record['id']for record in connected['assessment']['cases']}
                        assert selected['id']in replay_ids and not connected['assessment']['missing_cases']
                        if kind=='entity-identity':
                            source_ids={value['value']for value in selected['bindings']['source_cases']['value']}
                            assert source_ids and source_ids.issubset(replay_ids), 'Identity replay reused source observations'
                        (destination/('replay-'+kind+'-evidence.json.gz')).write_bytes(gzip.compress(json.dumps(connected).encode()))
                        result['connected_replay'][kind]={'requested_case':selected['id'],'executed_cases':len(replay_ids),'result':connected['result'],'adoption_authority':False}


            result['status']='pass'
    except Exception as error:
        result['error']=str(error);raise
    finally:
        result['binary_unchanged']=result['binary_sha256']==hashlib.sha256(binary.read_bytes()).hexdigest()
        args.report.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
