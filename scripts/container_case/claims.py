exec(open('/work/accept.py').read().split("run('installer-real'")[0])
for key in ('a','b'):
 if not (E/('claim-'+key+'.log')).exists():
  run('claim-'+key,['dctx0','tasks','claim-next','dctx0','--actor','container-agent-'+key,'--project',str(ROOT),'--target','go','--json'])
for key in ('a','b'):
 data=json.loads((E/('claim-'+key+'.log')).read_text())['body']['data'];s=data['state']
 run('release-'+key,['dctx0','tasks','release','dctx0',str(data['task_id']),'--actor',s['actor'],'--claim',str(s['claim']),'--version',str(s['version']),'--reason','Container ownership test complete','--json'])
a=json.loads(run('claim-measured-a',['dctx0','tasks','claim-next','dctx0','--actor','container-agent-a','--project',str(ROOT),'--target','go','--json']))['body']['data']
b=json.loads(run('claim-measured-b',['dctx0','tasks','claim-next','dctx0','--actor','container-agent-b','--project',str(ROOT),'--target','go','--json']))['body']['data']
assert a['task_id']!=b['task_id'];assert 'Keep Unicode simple casing' in json.dumps(a['state']['execution_bundle']) if 'execution_bundle' in a['state'] else 'Keep Unicode simple casing' in json.dumps(a)
for key,data in (('a',a),('b',b)):
 s=data['state'];run('release-measured-'+key,['dctx0','tasks','release','dctx0',str(data['task_id']),'--actor',s['actor'],'--claim',str(s['claim']),'--version',str(s['version']),'--reason','Container ownership test complete','--json'])
run('final-waves',['dctx0','tasks','waves','dctx0','--json'])
(E/'claim-measurements.json').write_text(json.dumps(samples,indent=2))
summary=json.loads((E/'summary.json').read_text());summary['distinct_claimed_tasks']=[a['task_id'],b['task_id']];summary['profile_note_frozen_in_claim']=True;summary['task_adoption']='Blocked: published v0.1.0 reports INCOMPLETE_REQUIREMENTS/REQUIREMENT_MAPPING for pure/example facts despite finite-plan PASS; no tasks falsely delivered';(E/'summary.json').write_text(json.dumps(summary,indent=2))
