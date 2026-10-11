import {expect,test} from 'bun:test';
import {score,eligible,describe,named} from './expressions.ts';
test('score evaluates every valid finite input pair and rejects invalid counts',()=>{
  for(const a of [0n,1n,2n,3n]) for(const b of [1n,2n,3n]) expect(score(a,b)).toBe(a+2n*b);
  expect(()=>score(1n,0n)).toThrow(); expect(()=>score(4n,1n)).toThrow(); expect(()=>score(1n,4n)).toThrow();
});
test('eligibility and boundary construction call the actual production factories',()=>{
  for(const value of [0n,1n,2n,3n]) expect(eligible(value)).toBe(value>0n);
  expect(()=>eligible(-1n)).toThrow();
  expect(describe('Read')).toBe('Task: Read'); expect(()=>describe('')).toThrow();
  expect(named('Read')).toEqual({tag:'named',title:'Read'}); expect(()=>named('')).toThrow();
});
