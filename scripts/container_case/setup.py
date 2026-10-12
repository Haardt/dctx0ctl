from pathlib import Path
import subprocess,time,json,os,resource
ROOT=Path('/work/casecli');ROOT.mkdir(parents=True,exist_ok=True); (ROOT/'dctx0').mkdir(exist_ok=True)
E=Path('/work/evidence');E.mkdir(exist_ok=True)
samples=[]
def run(name,args,allowed=(0,)):
 started=time.perf_counter(); p=subprocess.Popen(args,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True);out=p.communicate()[0];usage=resource.getrusage(resource.RUSAGE_CHILDREN)
 (E/(name+'.log')).write_text(out);samples.append(dict(name=name,command=args,exit_code=p.returncode,elapsed_ms=(time.perf_counter()-started)*1000,peak_rss_bytes=usage.ru_maxrss*1024,memory_scope='cumulative child high-water RSS for this runner; not incremental or aggregate RAM'))
 (E/'setup-measurements.json').write_text(json.dumps(samples,indent=2)); print(name,p.returncode,out[:1000]);assert p.returncode in allowed
run('skills',['dctx0','install','--project',str(ROOT),'--agent','codex,claude'])
run('profile-help',['dctx0','profile','go','create','--help'],(0,1))
(ROOT/'dctx0/case.dctx0').write_text('''workspace CaseCli {
  surface Text {
    operation upper(text: String) -> String { pure }
    operation lower(text: String) -> String { pure }
  }
  example "upper mixed" {\n    when result = Text.upper("Hello World")\n    expect result == "HELLO WORLD" }
  example "lower mixed" {\n    when result = Text.lower("Hello World")\n    expect result == "hello world" }
  example "upper empty" {\n    when result = Text.upper("")\n    expect result == "" }
  example "lower empty" {\n    when result = Text.lower("")\n    expect result == "" }
  example "upper punctuation" {\n    when result = Text.upper("a-B 123!")\n    expect result == "A-B 123!" }
  example "lower punctuation" {\n    when result = Text.lower("a-B 123!")\n    expect result == "a-b 123!" }
  example "upper unicode" {\n    when result = Text.upper("Grüße café")\n    expect result == "GRÜßE CAFÉ" }
  example "lower unicode" {\n    when result = Text.lower("GRÜßE CAFÉ")\n    expect result == "grüße café" }
}
''')
run('check',['dctx0','check','dctx0'])
run('profile-create',['dctx0','profile','go','create','--project',str(ROOT)])
run('tasks-plan',['dctx0','tasks','plan','dctx0','--project',str(ROOT),'--target','go','--json'])
run('tasks-waves',['dctx0','tasks','waves','dctx0','--json'])
run('verify-init',['dctx0','verify','init','dctx0','--project',str(ROOT),'--target','go','--subject','CaseCli.Text.upper','--subject','CaseCli.Text.lower'])
