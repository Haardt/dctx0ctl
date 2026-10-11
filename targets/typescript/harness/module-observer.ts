import {plugin} from 'bun';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
type Site={start_byte:number;end_byte:number;argument_start:number;argument_end:number};
type Source={source:string;sha256:string;sites:Site[]};
type Manifest={schema_version:number;observer_version:string;project_root_relative:string;sources:Source[]};
const observerVersion='dctx0-esm-observer-2';
const hook=Symbol.for('dctx0.module-observer.v1');
const loaded=new Set<string>();
const attempts:{source:string;start_byte:number;specifier:string;resolved:string|null}[]=[];
let complete=true,installed=false,manifest:Manifest|undefined,bytes=0;
export function installModuleObserver(){
 if(installed)return;installed=true;
 let text:string;
 try{text=readFileSync(resolve(import.meta.dir,'module-observer.json'),'utf8');}
 catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return;throw error;}
 if(Buffer.byteLength(text)>262144)throw new Error('Module observer manifest exceeds budget');
 manifest=JSON.parse(text);
 if(!manifest||manifest.schema_version!==1||!['dctx0-esm-observer-1',observerVersion].includes(manifest.observer_version)||!Array.isArray(manifest.sources))throw new Error('Unsupported module observer manifest');
 const root=resolve(import.meta.dir,manifest.project_root_relative);
 const sources=new Map<string,Source>();
 for(const source of manifest.sources){
  if(!source.source.endsWith('.mjs')||source.source.split('/').some(part=>part==='..'||part==='')||sources.has(resolve(root,source.source)))throw new Error('Invalid module observer source');
  sources.set(resolve(root,source.source),source);
 }
 Object.defineProperty(globalThis,hook,{value:(value:unknown,source:string,start:number,opaque=false)=>{
  if(opaque){complete=false;return value;}
  const entry=manifest!.sources.find(entry=>entry.source===source);
  if(!entry?.sites.some(site=>site.start_byte===start)||!loaded.has(source)||typeof value!=='string'||value.length>4096){complete=false;return value;}
  let resolved:string|null=null;
  try{resolved=import.meta.resolve(value,pathToFileURL(resolve(root,source)).href);}catch{}
  const attempt={source,start_byte:start,specifier:value,resolved};
  const size=Buffer.byteLength(JSON.stringify(attempt));
  if(attempts.length>=4096||bytes+size>262144){complete=false;return value;}
  attempts.push(attempt);bytes+=size;return value;
 },writable:false,configurable:false});
 const escape=(path:string)=>path.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 const filter=new RegExp('^(?:'+[...sources.keys()].map(escape).join('|')+')$');
 plugin({name:observerVersion,setup(build){build.onLoad({filter},args=>{
  const source=sources.get(args.path);if(!source)throw new Error('Unexpected observed module');
  const content=readFileSync(args.path);
  if(createHash('sha256').update(content).digest('hex')!==source.sha256)throw new Error('Observed module source changed');
  let transformed=content;
  let boundary=content.length;
  for(const site of [...source.sites].sort((left,right)=>right.argument_start-left.argument_start)){
   if(!Number.isSafeInteger(site.argument_start)||!Number.isSafeInteger(site.argument_end)||site.argument_start<site.start_byte||site.argument_end>site.end_byte||site.argument_start>=site.argument_end||site.argument_end>boundary)throw new Error('Invalid module observer range');
   const prefix=Buffer.from(`globalThis[Symbol.for('dctx0.module-observer.v1')]((`);
   const opaque=manifest!.observer_version===observerVersion&&site.argument_start===site.start_byte&&site.argument_end===site.end_byte;
   const suffix=Buffer.from(`),${JSON.stringify(source.source)},${site.start_byte},${opaque})`);
   transformed=Buffer.concat([transformed.subarray(0,site.argument_start),prefix,transformed.subarray(site.argument_start,site.argument_end),suffix,transformed.subarray(site.argument_end)]);
   boundary=site.argument_start;
  }
  loaded.add(source.source);
  return {contents:transformed.toString('utf8'),loader:'js'};
 });}});
}
export function moduleObserverEvidence(){
 return {schema_version:1,observer_version:manifest?.observer_version??observerVersion,loaded_sources:[...loaded].sort(),attempts:[...attempts],complete:complete&&manifest!==undefined};
}
