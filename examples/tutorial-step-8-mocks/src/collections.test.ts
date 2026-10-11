import {expect,test} from 'bun:test';
import * as c from './collections.ts';
test('list operations retain order, cardinality and exact values',()=>{
  const values=[3n,0n,1n,3n];
  expect(c.size(values)).toBe(4n);expect(c.total(values)).toBe(7n);
  expect(c.allPositive(values)).toBe(false);expect(c.allPositive([1n,3n])).toBe(true);expect(c.allPositive([])).toBe(true);
  expect(c.anyPositive(values)).toBe(true);expect(c.anyPositive([0n])).toBe(false);expect(c.anyPositive([])).toBe(false);
  expect(c.containsZero(values)).toBe(true);expect(c.containsZero([1n])).toBe(false);
  expect(c.unique(values)).toEqual([3n,0n,1n]);
  expect(c.minimum(values)).toBe(0n);expect(c.maximum(values)).toBe(3n);
  expect(()=>c.minimum([])).toThrow();expect(()=>c.maximum([])).toThrow();
  expect(c.positive(values)).toEqual([3n,1n,3n]);expect(c.doubled(values)).toEqual([6n,0n,2n,6n]);
  expect(c.ascending(values)).toEqual([0n,1n,3n,3n]);expect(c.descending(values)).toEqual([3n,3n,1n,0n]);
  expect(c.head(values)).toEqual({tag:'some',value:3n});expect(c.head([])).toEqual({tag:'none'});
  expect(c.only([2n])).toBe(2n);expect(()=>c.only([])).toThrow();expect(()=>c.only(values)).toThrow();
  expect(c.average(values)).toEqual({tag:'some',value:1.75});expect(c.average([])).toEqual({tag:'none'});
  expect(c.zeroIndex(values)).toBe(1n);expect(c.zeroIndex([1n])).toBe(-1n);
  expect(c.fallback(values)).toBe(3n);expect(c.fallback([])).toBe(-1n);
  expect(c.backwards(values)).toEqual([3n,1n,0n,3n]);expect(c.window(values)).toEqual([0n,1n,3n]);expect(c.window([])).toEqual([]);
  expect(values).toEqual([3n,0n,1n,3n]);
  expect(()=>c.counts('wrong')).toThrow();expect(()=>c.counts([4n])).toThrow();expect(()=>c.counts([1])).toThrow();
});
test('text operations use actual TypeScript string semantics',()=>{
  expect(c.lower('ÄBC')).toBe('äbc');expect(c.upper('straße')).toBe('STRASSE');expect(c.clean(' \tRead\n')).toBe('Read');
  expect(c.textSize('🧩')).toBe(2n);expect(c.starts('Task: Read')).toBe(true);expect(c.starts('task')).toBe(false);
  expect(c.ends('Task done')).toBe(true);expect(c.ends('Done')).toBe(false);
  expect(c.includes('Task: Read')).toBe(true);expect(c.includes('read')).toBe(false);
  expect(()=>c.lower(1)).toThrow();
});
test('optional values distinguish presence from the contained count',()=>{
  expect(c.optional(0n,true)).toEqual({tag:'some',value:0n});expect(c.optional(2n,false)).toEqual({tag:'none'});
  expect(()=>c.optional(4n,true)).toThrow();expect(()=>c.optional(1n,'yes')).toThrow();
});
