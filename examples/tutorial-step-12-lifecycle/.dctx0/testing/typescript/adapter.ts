import {SystemClock,Diagnostics} from '../../../src/diagnostics.ts';
import {observeBoard} from './resource-observer.ts';
import {MissingTask,TaskConflict,missingTask,taskViewed} from '../../../src/events.ts';
import mockCases from './mock-cases.json';
import {BoardReader,Catalog} from '../../../src/catalog.ts';
import type {Option} from '../../../src/data.ts';
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
        {subject:prefix+'TaskConflict',role:'contract',artifact:'src/events.ts',entry_point:'missingTask'},
        ...['insert','replace','save','remove','saveNote'].map(name=>({subject:prefix+'Catalog.TaskWriter.'+name,role:'port',artifact:'src/entities.ts',entry_point:'Board.'+name})),
        {subject:prefix+'Signals.Clock.now',role:'operation',artifact:'src/diagnostics.ts',entry_point:'SystemClock.now'},
        {subject:prefix+'Diagnostics.timestamp',role:'operation',artifact:'src/diagnostics.ts',entry_point:'Diagnostics.timestamp'},
        {subject:prefix+'MissingTask',role:'contract',artifact:'src/events.ts',entry_point:'missingTask'},
        {subject:prefix+'TaskViewed',role:'contract',artifact:'src/events.ts',entry_point:'taskViewed'},
        {subject:prefix+'Catalog.Public.requireTask',role:'operation',artifact:'src/catalog.ts',entry_point:'Catalog.requireTask'},
        {subject:prefix+'Catalog.TaskReader.lookup',role:'port',artifact:'src/catalog.ts',entry_point:'BoardReader.lookup'},
        {subject:prefix+'Catalog.Public.find',role:'operation',artifact:'src/catalog.ts',entry_point:'Catalog.find'},
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
      artifacts: ['src/board.ts','src/data.ts','src/expressions.ts','src/collections.ts','src/decimal.ts','src/entities.ts','src/catalog.ts','src/events.ts','src/diagnostics.ts'], capabilities: ['sessions', 'values', 'invocation', 'events', 'state','real-ports','call-observation','mock-injection','resource-observation'],
      representations: { Int: 'native bigint; lossless integer strings on the wire', Decimal:'Exact decimal string', Duration:'Finite number of seconds', Bytes:'Uint8Array, validated before conversion', Option:'Explicit none/some object', Dctx0Result:'Explicit ok/error object', Details:'Native record with field-specific representation mapping' },
    };
  },
  async begin(_caseId,context) {
    const board=new entities.Board(initialState.tasks,initialState.notes.map(row=>({...row,sequence:BigInt(row.sequence)})));
    const resources=observeBoard(board,context);
    const clock=new SystemClock();
    const diagnostics=new Diagnostics(()=>resources.observe(prefix+'Signals.Clock.now',()=>clock.now()));
    const reader=new BoardReader(board);
    const wireSummary=(row:Option<entities.TaskSummary>):Value=>({kind:'option',value:row.tag==='none'?null:encodeEntity('TaskSummary',row.value)});
    const catalog=new Catalog({async lookup(id){
      if(mockCases.cases.includes(_caseId)) {
        const invocation={subject:prefix+'Catalog.TaskReader.lookup',arguments:[{kind:'string' as const,value:id}]};
        const reply=await context.collaborator(invocation);context.recordCall(invocation,reply);
        if(reply.kind!=='returned') throw new Error(reply.kind==='threw'?reply.message:reply.reason);
        if(reply.value.kind!=='option') throw new TypeError('Reader reply has no option representation');
        if(reply.value.value===null)return {tag:'none'};
        return {tag:'some',value:entities.taskSummary(decodeEntity('TaskSummary',reply.value.value))};
      }
      const invocation={subject:prefix+'Catalog.TaskReader.lookup',arguments:[{kind:'string' as const,value:id}]};
      try {const output=reader.lookup(id);context.recordCall(invocation,{kind:'returned',value:wireSummary(output)});return output;}
      catch(error){context.recordCall(invocation,{kind:'threw',message:String(error)});throw error;}
    }},event=>context.emit(prefix+'TaskViewed',{kind:'record',value:{taskId:{kind:'string',value:event.taskId}}}));
    return {
      async validate(subject, input) {
        const name=subject.slice(prefix.length);
        if(name==='MissingTask'||name==='TaskConflict'||name==='TaskViewed') {
          try {
            const native=primitive(input);
            const output=name!=='TaskViewed'?missingTask(native):taskViewed(native);
            const payload:Value={kind:'record',value:{taskId:{kind:'string',value:output.taskId}}};
            return {kind:'accepted',value:payload};
          }catch(error){return {kind:'rejected',reason:String(error)};}
        }
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
        return await resources.run(invocation.subject,async()=>{
        if(invocation.subject.startsWith(prefix+'Catalog.TaskWriter.')) {
          try {
            const operation=invocation.subject.slice((prefix+'Catalog.TaskWriter.').length);
            if(operation==='remove'){board.remove(text(invocation.arguments[0]!));return {kind:'returned',value:{kind:'unit'}};}
            const entity=operation==='saveNote'?'Note':'Task',request=decodeEntity(entity,invocation.arguments[0]!);
            let output:unknown;
            switch(operation){
              case 'insert':output=board.insert(request);break;
              case 'replace':output=board.replace(request);break;
              case 'save':output=board.save(request);break;
              case 'saveNote':output=board.saveNote(request);break;
              default:throw new Error('Unknown writer entry point');
            }
            return {kind:'returned',value:encodeEntity(entity,output)};
          }catch(error){
            if(error instanceof MissingTask||error instanceof TaskConflict)return {kind:'returned',value:{kind:'failure',value:{declaration:prefix+error.name,payload:{kind:'record',value:{taskId:{kind:'string',value:error.payload.taskId}}}}}};
            return {kind:'threw',message:String(error)};
          }
        }
        if(invocation.subject===prefix+'Signals.Clock.now'||invocation.subject===prefix+'Diagnostics.timestamp') {
          return {kind:'returned',value:{kind:'string',value:invocation.subject.endsWith('.now')?clock.now():diagnostics.timestamp()}};
        }
        if(invocation.subject===prefix+'Catalog.Public.requireTask') {
          try{return {kind:'returned',value:encodeEntity('TaskSummary',await catalog.requireTask(text(invocation.arguments[0]!)))};}
          catch(error){
            if(error instanceof MissingTask)return {kind:'returned',value:{kind:'failure',value:{declaration:prefix+'MissingTask',payload:{kind:'record',value:{taskId:{kind:'string',value:error.payload.taskId}}}}}};
            return {kind:'threw',message:String(error)};
          }
        }
        if(invocation.subject===prefix+'Catalog.TaskReader.lookup'||invocation.subject===prefix+'Catalog.Public.find') {
          try {const id=text(invocation.arguments[0]!);return {kind:'returned',value:wireSummary(invocation.subject.endsWith('lookup')?reader.lookup(id):await catalog.find(id))};}
          catch(error){return {kind:'threw',message:String(error)};}
        }
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
        });
      },
      async observe(requested) {
        const state=board.snapshot();
        return Object.fromEntries(requested.map(subject=>{
          const name=subject.slice(prefix.length);
          if(name!=='Task'&&name!=='Note') throw new Error('No actual state observer for '+subject);
          return [subject,state[name].map(row=>encodeEntity(name,row))];
        }));
      },
      async close() {resources.close();},
    };
  },
};
