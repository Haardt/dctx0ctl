import {next,validateCount} from '../../../production.ts';
import type {ProjectAdapter} from './contract.ts';
import {decodeInput} from './wire-codecs.ts';
// Foreign option/variant/failure inputs use an authored tagged envelope; primitive inputs retain their native shape.
export const adapter:ProjectAdapter={
 async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'production.ts',entry_point:'next'},{subject:'W.Count',role:'validator',artifact:'production.ts',entry_point:'validateCount'}],artifacts:['production.ts'],capabilities:['sessions','invocation','events','values'],representations:{Int:'bigint'}};},
 async begin(){return {async close(){},async validate(subject,value){const input=value.kind==='integer'?BigInt(value.value):['option','variant','failure'].includes(value.kind)?value:decodeInput(subject,value);return validateCount(input)?{kind:'accepted',value}:{kind:'rejected',reason:'Production count parser rejected input'};},async invoke(invocation){const value=invocation.arguments[0];if(value?.kind!=='integer')throw new Error('Adapter input unavailable');const result=next(BigInt(value.value));return {kind:'returned',value:{kind:'integer',value:String(result)}};}};},
};
