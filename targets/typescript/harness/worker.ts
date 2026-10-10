import {readFile} from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { inspect } from 'node:util';
import type { ProjectAdapter,TestSession,Observation,Value } from './contract.ts';
const adapterModule = __ADAPTER_IMPORT__;
const subjects = new Set<string>(__SUBJECTS__);
const version=1, maxBytes=1048576;
const protocolWrite=process.stdout.write.bind(process.stdout);
for(const level of ['log','info','warn','error','debug'] as const){
 console[level]=(...values:unknown[])=>process.stderr.write(values.map(v=>typeof v==='string'?v:inspect(v)).join(' ')+'\n');
}
function send(message:unknown){
 const line=JSON.stringify({protocol_version:version,message});
 if(Buffer.byteLength(line)>maxBytes) throw new Error('Outgoing protocol message exceeds byte limit');
 protocolWrite(line+'\n');
}
function technical(id:number|null,category:string,error:unknown){
 const message=(error instanceof Error?error.message:String(error)).slice(0,4096);
 send({kind:'technical-error',id,category,message});
}
let session:TestSession|undefined;
let adapter:ProjectAdapter|undefined;
let events:{subject:string;payload:Value}[]=[];
let calls:{invocation:import('./contract.ts').Invocation;observation:Observation}[]=[];
let observesCalls=false;
let nextMock=1;
let epoch=0;
const pending=new Map<number,{resolve:(o:Observation)=>void;reject:(e:Error)=>void}>();
function rejectPending(reason:string){for(const waiter of pending.values())waiter.reject(new Error(reason));pending.clear();}
function keys(object:unknown,expected:string[]){
 if(!object||typeof object!=='object'||Array.isArray(object))throw new Error('Expected object');
 const actual=Object.keys(object);
 if(actual.length!==expected.length||actual.some(k=>!expected.includes(k)))throw new Error('Unexpected protocol fields');
}
async function getAdapter(){
 if(!adapter){const module=await import(adapterModule);adapter=module.adapter;
  if(!adapter||typeof adapter.describe!=='function'||typeof adapter.begin!=='function')throw new Error('Missing project adapter methods');}
 return adapter;
}
async function end(){
 const previous=session;session=undefined;events=[];calls=[];epoch++;
 rejectPending('Session ended before collaborator response');
 if(previous)await previous.close();
}
async function execute(id:number,request:any){
 const reply=(value:unknown)=>send({kind:'reply',id,reply:value});
 const requireSession=()=>{if(!session)throw new Error('No active testing session');return session;};
 switch(request.kind){
  case 'metrics':{
   keys(request,['kind']);let peak_rss_bytes:number|null=null;
   if(process.platform==='linux'){const status=await readFile('/proc/self/status','utf8');const match=status.match(/^VmHWM:\s+(\d+)\s+kB$/m);if(match)peak_rss_bytes=Number(match[1])*1024;}
   reply({kind:'measured',peak_rss_bytes,runtime_version:Bun.version,platform:process.platform});break;
  }
  case 'describe':keys(request,['kind']);reply({kind:'described',description:await(await getAdapter()).describe()});break;
  case 'begin':{
   keys(request,['kind','case_id']);if(typeof request.case_id!=='string'||!request.case_id)throw new Error('Invalid case ID');
   if(session)throw new Error('Previous session must be closed');events=[];calls=[];const ownEpoch=++epoch;
   observesCalls=(await(await getAdapter()).describe()).capabilities.includes('call-observation');
   session=await(await getAdapter()).begin(request.case_id,{
    collaborator(invocation){return new Promise<Observation>((resolve,reject)=>{
     if(epoch!==ownEpoch){reject(new Error('Session is no longer active'));return;}
     const request_id=nextMock++;pending.set(request_id,{resolve,reject});
     send({kind:'mock-call',request_id,invocation});
    });},
    emit(subject,payload){if(epoch!==ownEpoch)throw new Error('No active session for event');events.push({subject,payload});},
    recordCall(invocation,observation){if(epoch!==ownEpoch||!observesCalls)throw new Error('No active actual call observer');calls.push({invocation,observation});},
   });
   if(!session||typeof session.invoke!=='function'||typeof session.close!=='function'){session=undefined;throw new Error('Invalid project session');}
   reply({kind:'begun',case_id:request.case_id});break;
  }
  case 'invoke':{
   keys(request,['kind','invocation']);keys(request.invocation,['subject','arguments']);
   if(!subjects.has(request.invocation.subject))throw new Error('Invocation outside selected subjects');
   const active=requireSession();events=[];calls=[];
   // A rejected adapter promise is technical, never invented application evidence.
   const observation=await active.invoke(request.invocation);
   reply({kind:'invoked',observation,events,...(observesCalls?{calls}: {})});break;
  }
  case 'validate':{
   keys(request,['kind','subject','value']);if(!subjects.has(request.subject))throw new Error('Validation outside selected subjects');
   const active=requireSession();reply({kind:'value-checked',outcome:active.validate?await active.validate(request.subject,request.value):{kind:'unavailable',reason:'No production value factory/decoder described'}});break;
  }
  case 'observe':{
   keys(request,['kind','entities']);const active=requireSession();
   if(!active.observe)throw new Error('No actual state observer');
   if(!Array.isArray(request.entities)||request.entities.some((s:unknown)=>typeof s!=='string'||!subjects.has(s)))throw new Error('Observation outside selected subjects');
   reply({kind:'observed',states:await active.observe(request.entities)});break;
  }
  case 'end':keys(request,['kind']);await end();reply({kind:'ended'});break;
  case 'shutdown':keys(request,['kind']);await end();reply({kind:'bye'});process.stdin.destroy();break;
  default:throw new Error('Unknown worker request');
 }
}
let chain=Promise.resolve();
let stopped=false;
const input=createInterface({input:process.stdin,crlfDelay:Infinity});
input.on('line',(line:string)=>{
 let envelope:any;
 try{
  if(Buffer.byteLength(line)>maxBytes)throw new Error('Incoming protocol message exceeds byte limit');
  envelope=JSON.parse(line);keys(envelope,['protocol_version','id','request']);
  if(envelope.protocol_version!==version||!Number.isSafeInteger(envelope.id)||envelope.id<=0)throw new Error('Invalid protocol version/request ID');
  const request=envelope.request;
  if(request?.kind==='mock-reply'){
   keys(request,['kind','request_id','observation']);const waiter=pending.get(request.request_id);
   if(!waiter)throw new Error('Unknown collaborator response');pending.delete(request.request_id);waiter.resolve(request.observation);return;
  }
  if(stopped)throw new Error('Worker already shutting down');
  if(request?.kind==='shutdown')stopped=true;
  chain=chain.then(async()=>{
   try{await execute(envelope.id,request);}catch(error){
    technical(envelope.id,'adapter-or-request',error);
    try{await end();}catch(cleanup){technical(envelope.id,'cleanup',cleanup);}
   }
  });
 }catch(error){technical(envelope?.id??null,'protocol',error);rejectPending('Protocol violation');}
});
input.on('close',()=>{rejectPending('Worker input closed');void chain.finally(async()=>{try{await end();}catch(error){technical(null,'cleanup',error);}});});
