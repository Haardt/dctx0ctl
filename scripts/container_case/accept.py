from pathlib import Path
import json,subprocess,time,os
ROOT=Path('/work/casecli');E=Path('/work/evidence');samples=[]
def run(name,args,allowed=(0,)):
 started=time.perf_counter()
 with (E/(name+'.log')).open('w') as log:
  p=subprocess.Popen(args,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT);_,status,u=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status)
 out=(E/(name+'.log')).read_text();samples.append(dict(name=name,command=args,exit_code=p.returncode,elapsed_ms=(time.perf_counter()-started)*1000,peak_rss_bytes=u.ru_maxrss*1024,memory_scope='wait4 child/reaped descendants peak, not collective RAM'))
 (E/'acceptance-measurements.json').write_text(json.dumps(samples,indent=2));print(name,p.returncode,out[:300],flush=True);assert p.returncode in allowed;return out
run('installer-real',['bash','-o','pipefail','-c','curl -fsSL https://github.com/Haardt/dctx0ctl/releases/latest/download/install.sh | bash'])
run('installed-version',['dctx0','--version'])
assert len(list((ROOT/'.agents/skills').glob('*/SKILL.md')))==3
assert len(list((ROOT/'.claude/skills').glob('*/SKILL.md')))==3
profile=ROOT/'.dctx0/profiles/go/common.md';profile.write_text(profile.read_text()+'\nKeep Unicode simple casing and exit code 2 for usage errors.\n');previous=profile.read_bytes()
run('profile-preserve',['dctx0','profile','go','create','--project',str(ROOT),'--agents']);assert profile.read_bytes()==previous
skill=ROOT/'.agents/skills/dctx0/SKILL.md';skill.write_text(skill.read_text()+'\nLocal project note.\n');before=skill.read_bytes()
run('skills-preserve',['dctx0','install','--project',str(ROOT),'--agent','codex,claude'],(0,1));assert skill.read_bytes()==before
skill.write_bytes(before.replace(b'\nLocal project note.\n',b''))
run('task-context',['dctx0','tasks','next','dctx0','--project',str(ROOT),'--target','go'])
run('native-go-test',['go','test','./textcase'])
run('cli-empty',['./casecli','upper','']);run('cli-unicode',['./casecli','upper','Grüße café'])
assert run('cli-missing',['./casecli'],(2,)).startswith('usage:')
assert 'unknown command' in run('cli-unknown',['./casecli','typo','hello'],(2,))
run('verify-baseline',['dctx0','verify','run','dctx0','--project',str(ROOT),'--no-adopt','--json','--full'])
good=json.loads((E/'verify-baseline.log').read_text());assert good['evidence']['run']['result']['status']=='PASS'
source=ROOT/'textcase/textcase.go';original=source.read_text();source.write_text(original.replace('strings.ToUpper(text)','text'))
run('verify-mutated',['dctx0','verify','run','dctx0','--project',str(ROOT),'--no-adopt','--json','--full'],(1,))
bad=json.loads((E/'verify-mutated.log').read_text());assert bad['evidence']['run']['result']['status']=='FAIL'
failed=next(c for c in bad['evidence']['run']['cases'] if c['result']['status']=='FAIL');case=failed['id'];rid=bad['evidence']['id']
run('replay-mutated',['dctx0','verify','replay','dctx0',rid,'--case',case,'--project',str(ROOT),'--json','--full'],(1,))
source.write_text(original)
run('replay-repaired',['dctx0','verify','replay','dctx0',rid,'--case',case,'--project',str(ROOT),'--json','--full'])
run('verify-repaired',['dctx0','verify','run','dctx0','--project',str(ROOT),'--no-adopt','--json','--full'])
run('verify-adopt',['dctx0','verify','run','dctx0','--project',str(ROOT),'--json','--full'])
run('tasks-final',['dctx0','tasks','list','dctx0','--json'])
summary={'release':'v0.1.0','container':'dctx0-case-acceptance','base_image':'golang:1.25.5','cases':len(good['evidence']['run']['cases']),'examples':sum(c['kind']=='example' for c in good['evidence']['run']['cases']),'baseline':'PASS','mutation':'FAIL','failed_case':case,'failed_run':rid,'broken_replay_exit':1,'repaired_replay_exit':0,'repaired':'PASS','agents':'Codex/Claude skill installation and agent guidance; no external LLM sessions started','spec_scope':'8 explicit casing examples plus generated String result-contract checks; no general all-Unicode casing proof','memory_limit_bytes':3221225472,'cpus':2,'measurements':samples}
(E/'summary.json').write_text(json.dumps(summary,indent=2))
