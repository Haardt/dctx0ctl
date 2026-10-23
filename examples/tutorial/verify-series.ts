import {strict as assert} from 'node:assert';
import {Database} from 'bun:sqlite';
import {createHash} from 'node:crypto';
import {mkdtemp,readFile,readdir,rename} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';

const root=resolve(import.meta.dir,'../..'),binary=process.env.DCTX0_BIN??resolve(root,'target/release/dctx0');
const inventory=await Bun.file(resolve(import.meta.dir,'features.json')).json();
const requested=process.argv.slice(2).map(Number);
const scratch=await mkdtemp(join(tmpdir(),'dctx0-tutorial-series-'));
assert.ok(requested.every(number=>Number.isInteger(number)&&number>=1&&number<=16),'Select steps 1..16');
async function execute(command:string[],cwd:string,output:string):Promise<string>{
  const child=Bun.spawn(command,{cwd,env:{...process.env,PATH:'/home/held/.cargo/bin:'+(process.env.PATH??'')},stdout:'pipe',stderr:'pipe'});
  const [stdout,stderr,code]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
  await Bun.write(output,stdout+stderr);assert.equal(code,0,command.join(' ')+'\n'+stderr);return stdout;
}
function coverage(text:string){
  return text.split('end_of_record').filter(section=>section.includes('SF:')).map(section=>Object.fromEntries(section.trim().split('\n').filter(line=>/^(SF|FNF|FNH|LF|LH):/.test(line)).map(line=>[line.slice(0,line.indexOf(':')),line.slice(line.indexOf(':')+1)])));
}
async function production(directory:string):Promise<string[]>{
  const entries=await readdir(resolve(directory,'src'));
  return [...entries.filter(name=>name.endsWith('.ts')&&!name.endsWith('.test.ts')).map(name=>'src/'+name),...(await Bun.file(resolve(directory,'server.ts')).exists()?['server.ts']:[])];
}
const results=[];
for(const step of inventory.steps.filter((step:any)=>!requested.length||requested.includes(step.number))){
  const project=resolve(root,'examples',step.directory),prefix=resolve(import.meta.dir,'validation','step-'+step.number);
  console.log('Step '+step.number+': install, typecheck, all production tests and native complete');
  await execute(['bun','install','--frozen-lockfile'],project,prefix+'-install.log');
  await execute(['bun','run','typecheck'],project,prefix+'-types.log');
  await execute(['bun','run','test','--coverage','--coverage-reporter=lcov','--coverage-dir='+join(scratch,String(step.number))],project,prefix+'-tests.log');
  const records=coverage(await readFile(join(scratch,String(step.number),'lcov.info'),'utf8'));
  const files=await production(project);
  for(const file of files){
    const record=records.find(record=>record.SF===file);
    assert.ok(record,'Missing production coverage: '+step.directory+'/'+file);
    assert.equal(record.LH,record.LF,'Uncovered production lines: '+file);
    assert.equal(record.FNH,record.FNF,'Uncovered production functions: '+file);
  }
  if(await Bun.file(resolve(project,'.dctx0/testing/typescript/resource-observer.test.ts')).exists())await execute(['bun','test','./.dctx0/testing/typescript/resource-observer.test.ts'],project,prefix+'-observer.log');
  const complete=JSON.parse(await execute([binary,'verify','complete','dctx0','--project','.','--json'],project,prefix+'-complete.log'));
  assert.equal(complete.result.status,'PASS');assert.equal(complete.open_tasks,0);assert.equal(complete.non_pass_tasks,0);
  const saved=JSON.parse(await execute([binary,'verify','show','dctx0',complete.native_run,'--json','--full'],project,'/tmp/dctx0-tutorial-saved-'+step.number+'.json'));
  const run=saved.run;assert.equal(run.required_cases.length,run.cases.length);
  assert.ok(run.cases.every((entry:any)=>entry.result.status==='PASS'),'Unexecuted or failing native case');
  const metricsArtifact=run.artifacts.find((artifact:any)=>artifact.media_type==='application/vnd.dctx0.runtime-metrics+json');
  assert.ok(metricsArtifact,'Missing actual worker memory evidence');
  const database=new Database(resolve(project,'dctx0/.dctx0ctl/state.sqlite3'),{readonly:true});
  let nativeMetrics;
  try {
    const row=database.query('SELECT bytes FROM verification_artifacts WHERE hash=?').get(metricsArtifact.hash) as {bytes:Uint8Array}|null;
    assert.ok(row,'Missing worker metrics artifact');
    assert.equal(createHash('sha256').update(row.bytes).digest('hex'),metricsArtifact.hash);
    nativeMetrics=JSON.parse(new TextDecoder().decode(row.bytes));
    assert.ok(nativeMetrics.peak_rss_bytes>0,'Native worker peak RSS is unavailable');
  }finally{database.close();}
  const hashes=Object.fromEntries(await Promise.all([...files,...await Array.fromAsync(new Bun.Glob('dctx0/*.dctx0').scan({cwd:project}))].map(async path=>[path,createHash('sha256').update(await readFile(resolve(project,path))).digest('hex')])));
  const report={schema_version:1,step:step.number,status:'current-source-pass',complete,production_coverage:records,source_hashes:hashes,recorded_cases:run.cases.length,case_failures:0,case_unknown:0,open_obligations:run.open_obligations,native_runtime_metrics:nativeMetrics,native_elapsed_us:run.elapsed_us,memory_scope:'Actual worker post-exec Linux VmHWM; cumulative peak for this worker, excluding Rust and separate compilers',remaining:['Final coupled release/performance acceptance']};
  const temporary=prefix+'.json.tmp';await Bun.write(temporary,JSON.stringify(report,null,2)+'\n');await rename(temporary,prefix+'.json');
  results.push({step:step.number,cases:run.cases.length,tasks:complete.tasks});console.log('Step '+step.number+': PASS ('+run.cases.length+' cases, '+complete.tasks+' tasks)');
}
await execute(['bun','test','examples/tutorial/agent-step.test.ts','examples/tutorial/reset-agent.test.ts','examples/tutorial/check-features.test.ts'],root,resolve(import.meta.dir,'validation','scripts-tests.log'));
await execute(['bun','examples/tutorial/check-features.ts'],root,resolve(import.meta.dir,'validation','features.log'));
console.log(JSON.stringify({status:'PASS',steps:results,scope:requested.length?'selected steps':'complete series',performance:'Final release/performance acceptance is separate and remains required'},null,2));
