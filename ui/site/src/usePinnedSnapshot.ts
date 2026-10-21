import {useEffect,useState} from 'react';
import {pinnedSnapshot,type SiteSnapshot,type Snapshot} from './model';

const pending=new WeakMap<SiteSnapshot,Map<string,Promise<Snapshot>>>();
// Serialize expansion across inspectors; completed snapshots stay only in the
// mounted consumer's state, rather than an ever-growing history cache.
let expansion:Promise<unknown>=Promise.resolve();
function decodeSnapshot(data:SiteSnapshot,id:string):Promise<Snapshot> {
 const requests=pending.get(data)??new Map<string,Promise<Snapshot>>();
 pending.set(data,requests);
 const existing=requests.get(id);if(existing)return existing;
 const request=expansion.catch(()=>undefined).then(async()=>{
  const stored=data.records.requirement_snapshots?.[id];
  if(!stored){
   if(data.records.omitted_requirement_snapshots?.includes(id))throw new Error(`Snapshot ${id} is outside this site's source history (current + previous, plus the selected task plan). Export with --snapshot ${id}, or use --history all to include older task sources. Stored evidence is unchanged.`);
   throw new Error('Pinned snapshot unavailable');
  }
  if('id' in stored)return stored;
  if(stored.encoding!=='gzip-base64-json-decimal-integers-v1')throw new Error('Unsupported pinned snapshot encoding');
  const bytes=Uint8Array.from(atob(stored.data),char=>char.charCodeAt(0));
  const stream=new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
  const snapshot=JSON.parse(await new Response(stream).text()) as Snapshot;
  if(snapshot.id!==id)throw new Error('Pinned snapshot identity mismatch');
  return snapshot;
 }).finally(()=>requests.delete(id));
 requests.set(id,request);expansion=request.then(()=>undefined,()=>undefined);return request;
}
export function usePinnedSnapshot(data:SiteSnapshot,id:string|null) {
 const snapshot=pinnedSnapshot(data,id);
 const [loaded,setLoaded]=useState<{data:SiteSnapshot;id:string;snapshot?:Snapshot;error?:string}>();
 useEffect(()=>{
  setLoaded(undefined);
  if(!id||snapshot)return;
  let cancelled=false;
  void decodeSnapshot(data,id).then(value=>{if(!cancelled)setLoaded({data,id,snapshot:value})},error=>{if(!cancelled)setLoaded({data,id,error:String(error)})});
  return()=>{cancelled=true};
 },[data,id,snapshot]);
 const result=loaded?.data===data&&loaded.id===id?loaded:undefined;
 return {snapshot:snapshot??result?.snapshot,error:result?.error,loading:!!id&&!snapshot&&!result};
}
