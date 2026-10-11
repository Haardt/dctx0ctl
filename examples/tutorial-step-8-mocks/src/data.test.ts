import { expect, test } from 'bun:test';
import * as d from './data.ts';
const sample = () => ({id:'00000000-0000-4000-8000-000000000001',title:'Read',status:'Open',choice:{tag:'named',title:'Book'},active:true,progress:2,estimate:60,amount:'12345678901234567890.01',attachment:[0,255],note:{tag:'some',value:'Later'},labels:['Book'],attempt:{tag:'ok',value:2n}});
test('primitive representations retain precision and reject invalid inputs', () => {
  expect(d.integer(123456789012345678901n)).toBe(123456789012345678901n); expect(d.flag(false)).toBe(false);
  for (const value of [0,10]) expect(d.measure(value)).toBe(value);
  for (const value of [0,3600]) expect(d.estimate(value)).toBe(value);
  expect(d.exactAmount('12345678901234567890.01')).toBe('12345678901234567890.01'); expect(d.noValue(undefined)).toBeUndefined();
  for (const value of [1,'1',null]) expect(() => d.integer(value)).toThrow();
  for (const value of [0,'false',null]) expect(() => d.flag(value)).toThrow();
  for (const value of [-1,11,NaN,Infinity,'1']) expect(() => d.measure(value)).toThrow();
  for (const value of [-1,3601,NaN,Infinity,'1']) expect(() => d.estimate(value)).toThrow();
  for (const value of [1,'1e3','NaN','', '+1.0']) expect(() => d.exactAmount(value)).toThrow(); expect(() => d.noValue(null)).toThrow();
});
test('bytes are validated before conversion and copied', () => {
  expect([...d.attachment([0,1,255])]).toEqual([0,1,255]);
  const source = new Uint8Array([128]); const copy = d.attachment(source); source[0] = 0; expect(copy[0]).toBe(128);
  expect([...d.attachment([])]).toEqual([]);
  for (const value of [null,'bytes',[-1],[256],[1.5],['1'],[1n]]) expect(() => d.attachment(value)).toThrow();
});
test('options and results preserve their alternatives', () => {
  expect(d.maybeTitle({tag:'none'})).toEqual({tag:'none'}); expect(d.maybeTitle({tag:'some',value:'Read'})).toEqual({tag:'some',value:'Read'});
  expect(d.presentTitle({tag:'some',value:'Read'}).value).toBe('Read'); expect(d.absent({tag:'none'})).toEqual({tag:'none'});
  expect(d.attempt({tag:'ok',value:2n})).toEqual({tag:'ok',value:2n}); expect(d.keepAttempt({tag:'error',error:'Retry'})).toEqual({tag:'error',error:'Retry'});
  for (const value of [null,[],{}, {tag:'other'}, {tag:'some'}, {tag:'some',value:''}, {tag:'none',value:'extra'}]) expect(() => d.maybeTitle(value)).toThrow();
  expect(() => d.presentTitle({tag:'none'})).toThrow(); expect(() => d.absent({tag:'some'})).toThrow();
  for (const value of [null,{}, {tag:'other',error:'Retry'}, {tag:'ok',value:4n}, {tag:'error',error:''}]) expect(() => d.attempt(value)).toThrow();
});
test('records and variants validate every field through production factories', () => {
  expect(d.status('Open')).toBe('Open'); expect(d.status('Done')).toBe('Done'); expect(() => d.status('Unknown')).toThrow();
  expect(d.choice({tag:'empty'})).toEqual({tag:'empty'}); expect(d.choice({tag:'named',title:'Book'})).toEqual({tag:'named',title:'Book'});
  for (const value of [null, {tag:'other',title:'Book'}, {tag:'named',title:''}]) expect(() => d.choice(value)).toThrow();
  expect(d.labels(['One','Two'])).toEqual(['One','Two']); expect(() => d.labels('One')).toThrow(); expect(() => d.labels([''])).toThrow();
  const result = d.keepDetails(d.details(sample())); expect(result.amount).toBe('12345678901234567890.01'); expect([...result.attachment]).toEqual([0,255]); expect(result.note).toEqual({tag:'some',value:'Later'});
  for (const value of [null,[],{}, {...sample(),extra:1}]) expect(() => d.details(value)).toThrow();
  const missing = sample() as Record<string,unknown>; delete missing.title; missing.extra=1; expect(() => d.details(missing)).toThrow();
  expect(() => d.details({...sample(),id:'invalid'})).toThrow();
});
