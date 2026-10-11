import {expect,test} from 'bun:test';
import * as d from './decimal.ts';
test('rounding preserves exact digits, signs and all four tie modes',()=>{
  expect(d.down('2.349')).toBe('2.34');expect(d.down('-2.349')).toBe('-2.34');
  expect(d.up('2.341')).toBe('2.35');expect(d.up('-2.341')).toBe('-2.35');
  expect(d.halfUp('2.345')).toBe('2.35');expect(d.halfUp('-2.345')).toBe('-2.35');
  expect(d.halfEven('2.345')).toBe('2.34');expect(d.halfEven('2.355')).toBe('2.36');
  expect(d.halfEven('-2.345')).toBe('-2.34');expect(d.halfEven('-2.355')).toBe('-2.36');
  expect(d.halfEven('2.344')).toBe('2.34');expect(d.halfEven('2.346')).toBe('2.35');
  expect(d.halfEven('9007199254740993.125')).toBe('9007199254740993.12');
  expect(d.up('2.340')).toBe('2.34');expect(d.up('1')).toBe('1');expect(d.up('1.20')).toBe('1.2');
  expect(d.down('0.001')).toBe('0');expect(()=>d.down('1e3')).toThrow();
});
test('division rounds repeating quotients and retains exact finite quotients',()=>{
  expect(d.third('1')).toBe('0.33');expect(d.third('-1')).toBe('-0.33');
  expect(d.third('2')).toBe('0.67');expect(d.third('3')).toBe('1');
  expect(d.third('0.375')).toBe('0.125');expect(d.third('0')).toBe('0');
});
