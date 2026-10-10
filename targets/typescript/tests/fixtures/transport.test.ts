import {test,expect} from 'bun:test';
import type {Subprocess} from 'bun';
const protocolSamples:unknown[]=[];
const samples:Record<string,{elapsed_ms:number;peak_rss_bytes:number;runtime_version:string}>={};
async function measure(c:Client,name:string,start:number){const elapsed_ms=performance.now()-start;const metrics=(await c.request({kind:'metrics'})).reply;expect(metrics.peak_rss_bytes).toBeGreaterThan(0);samples[name]={elapsed_ms,peak_rss_bytes:metrics.peak_rss_bytes,runtime_version:metrics.runtime_version};}

class Client {
 process:Subprocess<'pipe','pipe','pipe'>;
 reader:ReadableStreamDefaultReader<Uint8Array>;buffer='';id=0;
 logs:Promise<string>;
 constructor(worker='worker.ts'){
  this.process=Bun.spawn([process.execPath,'run',worker],{stdin:'pipe',stdout:'pipe',stderr:'pipe'});
  this.reader=this.process.stdout.getReader();this.logs=new Response(this.process.stderr).text();
 }
 send(request:unknown){const id=++this.id;this.process.stdin.write(JSON.stringify({protocol_version:1,id,request})+'\n');return id;}
 async next():Promise<any>{
  while(!this.buffer.includes('\n')){const data=await this.reader.read();if(data.done)throw new Error('Incomplete response');this.buffer+=new TextDecoder().decode(data.value);}
  const index=this.buffer.indexOf('\n'),line=this.buffer.slice(0,index);this.buffer=this.buffer.slice(index+1);const envelope=JSON.parse(line);protocolSamples.push(envelope);return envelope.message;
 }
 async request(request:unknown){const id=this.send(request);const response=await this.next();expect(response.id).toBe(id);return response;}
 async stop(){this.process.kill();await this.process.exited;}
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
