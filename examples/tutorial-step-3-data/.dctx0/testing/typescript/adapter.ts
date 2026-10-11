import * as data from '../../../src/data.ts';
import {decode, encode} from './data-codec.ts';
import { count, increment, echo, rawLabel, title, displayTitle, email, identifier, taskId, link, day, instant, shortList } from '../../../src/board.ts';
import type { ProjectAdapter, Value } from './contract.ts';
const prefix = 'Tutorial.Board.';
const dataFactories = {Integer:data.integer, Flag:data.flag, Measure:data.measure, Estimate:data.estimate, ExactAmount:data.exactAmount, Attachment:data.attachment, NoValue:data.noValue, MaybeTitle:data.maybeTitle, CompatOption:data.maybeTitle, PresentTitle:data.presentTitle, Absent:data.absent, Attempt:data.attempt, Status:data.status, Choice:data.choice, Details:data.details};
const textFactories = { RawLabel: rawLabel, Title: title, DisplayTitle: displayTitle, Email: email, Identifier: identifier, TaskId: taskId, Link: link, Day: day, Instant: instant, Moment: instant };
function text(input: Value): string { if (input.kind !== 'string') throw new TypeError('Expected text wire value'); return input.value; }
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
        ...Object.entries(dataFactories).map(([name,factory]) => ({subject:prefix+name,role:'contract',artifact:'src/data.ts',entry_point:factory.name})),
        ...['keepDetails','keepAttempt'].map(name => ({subject:prefix+'Actions.'+name,role:'operation',artifact:'src/data.ts',entry_point:name})),
        ...Object.entries(textFactories).map(([name, factory]) => ({ subject: prefix + name, role: 'contract', artifact: 'src/board.ts', entry_point: factory.name })),
        { subject: prefix + 'ShortList', role: 'contract', artifact: 'src/board.ts', entry_point: 'shortList' },
        { subject: prefix + 'Actions.echo', role: 'operation', artifact: 'src/board.ts', entry_point: 'echo' },
        { subject: prefix + 'Count', role: 'contract', artifact: 'src/board.ts', entry_point: 'count' },
        { subject: prefix + 'Actions.increment', role: 'operation', artifact: 'src/board.ts', entry_point: 'increment' },
      ],
      artifacts: ['src/board.ts','src/data.ts'], capabilities: ['sessions', 'values', 'invocation', 'events'],
      representations: { Int: 'native bigint; lossless integer strings on the wire', Decimal:'Exact decimal string', Duration:'Finite number of seconds', Bytes:'Uint8Array, validated before conversion', Option:'Explicit none/some object', Dctx0Result:'Explicit ok/error object', Details:'Native record with field-specific representation mapping' },
    };
  },
  async begin() {
    return {
      async validate(subject, input) {
        const name=subject.slice(prefix.length);
        const dataFactory=dataFactories[name as keyof typeof dataFactories];
        if(dataFactory) {
          try {return {kind:'accepted',value:encode(name,dataFactory(decode(name,input)))};}
          catch(error) {return {kind:'rejected',reason:String(error)};}
        }
        const factory = textFactories[subject.slice(prefix.length) as keyof typeof textFactories];
        if (factory) {
          try { return { kind: 'accepted', value: { kind: 'string', value: factory(primitive(input)) } }; }
          catch (error) { return { kind: 'rejected', reason: String(error) }; }
        }
        if (subject === prefix + 'ShortList') {
          try {
            const output = shortList(primitive(input));
            return { kind: 'accepted', value: { kind: 'list', value: output.map(value => ({ kind: 'integer', value: String(value) })) } };
          } catch (error) { return { kind: 'rejected', reason: String(error) }; }
        }
        if (subject !== prefix + 'Count') return { kind: 'unavailable', reason: 'Unknown contract' };
        try { return { kind: 'accepted', value: { kind: 'integer', value: String(count(primitive(input))) } }; }
        catch (error) { return { kind: 'rejected', reason: String(error) }; }
      },
      async invoke(invocation) {
        if(invocation.subject===prefix+'Actions.keepDetails' || invocation.subject===prefix+'Actions.keepAttempt') {
          const name=invocation.subject.endsWith('keepDetails')?'Details':'Attempt';
          try {
            const decoded=decode(name,invocation.arguments[0]!);
            const output=name==='Details'?data.keepDetails(decoded as data.Details):data.keepAttempt(decoded as data.Attempt);
            return {kind:'returned',value:encode(name,output)};
          } catch(error) {return {kind:'threw',message:String(error)};}
        }
        if (invocation.subject === prefix + 'Actions.echo') {
          try { return { kind: 'returned', value: { kind: 'string', value: echo(text(invocation.arguments[0]!)) } }; }
          catch (error) { return { kind: 'threw', message: String(error) }; }
        }
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
