import type { Value } from './contract.ts';
import type { Details, Attempt, Choice, Option } from '../../../src/data.ts';
const prefix = 'Tutorial.Board.';
export function primitive(value: Value): unknown {
  switch (value.kind) {
    case 'unit': return undefined;
    case 'boolean': case 'string': case 'decimal': return value.value;
    case 'integer': return BigInt(value.value);
    case 'float': return Number(value.value);
    case 'list': return value.value.map(primitive);
    case 'record': return Object.fromEntries(Object.entries(value.value).map(([key,entry]) => [key,primitive(entry)]));
    case 'option': return value.value === null ? {tag:'none'} : {tag:'some',value:primitive(value.value)};
    case 'variant': return {tag:value.value.name,value:value.value.payload === null ? undefined : primitive(value.value.payload)};
    case 'failure': return {failure:value.value.declaration,payload:value.value.payload === null ? undefined : primitive(value.value.payload)};
  }
}
export function decode(name: string, input: Value): unknown {
  if (name === 'Attachment' && input.kind === 'list') return input.value.map(byte => byte.kind === 'integer' ? Number(byte.value) : primitive(byte));
  if (name === 'Status' && input.kind === 'variant' && input.value.declaration === prefix+'Status' && input.value.payload === null) return input.value.name;
  if (name === 'Choice' && input.kind === 'variant' && input.value.declaration === prefix+'Choice') {
    if (input.value.name === 'Empty' && input.value.payload === null) return {tag:'empty'};
    if (input.value.name === 'Named' && input.value.payload !== null) return {tag:'named',title:primitive(input.value.payload)};
  }
  if (name === 'Attempt' && input.kind === 'variant' && input.value.declaration === 'builtin:Dctx0Result' && input.value.payload !== null) {
    if (input.value.name === 'Success') return {tag:'ok',value:primitive(input.value.payload)};
    if (input.value.name === 'Failure') return {tag:'error',error:primitive(input.value.payload)};
  }
  if (name === 'Details' && input.kind === 'record') return Object.fromEntries(Object.entries(input.value).map(([field,value]) => [field,decode(({status:'Status',choice:'Choice',attachment:'Attachment',attempt:'Attempt'} as Record<string,string>)[field] ?? '',value)]));
  return primitive(input);
}
const text = (value: string): Value => ({kind:'string',value});
const variant = (declaration: string,name: string,payload: Value|null): Value => ({kind:'variant',value:{declaration,name,payload}});
export function encode(name: string, value: unknown): Value {
  switch (name) {
    case 'Integer': return {kind:'integer',value:String(value)};
    case 'Flag': return {kind:'boolean',value:value as boolean};
    case 'Measure': case 'Estimate': return {kind:'float',value:String(value)};
    case 'ExactAmount': return {kind:'decimal',value:value as string};
    case 'Attachment': return {kind:'list',value:[...(value as Uint8Array)].map(byte => ({kind:'integer',value:String(byte)}))};
    case 'NoValue': return {kind:'unit'};
    case 'Absent': case 'MaybeTitle': case 'CompatOption': case 'PresentTitle': {
      const box = value as Option<string>; return {kind:'option',value:box.tag === 'none' ? null : text(box.value)};
    }
    case 'Status': return variant(prefix+'Status',value as string,null);
    case 'Choice': {const box=value as Choice;return variant(prefix+'Choice',box.tag==='empty'?'Empty':'Named',box.tag==='empty'?null:text(box.title));}
    case 'Attempt': {const box=value as Attempt;return variant('builtin:Dctx0Result',box.tag==='ok'?'Success':'Failure',box.tag==='ok'?{kind:'integer',value:String(box.value)}:text(box.error));}
    case 'Details': {
      const data=value as Details;
      return {kind:'record',value:{id:text(data.id),title:text(data.title),status:encode('Status',data.status),choice:encode('Choice',data.choice),active:encode('Flag',data.active),progress:encode('Measure',data.progress),estimate:encode('Estimate',data.estimate),amount:encode('ExactAmount',data.amount),attachment:encode('Attachment',data.attachment),note:encode('MaybeTitle',data.note),labels:{kind:'list',value:data.labels.map(text)},attempt:encode('Attempt',data.attempt)}};
    }
    default: throw new Error('No representation for '+name);
  }
}
