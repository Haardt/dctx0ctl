from pathlib import Path
import subprocess,time,json,resource
ROOT=Path('/work/casecli');E=Path('/work/evidence');samples=[]
def run(name,args,allowed=(0,)):
 started=time.perf_counter()
 with (E/(name+'.log')).open('w') as log:
  p=subprocess.Popen(args,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
  _,status,usage=__import__('os').wait4(p.pid,0);p.returncode=__import__('os').waitstatus_to_exitcode(status)
 out=(E/(name+'.log')).read_text();samples.append(dict(name=name,command=args,exit_code=p.returncode,elapsed_ms=(time.perf_counter()-started)*1000,peak_rss_bytes=usage.ru_maxrss*1024,memory_scope='wait4 child/reaped descendant peak RSS, not collective simultaneous RAM'))
 (E/'implementation-measurements.json').write_text(json.dumps(samples,indent=2));print(name,p.returncode,out[:1500]);assert p.returncode in allowed
(ROOT/'textcase').mkdir(exist_ok=True);(ROOT/'cmd/casecli').mkdir(parents=True,exist_ok=True)
(ROOT/'go.mod').write_text('module casecli\n\ngo 1.25.0\n')
(ROOT/'textcase/textcase.go').write_text('''package textcase
import "strings"
func Upper(text string) string { return strings.ToUpper(text) }
func Lower(text string) string { return strings.ToLower(text) }
''')
(ROOT/'cmd/casecli/main.go').write_text('''package main
import("casecli/textcase";"fmt";"os";"strings")
func main(){
 if len(os.Args)<3 { fmt.Fprintln(os.Stderr,"usage: casecli upper|lower TEXT");os.Exit(2) }
 input:=strings.Join(os.Args[2:]," ")
 switch os.Args[1] { case "upper": fmt.Println(textcase.Upper(input));case "lower":fmt.Println(textcase.Lower(input));default:fmt.Fprintln(os.Stderr,"unknown command");os.Exit(2) }
}
''')
(ROOT/'.dctx0/testing/go/adapter.go').write_text('''package main
import("casecli/textcase";"encoding/json";"fmt")
type caseAdapter struct{}
type caseSession struct{}
var adapter ProjectAdapter = caseAdapter{}
func(caseAdapter) Describe()(AdapterDescription,error){return AdapterDescription{ProtocolVersion:1,Subjects:[]SubjectDescription{
 {Subject:"CaseCli.Text.upper",Role:"operation",Artifact:"textcase/textcase.go",EntryPoint:"textcase.Upper"},
 {Subject:"CaseCli.Text.lower",Role:"operation",Artifact:"textcase/textcase.go",EntryPoint:"textcase.Lower"},
},Artifacts:[]string{"textcase/textcase.go","cmd/casecli/main.go"},Capabilities:[]string{"sessions","invocation","events"},Representations:map[string]string{"String":"Go UTF-8 string; Unicode simple case conversion"}},nil}
func(caseAdapter) Begin(string,TestContext)(TestSession,error){return caseSession{},nil}
func(caseSession) Close()error{return nil}
func(caseSession) Invoke(i Invocation)(Observation,error){
 if len(i.Arguments)!=1||i.Arguments[0].Kind!="string" {return Observation{},fmt.Errorf("one String argument required")}
 var text string;if err:=json.Unmarshal(i.Arguments[0].Value,&text);err!=nil{return Observation{},err}
 var result string;switch i.Subject {case "CaseCli.Text.upper":result=textcase.Upper(text);case "CaseCli.Text.lower":result=textcase.Lower(text);default:return Observation{},fmt.Errorf("unknown subject %s",i.Subject)}
 raw,err:=json.Marshal(result);if err!=nil{return Observation{},err};value:=Value{Kind:"string",Value:raw};return Observation{Kind:"returned",Value:&value},nil
}
''')
run('format',['gofmt','-w','textcase','cmd','.dctx0/testing/go/adapter.go'])
run('profile-agents',['dctx0','profile','go','create','--project',str(ROOT),'--agents'])
assert (ROOT/'AGENTS.md').is_file()
run('build-cli',['go','build','-o','casecli','./cmd/casecli'])
run('upper-cli',['./casecli','upper','Hello World'])
run('lower-cli',['./casecli','lower','Hello World'])
run('verify-good',['dctx0','verify','run','dctx0','--project',str(ROOT),'--no-adopt','--json'])
