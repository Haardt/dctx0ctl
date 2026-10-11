import {expect,test} from 'bun:test';
import {timestamp} from './timestamp.ts';
import {validateRuntimeContract} from './contracts.ts';
test('calendar validation preserves valid wire precision and offsets',()=>{
 for(const value of ['2024-02-29T12:00:00Z','2000-02-29t23:59:60z','0000-01-01T00:00:00+23:59','2026-10-11 12:00:00.123456789123-00:00'])expect(timestamp(value)).toBe(value);
});
test('impossible dates and out of range clock/offset values are rejected',()=>{
 for(const value of ['2026-02-30T00:00:00Z','1900-02-29T00:00:00Z','2024-04-31T00:00:00Z','2026-00-01T00:00:00Z','2026-13-01T00:00:00Z','2026-01-00T00:00:00Z','2026-01-01T24:00:00Z','2026-01-01T00:60:00Z','2026-01-01T00:00:61Z','2026-01-01T00:00:00+24:00','2026-01-01T00:00:00+00:60','2026-01-01T00:00:00','2026-01-01T00:00:00Z\n','invalid']){
  expect(()=>timestamp(value)).toThrow(TypeError);
  expect(()=>validateRuntimeContract('OpenVoting',{pollId:'00000000-0000-4000-8000-000000000020',candidateFilmIds:[],scheduledFor:value})).toThrow(TypeError);
 }
 expect(()=>timestamp(null)).toThrow(TypeError);
});
