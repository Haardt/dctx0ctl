import { test, expect } from 'bun:test';
import { count, increment } from './board.ts';

test('count accepts exactly the four declared values', () => {
  for (const value of [0n, 1n, 2n, 3n]) expect(count(value)).toBe(value);
  for (const value of [-1n, 4n, 1.5, '1', null, undefined]) {
    expect(() => count(value)).toThrow(TypeError);
  }
});
test('increment executes the real operation and enforces its prerequisite', () => {
  for (const value of [0n, 1n, 2n]) expect(increment(value)).toBe(value + 1n);
  expect(() => increment(3n)).toThrow(RangeError);
  expect(() => increment(-1n)).toThrow(TypeError);
});
