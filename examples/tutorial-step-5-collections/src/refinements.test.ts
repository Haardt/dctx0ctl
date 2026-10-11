import { test, expect } from 'bun:test';
import { rawLabel, title, displayTitle, echo, email, identifier, taskId, link, day, instant, shortList } from './board.ts';

test('text aliases enforce UTF-16 length and nonempty trimmed input', () => {
  for (const value of ['a', '😀', 'x'.repeat(16), '\u0085']) {
    expect(title(value)).toBe(value);
    expect(displayTitle(value)).toBe(value);
    expect(echo(value)).toBe(value);
  }
  for (const value of ['', ' ', '\ufeff', 'x'.repeat(17), 1, null]) {
    expect(() => title(value)).toThrow(TypeError);
  }
  expect(rawLabel('')).toBe('');
  expect(() => rawLabel({})).toThrow(TypeError);
});
test('all declared email and identity formats are actually validated', () => {
  for (const value of ['a@b.co', 'user@example.test']) expect(email(value)).toBe(value);
  for (const value of ['a@@b.co', 'a@b', '@b.co', 'a@b.', ' a@b.co', 'x'.repeat(254) + '@b.co']) expect(() => email(value)).toThrow(TypeError);
  const uuid = '00000000-0000-4000-8000-000000000001';
  expect(identifier(uuid)).toBe(uuid);
  expect(taskId(uuid)).toBe(uuid);
  expect(taskId('00000000-0000-0000-0000-000000000000')).toBe('00000000-0000-0000-0000-000000000000');
  for (const value of ['broken', '00000000-0000-0000-0000-000000000000']) expect(() => identifier(value)).toThrow(TypeError);
  expect(() => taskId('broken')).toThrow(TypeError);
});
test('links require an absolute URL; dates reject calendar rollover', () => {
  for (const value of ['https://example.test/tasks', 'mailto:user@example.test', 'urn:task:1']) expect(link(value)).toBe(value);
  expect(() => link('/tasks')).toThrow();
  expect(() => link('https://bad host.test')).toThrow();
  for (const value of ['2000-02-29', '2026-10-11', '0000-02-29']) expect(day(value)).toBe(value);
  for (const value of ['1900-02-29', '2026-02-30', '2026-00-01', '2026-13-01', '2026-01-00', '2026-1-01', '2026-01-01\n']) expect(() => day(value)).toThrow(TypeError);
});
test('timestamps preserve offsets and precision without accepting impossible times', () => {
  for (const value of ['2026-10-11T12:34:56Z', '2026-10-11T12:34:56.123456789+02:00', '2026-10-11t12:34:60z']) expect(instant(value)).toBe(value);
  for (const value of ['2026-02-30T00:00:00Z', '2026-10-11T24:00:00Z', '2026-10-11T00:60:00Z', '2026-10-11T00:00:61Z', '2026-10-11T00:00:00+24:00', '2026-10-11T00:00:00+00:60', 'today', '2026-10-11T00:00:00Z\n']) expect(() => instant(value)).toThrow(TypeError);
});
test('list refinements reject invalid elements and preserve ownership', () => {
  const input = [1n, 2n];
  expect(shortList(input)).toEqual(input);
  expect(shortList(input)).not.toBe(input);
  expect(shortList([1n])).toEqual([1n]);
  for (const value of [[], [1n, 2n, 3n], [1], [1n, null], null]) expect(() => shortList(value)).toThrow(TypeError);
});
