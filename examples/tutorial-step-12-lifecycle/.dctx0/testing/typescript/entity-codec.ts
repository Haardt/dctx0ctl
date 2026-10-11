import type {Value} from './contract.ts';
import {primitive,decode,encode} from './data-codec.ts';
import type {Task,Note,TaskSummary} from '../../../src/entities.ts';
export function decodeEntity(name:string,input:Value):unknown {
  if(input.kind!=='record')return primitive(input);
  return Object.fromEntries(Object.entries(input.value).map(([field,value])=>[field,name==='Task'&&field==='status'?decode('Status',value):primitive(value)]));
}
const text=(value:string):Value=>({kind:'string',value});
export function encodeEntity(name:string,value:unknown):Value {
  if(name==='Task'){const row=value as Task;return {kind:'record',value:{id:text(row.id),title:text(row.title),status:encode('Status',row.status)}};}
  if(name==='Note'){const row=value as Note;return {kind:'record',value:{taskId:text(row.taskId),sequence:{kind:'integer',value:String(row.sequence)},text:text(row.text)}};}
  if(name==='TaskSummary'){const row=value as TaskSummary;return {kind:'record',value:{id:text(row.id),title:text(row.title)}};}
  throw new Error('Unsupported entity representation '+name);
}
