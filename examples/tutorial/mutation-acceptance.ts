import {strict as assert} from 'node:assert';
import {cp,mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';

const root=resolve(import.meta.dir,'../..'),binary=process.env.DCTX0_BIN??resolve(root,'target/release/dctx0');
const inventory=await Bun.file(resolve(import.meta.dir,'features.json')).json();
const scratch=await mkdtemp(join(tmpdir(),'dctx0-tutorial-mutations-'));
type Mutation={step:number,name:string,file:string,from:string,to:string,subject:string,replay?:boolean};
const mutations:Mutation[]=[
  {step:1,name:'wrong increment',file:'src/board.ts',from:'return value + 1n;',to:'return value + 0n;',subject:'Tutorial.Board.Actions.increment'},
  {step:2,name:'loosened title boundary',file:'src/board.ts',from:'text.length > 16',to:'text.length > 17',subject:'Tutorial.Board.Title'},
  {step:3,name:'loosened float boundary',file:'src/data.ts',from:'value > 10',to:'value > 11',subject:'Tutorial.Board.Measure'},
  {step:4,name:'wrong arithmetic',file:'src/expressions.ts',from:'(left + right) * 2n - left',to:'(left + right) * 3n - left',subject:'Tutorial.Board.Actions.score'},
  {step:5,name:'lost collection mapping',file:'src/collections.ts',from:'entry=>entry*2n',to:'entry=>entry*3n',subject:'Tutorial.Board.Actions.doubled'},
  {step:6,name:'empty real read surface',file:'src/entities.ts',from:'return this.tasks.map(row=>taskSummary({id:row.id,title:row.title}));',to:'return [];',subject:'Tutorial.Board.Tasks.list'},
  {step:7,name:'consumer ignores actual reader result',file:'src/catalog.ts',from:'return this.reader.lookup(taskId(id));',to:"taskId(id);return {tag:'none'};",subject:'Tutorial.Board.Catalog.Public.find'},
  {step:8,name:'consumer skips mock collaborator',file:'src/catalog.ts',from:'return await this.reader.lookup(taskId(id));',to:"taskId(id);return {tag:'none'};",subject:'Tutorial.Board.Catalog.Public.find'},
  {step:9,name:'missing actual event',file:'src/catalog.ts',from:'this.publish(taskViewed({taskId:key}));',to:'void key;',subject:'Tutorial.Board.Catalog.Public.requireTask'},
  {step:10,name:'invalid clock value',file:'src/diagnostics.ts',from:'return this.source().toISOString();',to:"return 'invalid-time';",subject:'Tutorial.Board.Diagnostics.timestamp'},
  {step:11,name:'replacement without state change',file:'src/entities.ts',from:'this.tasks[index]=row;return task(row);',to:'return task(row);',subject:'Tutorial.Board.Catalog.TaskWriter.replace'},
  {step:12,name:'lifecycle status remains Open',file:'src/entities.ts',from:'this.tasks[index]=row;return task(row);',to:"this.tasks[index]={...row,status:'Open'};return task(row);",subject:'Tutorial.Board.TaskLifecycle',replay:true},
  {step:13,name:'external readiness ignored',file:'src/external.ts',from:'return (await this.query()).ready;',to:'await this.query();return false;',subject:'Tutorial.Board.Connectivity.ready'},
  {step:14,name:'fragment result is constant',file:'src/copy.ts',from:'return title(value);',to:"title(value);return 'Changed';",subject:'Tutorial.Board.Copy.title'},
  {step:16,name:'HTTP insertion silently upserts',file:'src/http.ts',from:'board.insert(row)',to:'board.save(row)',subject:'Tutorial.Board.Http.insert',replay:true},
  {step:16,name:'HTTP replacement silently inserts',file:'src/http.ts',from:'board.replace(row)',to:'board.save(row)',subject:'Tutorial.Board.Http.replace'},
  {step:1,name:'adapter replays a golden answer',file:'.dctx0/testing/typescript/adapter.ts',from:'String(increment(integer(invocation.arguments[0])))',to:"'1'",subject:'Tutorial.Board.Actions.increment'},
];
const results:any[]=[];
async function command(args:string[],project:string,accepted:number[]=[0]){
  const child=Bun.spawn([binary,...args],{cwd:project,stdout:'pipe',stderr:'pipe',env:{...process.env,PATH:'/home/held/.cargo/bin:'+(process.env.PATH??'')}});
  const [out,error,code]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
  assert.ok(accepted.includes(code),args.join(' ')+'\n'+out+'\n'+error);
  if(out.trim())return JSON.parse(out);
  assert.equal(code,1,'A successful command must return its report');
  return {exit_code:code,error};
}
async function copy(step:number,name:string){
  const reference=resolve(root,'examples',inventory.steps.find((entry:any)=>entry.number===step).directory),project=join(scratch,name);
  for(const path of ['src','dctx0','.dctx0/testing','package.json','bun.lock','tsconfig.json'])await cp(join(reference,path),join(project,path),{recursive:true,filter:path=>!path.includes('/.dctx0ctl/')});
  // History belongs to this isolated copy. Original application sources remain untouched.
  const child=Bun.spawn(['bun','install','--frozen-lockfile'],{cwd:project,stdout:'ignore',stderr:'pipe'});
  const error=await new Response(child.stderr).text();assert.equal(await child.exited,0,error);return project;
}
for(const mutation of mutations){
  const project=await copy(mutation.step,'step-'+mutation.step+'-'+results.length),file=join(project,mutation.file);
  const original=await readFile(file,'utf8');assert.ok(original.includes(mutation.from),'Mutation no longer matches '+mutation.name);
  await writeFile(file,original.replace(mutation.from,mutation.to));
  await command(['check','dctx0','--json'],project);
  const native=(await command(['verify','run','dctx0','--project','.','--subject',mutation.subject,'--no-adopt','--json'],project,[0,1])).evidence;
  const failing=native.run.cases.filter((entry:any)=>entry.result.status==='FAIL');
  assert.ok(failing.length,'Mutation was not detected by a real failed case: '+mutation.name);
  const item:any={step:mutation.step,name:mutation.name,subject:mutation.subject,result:'detected',native_run:native.id,cases:native.run.cases.length,counterexample:failing[0]};
  if(mutation.replay){
    const failed=await command(['verify','replay','dctx0',native.id,'--case',failing[0].id,'--project','.','--json'],project,[0,1]);
    assert.equal(failed.case_result.status,'FAIL','Replay lost the counterexample');
    await writeFile(file,original);
    const fixed=await command(['verify','replay','dctx0',native.id,'--case',failing[0].id,'--project','.','--json'],project,[0,1]);
    assert.equal(fixed.case_result.status,'PASS','Replay did not execute the restored actual code');
    assert.ok(fixed.changes.some((entry:any)=>entry.component.startsWith('implementation:')),'Changed implementation must be reported');
    item.replay={failing:{result:failed.case_result,case:failed.replayed_case,changes:failed.changes},restored:{result:fixed.case_result,case:fixed.replayed_case,changes:fixed.changes},preserves_input:true};
  }
  results.push(item);console.log('Detected: '+mutation.name);
  await Bun.write(resolve(import.meta.dir,'validation/mutations.json'),JSON.stringify({schema_version:1,status:'incomplete',isolated_workspaces:scratch,results},null,2)+'\n');
}
// Manual policy evidence is tested separately; it cannot overrule native failures.
for(const rejection of ['request-changes','high-finding','missing-review','stale-source']){
  const project=await copy(15,'review-'+rejection),path=join(project,'.dctx0/testing/typescript/inspection-notes.json');
  const notes=JSON.parse(await readFile(path,'utf8')),review=notes.subjects['Tutorial.Board.BoundaryReview'];
  if(rejection==='request-changes')review.review.approved=false;
  if(rejection==='high-finding')review.review.findings.push({id:'unresolved-boundary',severity:'high',description:'Explicit test rejection of the release policy'});
  if(rejection==='missing-review')delete notes.subjects['Tutorial.Board.BoundaryReview'];
  if(rejection==='stale-source')await writeFile(join(project,'src/board.ts'),await readFile(join(project,'src/board.ts'),'utf8')+'\n// Changed after inspection\n');
  await writeFile(path,JSON.stringify(notes,null,2)+'\n');
  const report=await command(['verify','complete','dctx0','--project','.','--json'],project,[1]);
  assert.ok(report.error,'Rejected completion must report its specific reason');
  assert.match(report.error,rejection==='stale-source'?/New source inspection required: src\/board.ts/:/Effective assessment incomplete/,'An unrelated technical error is not policy acceptance');
  const run=report.error.match(/inspect run ([a-f0-9]{64})/)?.[1];
  if(run){
    const saved=await command(['verify','show','dctx0',run,'--json'],project);
    assert.notEqual(saved.run.result.status,'PASS');
    report.effective_result=saved.run.result;report.open_obligations=saved.run.open_obligations;
  }
  results.push({step:15,name:rejection,result:'rejected',report});console.log('Rejected: '+rejection);
}
await Bun.write(resolve(import.meta.dir,'validation/mutations.json'),JSON.stringify({schema_version:1,status:'PASS',basis:'Isolated mutations of actual production/adapter code and source-bound review evidence; central Rust assessment and saved-input replay',isolated_workspaces:scratch,results},null,2)+'\n');
console.log('PASS: '+results.length+' negative acceptance checks');
