import {Application} from '../../../src/app.ts';
import * as factory from '../../../src/board.ts';
import type {ProjectAdapter, Value} from './contract.ts';

export const adapter: ProjectAdapter = {
  async describe() { return {
    protocol_version: 1,
    subjects: [
      {subject: 'Tutorial.Board.Count', role: 'contract', artifact: 'src/board.ts', entry_point: 'count'},
      {subject: 'Tutorial.Board.Actions.increment', role: 'operation', artifact: 'src/app.ts', entry_point: 'POST /increment'},
    ],
    artifacts: ['src/board.ts', 'src/app.ts', 'src/server.ts'],
    capabilities: ['sessions', 'values', 'invocation', 'events'],
    representations: {Int: 'native bigint; exact integer strings over HTTP'},
  }; },
  async begin() {
    const application = new Application();
    return {
      async validate(subject, input) {
        if (subject !== 'Tutorial.Board.Count') return {kind: 'unavailable', reason: 'Unknown contract'};
        try {
          const value: unknown = input.kind === 'integer' ? BigInt(input.value) : input.kind === 'unit' ? undefined : input.value;
          return {kind: 'accepted', value: {kind: 'integer', value: String(factory.count(value))}};
        } catch (error) { return {kind: 'rejected', reason: String(error)}; }
      },
      async invoke(invocation) {
        if (invocation.subject !== 'Tutorial.Board.Actions.increment') return {kind: 'unavailable', reason: 'Unknown operation'};
        const argument: Value | undefined = invocation.arguments[0];
        if (argument?.kind !== 'integer') return {kind: 'threw', message: 'Expected integer wire value'};
        try {
          const response = await fetch(new URL('/increment', application.server.url), {
            method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify({count: argument.value}),
          });
          if (!response.ok) throw new Error('Application HTTP status ' + response.status);
          const body = await response.json() as {result: string};
          return {kind: 'returned', value: {kind: 'integer', value: body.result}};
        } catch (error) { return {kind: 'threw', message: String(error)}; }
      },
      async close() { await application.close(); },
    };
  },
};
