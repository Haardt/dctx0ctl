import { count, increment } from '../../../src/board.ts';
import type { ProjectAdapter, Value } from './contract.ts';
const prefix = 'Tutorial.Board.';
function integer(value: Value | undefined): bigint {
  if (!value || value.kind !== 'integer' || !/^[+-]?\d+$/.test(value.value)) {
    throw new TypeError('Expected a lossless integer wire value');
  }
  return BigInt(value.value);
}
function primitive(value: Value): unknown {
  switch (value.kind) {
    case 'unit': return undefined;
    case 'boolean': case 'string': return value.value;
    case 'integer': return BigInt(value.value);
    case 'float': return Number(value.value);
    case 'decimal': return value.value;
    case 'list': return value.value.map(primitive);
    case 'record': return Object.fromEntries(Object.entries(value.value).map(([key, entry]) => [key, primitive(entry)]));
    case 'option': return value.value === null ? { tag: 'none' } : { tag: 'some', value: primitive(value.value) };
    case 'variant': return { tag: value.value.name, value: value.value.payload === null ? undefined : primitive(value.value.payload) };
    case 'failure': return { failure: value.value.declaration, payload: value.value.payload === null ? undefined : primitive(value.value.payload) };
  }
}
export const adapter: ProjectAdapter = {
  async describe() {
    return {
      protocol_version: 1,
      subjects: [
        { subject: prefix + 'Count', role: 'contract', artifact: 'src/board.ts', entry_point: 'count' },
        { subject: prefix + 'Actions.increment', role: 'operation', artifact: 'src/board.ts', entry_point: 'increment' },
      ],
      artifacts: ['src/board.ts'], capabilities: ['sessions', 'values', 'invocation', 'events'],
      representations: { Int: 'native bigint; lossless integer strings on the wire' },
    };
  },
  async begin() {
    return {
      async validate(subject, input) {
        if (subject !== prefix + 'Count') return { kind: 'unavailable', reason: 'Unknown contract' };
        try { return { kind: 'accepted', value: { kind: 'integer', value: String(count(primitive(input))) } }; }
        catch (error) { return { kind: 'rejected', reason: String(error) }; }
      },
      async invoke(invocation) {
        if (invocation.subject !== prefix + 'Actions.increment') {
          return { kind: 'unavailable', reason: 'Unknown operation' };
        }
        try { return { kind: 'returned', value: { kind: 'integer', value: String(increment(integer(invocation.arguments[0]))) } }; }
        catch (error) { return { kind: 'threw', message: String(error) }; }
      },
      async close() {},
    };
  },
};
