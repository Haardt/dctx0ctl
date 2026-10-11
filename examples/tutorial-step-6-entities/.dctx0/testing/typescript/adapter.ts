import * as entities from '../../../src/entities.ts';
import initialState from './initial-state.json';
import {decodeEntity,encodeEntity} from './entity-codec.ts';
import * as decimals from '../../../src/decimal.ts';
import * as collections from '../../../src/collections.ts';
import signatures from './collection-signatures.json';
import {resultValue} from './collection-codec.ts';
import * as expressions from '../../../src/expressions.ts';
import * as data from '../../../src/data.ts';
import {decode, encode} from './data-codec.ts';
import { count, increment, echo, rawLabel, title, displayTitle, email, identifier, taskId, link, day, instant, shortList } from '../../../src/board.ts';
import type { ProjectAdapter, Value } from './contract.ts';
const prefix = 'Tutorial.Board.';
const entityFactories={Task:entities.task,Note:entities.note,TaskSummary:entities.taskSummary};
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
        ...Object.entries(entityFactories).map(([name,factory])=>({subject:prefix+name,role:'contract',artifact:'src/entities.ts',entry_point:factory.name})),
        ...['list','find','notes'].map(name=>({subject:prefix+'Tasks.'+name,role:'operation',artifact:'src/entities.ts',entry_point:'Board.'+name})),
        ...Object.keys(decimals).map(name=>({subject:prefix+'Actions.'+name,role:'operation',artifact:'src/decimal.ts',entry_point:name})),
        ...Object.keys(signatures.operations).map(name=>({subject:prefix+'Actions.'+name,role:'operation',artifact:'src/collections.ts',entry_point:name})),
        ...Object.keys(expressions).map(name=>({subject:prefix+'Actions.'+name,role:'operation',artifact:'src/expressions.ts',entry_point:name})),
        ...Object.entries(dataFactories).map(([name,factory]) => ({subject:prefix+name,role:'contract',artifact:'src/data.ts',entry_point:factory.name})),
        ...['keepDetails','keepAttempt'].map(name => ({subject:prefix+'Actions.'+name,role:'operation',artifact:'src/data.ts',entry_point:name})),
        ...Object.entries(textFactories).map(([name, factory]) => ({ subject: prefix + name, role: 'contract', artifact: 'src/board.ts', entry_point: factory.name })),
        { subject: prefix + 'ShortList', role: 'contract', artifact: 'src/board.ts', entry_point: 'shortList' },
        { subject: prefix + 'Actions.echo', role: 'operation', artifact: 'src/board.ts', entry_point: 'echo' },
        { subject: prefix + 'Count', role: 'contract', artifact: 'src/board.ts', entry_point: 'count' },
        { subject: prefix + 'Actions.increment', role: 'operation', artifact: 'src/board.ts', entry_point: 'increment' },
      ],
      artifacts: ['src/board.ts','src/data.ts','src/expressions.ts','src/collections.ts','src/decimal.ts','src/entities.ts'], capabilities: ['sessions', 'values', 'invocation', 'events', 'state'],
      representations: { Int: 'native bigint; lossless integer strings on the wire', Decimal:'Exact decimal string', Duration:'Finite number of seconds', Bytes:'Uint8Array, validated before conversion', Option:'Explicit none/some object', Dctx0Result:'Explicit ok/error object', Details:'Native record with field-specific representation mapping' },
    };
  },
  async begin() {
    const board=new entities.Board(initialState.tasks,initialState.notes.map(row=>({...row,sequence:BigInt(row.sequence)})));
    return {
      async validate(subject, input) {
        const name=subject.slice(prefix.length);
        const entityFactory=entityFactories[name as keyof typeof entityFactories];
        if(entityFactory) {
          try{return {kind:'accepted',value:encodeEntity(name,entityFactory(decodeEntity(name,input)))};}
          catch(error){return {kind:'rejected',reason:String(error)};}
        }
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
        if(invocation.subject.startsWith(prefix+'Tasks.')) {
          try {
            switch(invocation.subject.slice((prefix+'Tasks.').length)) {
              case 'list':return {kind:'returned',value:{kind:'list',value:board.list().map(row=>encodeEntity('TaskSummary',row))}};
              case 'find':{const row=board.find(text(invocation.arguments[0]!));return {kind:'returned',value:{kind:'option',value:row.tag==='none'?null:encodeEntity('TaskSummary',row.value)}};}
              case 'notes':return {kind:'returned',value:{kind:'list',value:board.notes(text(invocation.arguments[0]!)).map(row=>encodeEntity('Note',row))}};
            }
          }catch(error){return {kind:'threw',message:String(error)};}
        }
        const operation=invocation.subject.slice((prefix+'Actions.').length);
        if(Object.hasOwn(decimals,operation)) {
          try {
            const implementation=decimals[operation as keyof typeof decimals];
            return {kind:'returned',value:{kind:'decimal',value:implementation(primitive(invocation.arguments[0]!))}};
          } catch(error) {return {kind:'threw',message:String(error)};}
        }
        if(Object.hasOwn(signatures.operations,operation)) {
          try {
            const implementation=collections[operation as keyof typeof collections] as (...args:unknown[])=>unknown;
            return {kind:'returned',value:resultValue(signatures.operations[operation as keyof typeof signatures.operations],implementation(...invocation.arguments.map(primitive)))};
          } catch(error) {return {kind:'threw',message:String(error)};}
        }
        if(Object.hasOwn(expressions,operation)) {
          try {
            const args=invocation.arguments;
            switch(operation) {
              case 'score': return {kind:'returned',value:encode('Integer',expressions.score(integer(args[0]),integer(args[1])))};
              case 'eligible': return {kind:'returned',value:encode('Flag',expressions.eligible(integer(args[0])))};
              case 'describe': return {kind:'returned',value:{kind:'string',value:expressions.describe(text(args[0]!))}};
              case 'named': return {kind:'returned',value:encode('Choice',expressions.named(text(args[0]!)))};
            }
          } catch(error) {return {kind:'threw',message:String(error)};}
        }
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
      async observe(requested) {
        const state=board.snapshot();
        return Object.fromEntries(requested.map(subject=>{
          const name=subject.slice(prefix.length);
          if(name!=='Task'&&name!=='Note') throw new Error('No actual state observer for '+subject);
          return [subject,state[name].map(row=>encodeEntity(name,row))];
        }));
      },
      async close() {},
    };
  },
};
