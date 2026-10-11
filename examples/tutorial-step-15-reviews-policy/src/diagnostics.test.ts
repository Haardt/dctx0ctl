import {expect,test} from 'bun:test';
import {Diagnostics,SystemClock} from './diagnostics.ts';
test('system clock samples actual time and diagnostics invokes its provided boundary',()=>{
  const before=Date.now(),sample=new SystemClock().now(),after=Date.now();
  expect(Date.parse(sample)).toBeGreaterThanOrEqual(before);expect(Date.parse(sample)).toBeLessThanOrEqual(after);
  let calls=0;const diagnostics=new Diagnostics(()=>{calls++;return '2026-10-11T00:00:00Z';});
  expect(diagnostics.timestamp()).toBe('2026-10-11T00:00:00Z');expect(calls).toBe(1);
});
test('clock preserves a supplied real Date and propagates an invalid sample',()=>{
  expect(new SystemClock(()=>new Date('2026-10-11T00:00:00Z')).now()).toBe('2026-10-11T00:00:00.000Z');
  expect(()=>new SystemClock(()=>new Date(NaN)).now()).toThrow();
});
