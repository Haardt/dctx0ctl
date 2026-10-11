import {expect, test, spyOn} from 'bun:test';
import {Application} from './app.ts';
import * as counter from './board.ts';


test('counter enforces the finite contract and implements every valid increment', () => {
  for (const value of [0n, 1n, 2n, 3n]) expect(counter.count(value)).toBe(value);
  for (const value of [0n, 1n, 2n]) expect(counter.increment(value)).toBe(value + 1n);
  for (const invalid of [-1n, 4n, 1, '1', null]) expect(() => counter.count(invalid)).toThrow();
  expect(() => counter.increment(3n)).toThrow('full board');
});
test('real HTTP handler validates the payload, executes business code and handles routing errors', async () => {
  const application = new Application();
  try {
    for (const value of ['0', '1', '2']) {
      const response = await fetch(new URL('/increment', application.server.url), {method: 'POST', body: JSON.stringify({count: value})});
      expect(response.status).toBe(200);expect(await response.json()).toEqual({result: String(BigInt(value) + 1n)});
    }
    for (const body of ['{', '{}', '{"count":1}', '{"count":"1.1"}', '{"count":"-1"}', '{"count":"3"}', '{"count":"4"}']) {
      expect((await fetch(new URL('/increment', application.server.url), {method: 'POST', body})).status).toBe(400);
    }
    expect((await fetch(new URL('/missing', application.server.url))).status).toBe(404);
    expect((await fetch(new URL('/increment', application.server.url))).status).toBe(405);
    const original = counter.increment;
    const spy = spyOn(counter, 'increment').mockImplementation(() => {throw new Error('Unexpected application failure');});
    try {await expect(application.handle(new Request(new URL('/increment', application.server.url), {method: 'POST', body: JSON.stringify({count: '1'})}))).rejects.toThrow('Unexpected application failure');}
    finally {spy.mockRestore(); expect(counter.increment).toBe(original);}
  } finally {await application.close();}
});
test('actual executable entry point starts and stops a server', async () => {
  const old = process.env.PORT;process.env.PORT = '0';
  try {
    const {application} = await import('./server.ts');
    try {expect((await fetch(new URL('/increment', application.server.url), {method: 'POST', body: '{"count":"1"}'})).status).toBe(200);}
    finally {await application.close();}
  } finally {if (old === undefined) delete process.env.PORT;else process.env.PORT = old;}
});
