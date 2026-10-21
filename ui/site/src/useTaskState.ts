import {useEffect,useState} from 'react';
import type {SiteSnapshot,TaskState} from './model';

// Full reports can be megabytes per task. Keep only the selected task expanded.
export function useTaskState(data:SiteSnapshot,id:string) {
 const stored=data.records.tasks?.states[id];
 const [loaded,setLoaded]=useState<{stored:TaskState;state?:TaskState;error?:string}>();
 useEffect(()=>{
  setLoaded(undefined);
  if(!stored?.details)return;
  let cancelled=false;
  const decode=async()=>{
   const details=stored.details!;
   if(details.encoding!=='gzip-base64-json-decimal-integers-v1')throw new Error('Unsupported task detail encoding');
   const bytes=Uint8Array.from(atob(details.data),char=>char.charCodeAt(0));
   const stream=new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
   const state=JSON.parse(await new Response(stream).text()) as TaskState;
   if(state.version!==stored.version||state.work!==stored.work||state.check!==stored.check)throw new Error('Task detail identity mismatch');
   if(!cancelled)setLoaded({stored,state});
  };
  void decode().catch(error=>{if(!cancelled)setLoaded({stored,error:String(error)})});
  return()=>{cancelled=true};
 },[stored]);
 const result=loaded?.stored===stored?loaded:undefined;
 return {state:result?.state??stored,loading:!!stored?.details&&!result,error:result?.error};
}
