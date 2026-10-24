import {installModuleObserver,moduleObserverEvidence} from './module-observer.ts';
import {readFile} from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { inspect } from 'node:util';
import {resolve,dirname} from 'node:path';
import {createRequire,isBuiltin} from 'node:module';
import {AdapterError,EnvironmentError} from './contract.ts';
import type { ProjectAdapter,TestSession,Observation,Value } from './contract.ts';
const adapterModule = __ADAPTER_IMPORT__;
const subjects = new Set<string>(__SUBJECTS__);
const version=1, maxBytes=67108864;
const protocolOutput=Bun.stdout.writer();
for(const level of ['log','info','warn','error','debug'] as const){
 console[level]=(...values:unknown[])=>process.stderr.write(values.map(v=>typeof v==='string'?v:inspect(v)).join(' ')+'\n');
}
function send(message:unknown){
 const line=JSON.stringify({protocol_version:version,message});
 if(Buffer.byteLength(line)>maxBytes) throw new Error('Outgoing protocol message exceeds byte limit');
 protocolOutput.write(line+'\n');void protocolOutput.flush();
}
async function flushProtocol(){await protocolOutput.flush();}
let resolveFinished:()=>void;
export const finished=new Promise<void>(resolve=>{resolveFinished=resolve;});
function technical(id:number|null,category:string,error:unknown){
 const message=(error instanceof Error?error.message:String(error)).slice(0,4096);
 send({kind:'technical-error',id,category,message});
}
let session:TestSession|undefined;
let adapter:ProjectAdapter|undefined;
let events:{subject:string;payload:Value}[]=[];
let calls:{invocation:import('./contract.ts').Invocation;observation:Observation}[]=[];
let observesCalls=false;
let observesResources=false;
let resources:import('./contract.ts').ResourceObservation[]=[];
let observationBytes=0;
let observationCount=0;
let observationOverflow=false;
function recordObservation<T>(value:T):T {
 const bytes=Buffer.byteLength(JSON.stringify(value));
 if(observationCount>=10000||bytes>maxBytes-observationBytes){observationOverflow=true;throw new Error('Observation budget exceeded');}
 observationBytes+=bytes;observationCount++;
 return structuredClone(value);
}
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
 if(!adapter){installModuleObserver();const module=await import(adapterModule);adapter=module.adapter;
  if(!adapter||typeof adapter.describe!=='function'||typeof adapter.begin!=='function')throw new Error('Missing project adapter methods');}
 return adapter;
}
async function end(){
 const previous=session;session=undefined;events=[];calls=[];resources=[];observationBytes=0;observationCount=0;observationOverflow=false;epoch++;
 rejectPending('Session ended before collaborator response');
 if(previous)await previous.close();
}
async function execute(id:number,request:any){
 const reply=(value:unknown)=>send({kind:'reply',id,reply:value});
 const requireSession=()=>{if(!session)throw new Error('No active testing session');return session;};
 switch(request.kind){
  case 'observe-dependencies':{
   keys(request,['kind']);reply({kind:'dependencies-observed',evidence:moduleObserverEvidence()});break;
  }
  case 'metrics':{
   keys(request,['kind']);let peak_rss_bytes:number|null=null;
   if(process.platform==='linux'){const status=await readFile('/proc/self/status','utf8');const match=status.match(/^VmHWM:\s+(\d+)\s+kB$/m);if(match)peak_rss_bytes=Number(match[1])*1024;}
   reply({kind:'measured',peak_rss_bytes,runtime_version:Bun.version,platform:process.platform});break;
  }
  case 'resolve-dependency':{
   keys(request,['kind','mode','specifier','importer']);
   if(!['import','require'].includes(request.mode)||typeof request.specifier!=='string'||!request.specifier||typeof request.importer!=='string'||!request.importer)throw new Error('Invalid dependency resolution request');
   try{reply({kind:'dependency-resolved',path:isBuiltin(request.specifier)?(request.specifier==='bun'||request.specifier.startsWith('node:')||request.specifier.startsWith('bun:')?request.specifier:'node:'+request.specifier):(request.mode==='require'?createRequire(resolve(request.importer)).resolve(request.specifier):Bun.resolveSync(request.specifier,dirname(resolve(request.importer))))});}
   catch(error){reply({kind:'dependency-unresolved',reason:error instanceof Error?error.message:String(error)});}break;
  }
  case 'describe':keys(request,['kind']);reply({kind:'described',description:await(await getAdapter()).describe()});break;
  case 'begin':{
   keys(request,['kind','case_id']);if(typeof request.case_id!=='string'||!request.case_id)throw new Error('Invalid case ID');
   if(session)throw new Error('Previous session must be closed');events=[];calls=[];resources=[];observationBytes=0;observationCount=0;observationOverflow=false;const ownEpoch=++epoch;
   const capabilities=(await(await getAdapter()).describe()).capabilities;
   observesCalls=capabilities.includes('call-observation');
   observesResources=capabilities.includes('resource-observation');
   session=await(await getAdapter()).begin(request.case_id,{
    collaborator(invocation){return new Promise<Observation>((resolve,reject)=>{
     if(epoch!==ownEpoch){reject(new Error('Session is no longer active'));return;}
     const request_id=nextMock++;pending.set(request_id,{resolve,reject});
     send({kind:'mock-call',request_id,invocation});
    });},
    emit(subject,payload){if(epoch!==ownEpoch)throw new Error('No active session for event');events.push(recordObservation({subject,payload}));},
    recordResource(observation){if(epoch!==ownEpoch||!observesResources)throw new Error('No active resource observer');keys(observation,observation.operation===undefined?['resource','access']:['operation','resource','access']);if(typeof observation.resource!=='string'||!observation.resource||(observation.operation!==undefined&&(typeof observation.operation!=='string'||!observation.operation))||!['read','write','observe'].includes(observation.access))throw new Error('Invalid resource observation');resources.push(recordObservation(observation));},
    recordCall(invocation,observation){if(epoch!==ownEpoch||!observesCalls)throw new Error('No active actual call observer');calls.push(recordObservation({invocation,observation}));},
   });
   if(!session||typeof session.invoke!=='function'||typeof session.close!=='function'){session=undefined;throw new Error('Invalid project session');}
   reply({kind:'begun',case_id:request.case_id});break;
  }
  case 'invoke':{
   keys(request,['kind','invocation']);keys(request.invocation,['subject','arguments']);
   if(!subjects.has(request.invocation.subject))throw new Error('Invocation outside selected subjects');
   const active=requireSession();events=[];calls=[];resources=[];observationBytes=0;observationCount=0;observationOverflow=false;
   // A rejected adapter promise is technical, never invented application evidence.
   const observation=await active.invoke(request.invocation);
   if(observationOverflow)throw new Error('Observation budget exceeded');
   reply({kind:'invoked',observation,events,...(observesCalls?{calls}: {}),...(observesResources?{resources}: {})});break;
  }
  case 'validate':{
   keys(request,['kind','subject','value']);if(!subjects.has(request.subject))throw new Error('Validation outside selected subjects');
   const active=requireSession();reply({kind:'value-checked',outcome:active.validate?await active.validate(request.subject,request.value):{kind:'unavailable',reason:'No production value factory/decoder described'}});break;
  }
  case 'observe':{
   keys(request,['kind','entities']);const active=requireSession();
   if(!active.observe)throw new AdapterError('No actual state observer');
   if(!Array.isArray(request.entities)||request.entities.some((s:unknown)=>typeof s!=='string'||!subjects.has(s)))throw new Error('Observation outside selected subjects');
   reply({kind:'observed',states:await active.observe(request.entities)});break;
  }
  case 'end':keys(request,['kind']);await end();reply({kind:'ended'});break;
  case 'shutdown':keys(request,['kind']);await end();reply({kind:'bye'});await flushProtocol();process.stdin.destroy();resolveFinished();break;
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
    let cleanupError:{value:unknown}|undefined;try{await end();}catch(cleanup){cleanupError={value:cleanup};}
    technical(envelope.id,error instanceof EnvironmentError?'environment':error instanceof AdapterError?'adapter':'unclassified',error);if(cleanupError)technical(envelope.id,'cleanup',cleanupError.value);
   }
  });
 }catch(error){
  rejectPending('Protocol violation');
  chain=chain.then(async()=>{
   let cleanupError:{value:unknown}|undefined;try{await end();}catch(cleanup){cleanupError={value:cleanup};}
   technical(envelope?.id??null,'protocol',error);if(cleanupError)technical(envelope?.id??null,'cleanup',cleanupError.value);
  });
 }
});
input.on('close',()=>{rejectPending('Worker input closed');void chain.finally(async()=>{try{await end();}catch(error){technical(null,'cleanup',error);}await flushProtocol();}).then(()=>resolveFinished());});
