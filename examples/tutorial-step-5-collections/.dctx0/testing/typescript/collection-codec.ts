import type {Value} from './contract.ts';
import type {Option} from '../../../src/data.ts';
export function resultValue(type: string,value: unknown): Value {
  if(type.startsWith('List<')) return {kind:'list',value:(value as unknown[]).map(entry=>resultValue(type.slice(5,-1),entry))};
  if(type.startsWith('Option<')) {
    const box=value as Option<unknown>;return {kind:'option',value:box.tag==='none'?null:resultValue(type.slice(7,-1),box.value)};
  }
  switch(type) {
    case 'Int': case 'Count': return {kind:'integer',value:String(value)};
    case 'Float': return {kind:'float',value:String(value)};
    case 'Boolean': return {kind:'boolean',value:value as boolean};
    case 'String': return {kind:'string',value:value as string};
    default: throw new Error('Unsupported collection representation '+type);
  }
}
