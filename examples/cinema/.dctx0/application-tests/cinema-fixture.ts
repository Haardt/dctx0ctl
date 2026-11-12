import {Database} from 'bun:sqlite';
import {AsyncLocalStorage} from 'node:async_hooks';
import {mkdtemp,rm,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {CinemaSession} from '../../src/app/session.ts';
import {ApplicationFailure,type ApplicationEvent} from '../../src/application/movie-night.ts';
import {candidateOrderWinner} from '../../src/domain/candidate-order-winner.ts';
import {createCinemaFacade,observedPorts,type ArgumentReader} from './generated/facade.ts';
import {createHttpBindings} from './http-bindings.ts';
import {BindingError,inputValue,projectInput,projectValue,type CaseContext,type Invocation,type InvocationEvidence,type InvocationObservation,type Observation,type OperationFixture,type ResourceObservation,type VerificationEvent,type Value} from './library/index.ts';
import type {OperationModel} from './operation-fixtures.ts';
const root='HomeCinema.MovieNight.';
type Trace={calls:InvocationObservation[];events:VerificationEvent[];resources:ResourceObservation[]};
type Active={operation:string;trace:Trace};
type Configuration={time?:string;ids?:string[];httpCases?:string[]};
const tables={Film:{table:'films',keys:['id']},Comment:{table:'comments',keys:['id']},Rating:{table:'ratings',keys:['film_id','member_id'],fields:['filmId','memberId']},ScreeningPoll:{table:'polls',keys:['id']},FilmVote:{table:'votes',keys:['poll_id','member_id'],fields:['pollId','memberId']}} as const;
export type CinemaFixture=OperationFixture & {reopen:()=>Promise<void>};
export async function cinemaFixture(model:OperationModel,context:CaseContext,options:{transport?:'http'|'direct'}={}):Promise<CinemaFixture>{
 const configuration=JSON.parse(await readFile(new URL('../testing/typescript/config.json',import.meta.url),'utf8')) as Configuration;
 const directory=await mkdtemp(join(tmpdir(),'cinema-application-')),path=join(directory,'cinema.sqlite3');
 const active=new AsyncLocalStorage<Active>();
 let nextId=0,session:CinemaSession,restoreSql:(()=>void)|undefined,closed=false;
 function operation(subject:string){const result=model.operations[subject];if(!result)throw new BindingError('Missing generated operation model '+subject);return result;}
 function failure(error:unknown):Observation{
  if(error instanceof BindingError)throw error;
  if(error instanceof ApplicationFailure){const declaration=root+error.code;return {kind:'returned',value:{kind:'failure',value:{declaration,payload:projectValue(error.payload,{name:declaration,symbol_id:declaration,arguments:[]},model.data)}}};}
  return {kind:'threw',message:error instanceof Error?error.message:String(error)};
 }
 function returned(subject:string,value:unknown):Observation{return {kind:'returned',value:projectValue(value,operation(subject).result,model.data)};}
 function decodeObservation(subject:string,observed:Observation):unknown{
  if(observed.kind!=='returned')throw new BindingError('Central collaborator did not return a representable outcome: '+JSON.stringify(observed));
  if(observed.value.kind==='failure'){
   const value=observed.value.value,declaration=value.declaration;
   if(!declaration.startsWith(root)||value.payload===null)throw new BindingError('Unsupported collaborator business failure');
   throw new ApplicationFailure(declaration.slice(root.length),inputValue(value.payload,{name:declaration,symbol_id:declaration,arguments:[]},model.data) as Record<string,unknown>);
  }
  return inputValue(observed.value,operation(subject).result,model.data);
 }
 async function observePort<T>(subject:string,args:readonly unknown[],invoke:()=>Promise<T>):Promise<T>{
  const scope=active.getStore();if(!scope)throw new BindingError('Port executed outside an explicit test invocation context');
  const op=operation(subject),invocation={subject,arguments:args.map((value,index)=>projectValue(value,op.parameters[index]!.ty,model.data))};
  // A prepared mock step is closed: every injected port request must be
  // authorized centrally, including unexpected subjects. Empty capability
  // lists retain the explicit real-persistence path for real/setup steps.
  if(context.mocks.length>0){
   const observed=await context.collaborate(invocation);scope.trace.calls.push({invocation,observation:observed});return decodeObservation(subject,observed) as T;
  }
  try{const value=await active.run({operation:subject,trace:scope.trace},invoke);scope.trace.calls.push({invocation,observation:returned(subject,value)});return value;}
  catch(error){scope.trace.calls.push({invocation,observation:failure(error)});throw error;}
 }
 function publish(event:ApplicationEvent){
  const scope=active.getStore();if(!scope)throw new BindingError('Event outside the explicit invocation context');
  const declaration=root+event.name;scope.trace.events.push({subject:declaration,payload:projectValue(event.payload,{name:declaration,symbol_id:declaration,arguments:[]},model.data)});
 }
 function installSql(db:Database){
  const original=db.query,descriptor=Object.getOwnPropertyDescriptor(db,'query');
  db.query=function(sql:string){
   const statement=Reflect.apply(original,db,[sql]);
   return new Proxy(statement,{get(target,key){
    const method=Reflect.get(target,key,target);if(typeof method!=='function')return method;
    if(!['get','all','run','values'].includes(String(key)))return method.bind(target);
    return (...args:unknown[])=>{
     try{return Reflect.apply(method,target,args);}finally{
      const scope=active.getStore();if(scope){
       const read=/^\s*SELECT\b/i.test(sql),write=/^\s*(INSERT|UPDATE|DELETE|REPLACE)\b/i.test(sql);
       const matches=read?[...sql.matchAll(/\b(?:FROM|JOIN)\s+([a-z_]+)/gi)]:[sql.match(/\b(?:INTO|UPDATE|FROM)\s+([a-z_]+)/i)].filter((value):value is RegExpMatchArray=>value!==null);
       if(read||write){
        if(!matches.length)scope.trace.resources.push({operation:scope.operation,resource:'unmodeled:sqlite-statement',access:write?'write':'read'});
        for(const match of matches){const table=match[1].toLowerCase(),entity=Object.entries(tables).find(([,value])=>value.table===table)?.[0];scope.trace.resources.push({operation:scope.operation,resource:entity?root+entity:'unmodeled:sqlite-table:'+table,access:write?'write':'read'});}
       }else scope.trace.resources.push({operation:scope.operation,resource:'unmodeled:sqlite-statement',access:'read'});
      }
     }
    };
   }});
  } as Database['query'];
  restoreSql=()=>{if(descriptor)Object.defineProperty(db,'query',descriptor);else Reflect.deleteProperty(db,'query');};
 }
 function open(){
  session=new CinemaSession(path,publish,configuration.time&&configuration.ids?{now:()=>configuration.time!,id:()=>{const value=configuration.ids![nextId++];if(!value)throw new BindingError('Controlled Cinema ID corpus exhausted');return value;}}:undefined,actual=>observedPorts(actual,observePort));
  installSql(session.store.db);
 }
 try{open();}catch(error){await rm(directory,{recursive:true,force:true});throw error;}
 const arguments_:ArgumentReader={argument<T>(invocation:Invocation,index:number){
  const parameter=operation(invocation.subject).parameters[index];if(!parameter||index>=invocation.arguments.length)throw new BindingError('Generated native argument arity differs');
  const original=invocation.arguments[index],native=inputValue(original,parameter.ty,model.data);
  invocation.arguments[index]=projectInput(native,original,parameter.ty,model.data);return native as T;
 }};
 async function invoke(invocation:Invocation):Promise<InvocationEvidence>{
  if(invocation.arguments.length!==operation(invocation.subject).parameters.length)throw new BindingError('Native argument arity differs');
  const trace:Trace={calls:[],events:[],resources:[]};
  const service=(options.transport==='http'||options.transport!=='direct'&&configuration.httpCases?.includes(context.id))?createHttpBindings(session.http,model):session.service;
  const facade=createCinemaFacade({service,ports:session.store.ports,rules:{candidateOrderWinner}});
  let observation:Observation;
  try{const value=await active.run({operation:invocation.subject,trace},()=>facade(invocation,arguments_));observation=returned(invocation.subject,value);}
  catch(error){observation=failure(error);}
  return {observation,...trace};
 }
 return {subject:context.initialSubject??'',subjects:Object.keys(model.operations),invokeProduction:invoke,setup:invoke,oracle:invoke,
  readState(entities){
   const actual=session.store.observe() as Record<string,unknown[]>;const values:Record<string,Value[]>={};
   for(const entity of entities){
    const name=entity.startsWith(root)?entity.slice(root.length):'',storage=tables[name as keyof typeof tables];
    if(!storage||!Object.hasOwn(actual,name))throw new BindingError('Unknown complete entity observer '+entity);
    const rows=session.store.db.query('SELECT * FROM '+storage.table).all() as Record<string,string>[];
    for(const row of rows){const document=JSON.parse(row.document) as Record<string,unknown>;for(let index=0;index<storage.keys.length;index++){const field='fields' in storage?storage.fields[index]:storage.keys[index];if(row[storage.keys[index]]!==document[field])throw new BindingError('Physical SQLite key differs from observed '+entity);}}
    if(rows.length!==actual[name]!.length)throw new BindingError('Entity observer omitted persisted rows');
    values[entity]=actual[name]!.map(value=>projectValue(value,{name:entity,symbol_id:entity,arguments:[]},model.data));
   }
   return {entities:values,complete:true};
  },
  async reopen(){if(closed)throw new BindingError('Cannot reopen a closed fixture');restoreSql?.();await session.close();open();},
  async cleanup(){if(closed)return;closed=true;try{restoreSql?.();await session.close();}finally{await rm(directory,{recursive:true,force:true});}},
 };
}
