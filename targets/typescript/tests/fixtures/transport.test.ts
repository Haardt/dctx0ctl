import {test,expect,afterAll} from 'bun:test';
import {mkdirSync,writeFileSync,readFileSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {basename,isAbsolute} from 'node:path';
import type {Subprocess} from 'bun';
const protocolSamples:unknown[]=[];
const collectCoverage=process.env.DCTX0_NATIVE_COVERAGE==='1';
let coverageWorker=0;
const instrumentationHeaders:string[]=[];
const coverageReports:{directory:string;raw:string|null}[]=[];
afterAll(async()=>{
 if(!collectCoverage)return;
 const sources=new Map<string,{sha256:string;lines:Map<number,number>;functions:Map<string,number>;function_samples:{report:string;found:number;hit:number|null}[]}>();
 for(const report of coverageReports){
  expect(report.raw).not.toBeNull();let source:string|undefined;
  for(const line of report.raw!.split('\n')){
   if(line.startsWith('SF:')){
    source=line.slice(3);if(!['worker.ts','scaffold-worker.ts','contract.ts'].includes(source.split(/[/\\]/).at(-1)!)){source=undefined;continue;}
    if(!sources.has(source))sources.set(source,{sha256:createHash('sha256').update(readFileSync(source)).digest('hex'),lines:new Map(),functions:new Map(),function_samples:[]});
   }else if(source&&line.startsWith('DA:')){const [number,hits]=line.slice(3).split(',').map(Number);const counts=sources.get(source)!.lines;counts.set(number,(counts.get(number)??0)+hits);
   }else if(source&&line.startsWith('FNF:')){sources.get(source)!.function_samples.push({report:report.directory,found:Number(line.slice(4)),hit:null});
   }else if(source&&line.startsWith('FNH:')){const sample=sources.get(source)!.function_samples.at(-1);if(sample)sample.hit=Number(line.slice(4));
   }else if(source&&line.startsWith('FNDA:')){const index=line.indexOf(',',5);const hits=Number(line.slice(5,index)),name=line.slice(index+1);const counts=sources.get(source)!.functions;counts.set(name,(counts.get(name)??0)+hits);}
  }
 }
 const raw=await Bun.file('native-performance.json').json();
 raw.native_helper_coverage={schema_version:1,scope:'Generated Bun worker variants and testing contract; transport conformance only',instrumentation:'bun test --coverage --coverage-reporter lcov',instrumentation_headers:instrumentationHeaders,reports:coverageReports,sources:[...sources].map(([source,counts])=>({source,sha256:counts.sha256,lines:counts.lines.size,covered_lines:[...counts.lines.values()].filter(hits=>hits>0).length,functions:counts.functions.size||null,covered_functions:counts.functions.size?[...counts.functions.values()].filter(hits=>hits>0).length:null,function_samples:counts.function_samples,function_coverage_status:counts.functions.size?'identified functions':'unavailable: Bun reports per-run totals without function identities; totals can vary between executions',uncovered_lines:[...counts.lines].filter(([,hits])=>hits===0).map(([line])=>line)}))};
 raw.protocolSamples=protocolSamples;
 raw.samples_instrumentation='Native worker coverage instrumentation; not release benchmark evidence';
 await Bun.write('native-performance.json',JSON.stringify(raw));
});
const samples:Record<string,{elapsed_ms:number;peak_rss_bytes:number|null;runtime_version:string}>={};
async function measure(c:Client,name:string,start:number){const elapsed_ms=performance.now()-start;const metrics=(await c.request({kind:'metrics'})).reply;if(process.platform==='linux')expect(metrics.peak_rss_bytes).toBeGreaterThan(0);else expect(metrics.peak_rss_bytes).toBeNull();samples[name]={elapsed_ms,peak_rss_bytes:metrics.peak_rss_bytes,runtime_version:metrics.runtime_version};}

class Client {
 process:Subprocess<'pipe','pipe','pipe'>;
 reader:ReadableStreamDefaultReader<Uint8Array>;buffer='';id=0;
 logs:Promise<string>;coverageDirectory?:string;headerSeen=false;protocolSeen=false;
 constructor(worker='worker.ts'){
  let command=[process.execPath,'run',worker];
  if(collectCoverage){
   const id=++coverageWorker,wrapper=`coverage-worker-${id}.test.ts`;
   this.coverageDirectory=`native-coverage/worker-${id}`;mkdirSync(this.coverageDirectory,{recursive:true});
   writeFileSync(wrapper,`import {test} from 'bun:test'; test('actual generated worker',async()=>{const worker=await import(${JSON.stringify('./'+worker)});await worker.finished;},30000);`);
   command=[process.execPath,'test',wrapper,'--coverage','--coverage-reporter','lcov','--coverage-dir',this.coverageDirectory];
  }
  this.process=Bun.spawn(command,{stdin:'pipe',stdout:'pipe',stderr:'pipe'});
  this.reader=this.process.stdout.getReader();this.logs=new Response(this.process.stderr).text();
 }
 send(request:unknown){const id=++this.id;this.process.stdin.write(JSON.stringify({protocol_version:1,id,request})+'\n');return id;}
 async next():Promise<any>{
  while(!this.buffer.includes('\n')){const data=await this.reader.read();if(data.done)throw new Error('Incomplete response');this.buffer+=new TextDecoder().decode(data.value);}
  const index=this.buffer.indexOf('\n'),line=this.buffer.slice(0,index);this.buffer=this.buffer.slice(index+1);if(collectCoverage&&!this.protocolSeen&&!this.headerSeen&&line.startsWith('bun test v')){this.headerSeen=true;instrumentationHeaders.push(line);return await this.next();}const envelope=JSON.parse(line);this.protocolSeen=true;protocolSamples.push(envelope);return envelope.message;
 }
 async request(request:unknown){const id=this.send(request);const response=await this.next();expect(response.id).toBe(id);return response;}
 async stop(){
  if(this.coverageDirectory){
   if(this.process.exitCode===null){try{this.process.stdin.end();}catch{this.process.kill();}}let timer:ReturnType<typeof setTimeout>|undefined;
   const finished=await Promise.race([this.process.exited.then(()=>true),new Promise<boolean>(resolve=>{timer=setTimeout(()=>resolve(false),3000);})]);
   if(timer)clearTimeout(timer);if(!finished){this.process.kill();await this.process.exited;}
   const file=this.coverageDirectory+'/lcov.info';coverageReports.push({directory:this.coverageDirectory,raw:existsSync(file)?readFileSync(file,'utf8'):null});
  }else{this.process.kill();await this.process.exited;}
 }
}
const integer=(value:number)=>({kind:'integer',value:String(value)});
const call=(arg:unknown)=>({kind:'invoke',invocation:{subject:'operation',arguments:[arg]}});
test('actual source, isolated state, events, central mock responses and cleanup',async()=>{
 const bootStarted=performance.now();const c=new Client();try{
  expect((await c.request({kind:'describe'})).reply.description.protocol_version).toBe(1);
  expect((await c.request({kind:'begin',case_id:'first'})).reply.kind).toBe('begun');await measure(c,'initialization',bootStarted);
  let started=performance.now();let reply=(await c.request(call(integer(2)))).reply;await measure(c,'operation',started);expect(reply.observation.value).toEqual(integer(2));expect(reply.events[0].payload).toEqual(integer(2));
  expect((await c.request({kind:'observe',entities:['Entity']})).reply.states.Entity[0].value.count).toEqual(integer(2));
  started=performance.now();const id=c.send(call({kind:'string',value:'mock'}));const mock=await c.next();expect(mock.kind).toBe('mock-call');expect(mock.invocation.arguments).toEqual([integer(2)]);
  c.send({kind:'mock-reply',request_id:mock.request_id,observation:{kind:'returned',value:integer(7)}});
  reply=await c.next();expect(reply.id).toBe(id);expect(reply.reply.observation.value).toEqual(integer(7));await measure(c,'mock-transport',started);
  expect((await c.request({kind:'validate',subject:'value',value:integer(1)})).reply.outcome.kind).toBe('accepted');
  expect((await c.request({kind:'validate',subject:'value',value:{kind:'string',value:'bad'}})).reply.outcome.kind).toBe('rejected');
  started=performance.now();expect((await c.request({kind:'invoke',invocation:{subject:'HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner',arguments:[{kind:'list',value:['00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000001'].map(value=>({kind:'string',value}))},{kind:'list',value:['00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002'].map(value=>({kind:'string',value}))}]}})).reply.observation.value.value.value).toBe('00000000-0000-4000-8000-000000000001');await measure(c,'cinema-pure-call',started);await Bun.write('native-performance.json',JSON.stringify({schema_version:1,scope:'Bun transport conformance; not full Cinema verification',samples,protocolSamples}));
  await c.request({kind:'end'});await c.request({kind:'begin',case_id:'fresh'});
  expect((await c.request(call(integer(1)))).reply.observation.value).toEqual(integer(1));
  expect((await c.request(call({kind:'string',value:'exception'}))).reply.observation.kind).toBe('threw');
  expect((await c.request({kind:'shutdown'})).reply.kind).toBe('bye');
  expect(await c.process.exited).toBe(0);expect(await c.logs).toContain('actual application output');
 }finally{await c.stop();}
},{timeout:10000});
test('scaffolds, initialization, missing session, unsupported scope and cleanup never report success',async()=>{
 for(const [worker,requests] of [
  ['scaffold-worker.ts',[{kind:'describe'},{kind:'begin',case_id:'unimplemented'}]],
  ['worker.ts',[{kind:'begin',case_id:'init-error'}]],
  ['worker.ts',[call(integer(1))]],
  ['worker.ts',[{kind:'begin',case_id:'cleanup-error'},{kind:'end'}]],
  ['worker.ts',[{kind:'begin',case_id:'normal'},{kind:'invoke',invocation:{subject:'unselected',arguments:[]}}]],
  ['worker.ts',[{kind:'begin',case_id:'normal'},call({kind:'string',value:'adapter-error'})]],
 ] as const){
  const c=new Client(worker);try{let response;for(const request of requests)response=await c.request(request);expect(response.kind).toBe('technical-error');expect(response).not.toHaveProperty('PASS');}finally{await c.stop();}
 }
},{timeout:10000});
test('bad versions, corrupt frames, unexpected success flags and mock replies are technical failures',async()=>{
 for(const text of ['{','{"protocol_version":2,"id":1,"request":{"kind":"describe"}}','{"protocol_version":1,"id":1,"request":{"kind":"describe","PASS":true}}','{"protocol_version":1,"id":1,"request":{"kind":"mock-reply","request_id":999,"observation":{"kind":"returned","value":{"kind":"unit"}}}}']){
  const c=new Client();try{c.process.stdin.write(text+'\n');expect((await c.next()).kind).toBe('technical-error');}finally{await c.stop();}
 }
},{timeout:10000});
test('EOF releases a pending collaborator and closes its session',async()=>{
 const c=new Client();try{await c.request({kind:'begin',case_id:'eof'});c.send(call({kind:'string',value:'mock'}));expect((await c.next()).kind).toBe('mock-call');c.process.stdin.end();expect((await c.next()).reply.observation.kind).toBe('threw');expect(await c.process.exited).toBe(0);}finally{await c.stop();}
},{timeout:10000});

test('recorded observations retain their value at the actual observation time',async()=>{
 const c=new Client();try{
  await c.request({kind:'begin',case_id:'snapshot'});
  const response=(await c.request(call({kind:'string',value:'mutable-observation'}))).reply;
  expect(response.observation.value).toEqual(integer(1));
  expect(response.resources).toEqual([{resource:'Counter',access:'write'}]);
  expect(response.events.find((event:any)=>event.subject==='Snapshot').payload).toEqual(integer(1));
  await c.request({kind:'end'});
  await c.request({kind:'begin',case_id:'fresh-resource-observation'});
  expect((await c.request(call(integer(1)))).reply.resources).toEqual([]);
 }finally{await c.stop();}
},{timeout:10000});

test('observation overflow is explicit and a new session starts with a fresh budget',async()=>{
 const c=new Client();try{
  await c.request({kind:'begin',case_id:'observation-overflow'});
  const failed=await c.request(call({kind:'string',value:'observation-limit'}));
  expect(failed.kind).toBe('technical-error');
  expect(failed.message).toContain('Observation budget exceeded');
  expect((await c.request(call({kind:'string',value:'caught-observation-limit'}))).kind).toBe('technical-error');
  await c.request({kind:'end'});
  await c.request({kind:'begin',case_id:'fresh-budget'});
  expect((await c.request(call(integer(1)))).reply.observation.value).toEqual(integer(1));
 }finally{await c.stop();}
},{timeout:10000});

test('native dependency resolution preserves import and require export conditions',async()=>{
 const {mkdir}=await import('node:fs/promises');
 await mkdir('node_modules/conditional-example',{recursive:true});
 await Bun.write('node_modules/conditional-example/package.json',JSON.stringify({name:'conditional-example',exports:{'.':{import:'./imported.js',require:'./required.cjs'}}}));
 await Bun.write('node_modules/conditional-example/imported.js','throw Error("resolution must not execute this module");');
 await Bun.write('node_modules/conditional-example/required.cjs','throw Error("resolution must not execute this module");');
 const c=new Client();try{
  const importer=fileURLToPath(new URL('./adapter.ts',import.meta.url));
  const imported=await c.request({kind:'resolve-dependency',mode:'import',specifier:'conditional-example',importer});
  const required=await c.request({kind:'resolve-dependency',mode:'require',specifier:'conditional-example',importer});
  expect(imported.reply.kind).toBe('dependency-resolved');
  expect(isAbsolute(imported.reply.path)).toBe(true);expect(basename(imported.reply.path)).toBe('imported.js');
  expect(required.reply.kind).toBe('dependency-resolved');expect(isAbsolute(required.reply.path)).toBe(true);expect(basename(required.reply.path)).toBe('required.cjs');
  for(const mode of ['import','require']){
   expect((await c.request({kind:'resolve-dependency',mode,specifier:'path',importer})).reply.path).toBe('node:path');
   expect((await c.request({kind:'resolve-dependency',mode,specifier:'node:fs',importer})).reply.path).toBe('node:fs');
   expect((await c.request({kind:'resolve-dependency',mode,specifier:'bun:sqlite',importer})).reply.path).toBe('bun:sqlite');
   expect((await c.request({kind:'resolve-dependency',mode,specifier:'bun',importer})).reply.path).toBe('bun');
   for(const specifier of ['node:dctx0_missing_builtin','bun:dctx0_missing_builtin']){
    expect((await c.request({kind:'resolve-dependency',mode,specifier,importer})).reply.kind).toBe('dependency-unresolved');
   }
  }
  expect((await c.request({kind:'resolve-dependency',mode:'import',specifier:'missing-package',importer})).reply.kind).toBe('dependency-unresolved');
  expect((await c.request({kind:'describe'})).reply.kind).toBe('described');
 }finally{await c.stop();}
},{timeout:10000});


test('adapter and protocol faults are reported only after asynchronous session cleanup',async()=>{
 for(const kind of ['adapter','protocol','close-error']){
  const caseId='cleanup-order-'+kind,marker=caseId+'.closed';
  const c=new Client();try{
   await c.request({kind:'begin',case_id:caseId});
   let response;
   if(kind!=='protocol')response=await c.request(call({kind:'string',value:'adapter-error'}));
   else {c.process.stdin.write('{broken}\n');response=await c.next();}
   expect(response.kind).toBe('technical-error');
   if(kind==='close-error')expect((await c.next()).category).toBe('cleanup');
   expect(existsSync(marker)).toBe(true);
   expect(readFileSync(marker,'utf8')).toBe('closed');
  }finally{await c.stop();}
 }
},{timeout:10000});


test('explicit infrastructure causes remain distinct from unclassified errors',async()=>{
 for(const [case_id,category] of [['database-unavailable','environment'],['observer-missing','adapter'],['init-error','unclassified']]){
  const c=new Client();try{
   const response=await c.request({kind:'begin',case_id});
   expect(response.kind).toBe('technical-error');
   expect(response.category).toBe(category);
  }finally{await c.stop();}
 }
});


test('result conversion failures have adapter provenance',async()=>{
 const c=new Client();try{
  await c.request({kind:'begin',case_id:'normal'});
  const response=await c.request(call({kind:'string',value:'encode-error'}));
  expect(response.kind).toBe('technical-error');
  expect(response.category).toBe('adapter');
 }finally{await c.stop();}
});
