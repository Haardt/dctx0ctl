import {Database} from 'bun:sqlite';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {candidateOrderWinner} from '../../src/domain/candidate-order-winner.ts';
import {CinemaStore} from '../../src/persistence/sqlite.ts';
import {ApplicationFailure,MovieNight,type SaveFilm} from '../../src/application/movie-night.ts';
import {BindingError,inputValue,projectInput,projectValue,type Invocation,type InvocationEvidence,type OperationFixture,type ResourceObservation,type InvocationObservation,type VerificationEvent,type TypeRef,type Value,type ValueModel} from './library/index.ts';
const root='HomeCinema.MovieNight.';
export type OperationModel={data:ValueModel;operations:Record<string,{parameters:{name:string;ty:TypeRef}[];result:TypeRef}>};
function argumentsOf(invocation:Invocation,model:OperationModel):unknown[]{
 const operation=model.operations[invocation.subject];
 if(!operation||operation.parameters.length!==invocation.arguments.length)throw new BindingError('Operation argument model differs');
 return invocation.arguments.map((value,index)=>{
  const native=inputValue(value,operation.parameters[index]!.ty,model.data);
  // Report the actual converted value on the very Invocation clone supplied
  // to this fixture. Production consumes this native value, never a re-decode.
  invocation.arguments[index]=projectInput(native,value,operation.parameters[index]!.ty,model.data);
  return native;
 });
}
function returned(actual:unknown,invocation:Invocation,model:OperationModel):InvocationEvidence {
 const operation=model.operations[invocation.subject];
 if(!operation)throw new BindingError('Missing operation output model');
 return {observation:{kind:'returned',value:projectValue(actual,operation.result,model.data)},calls:null,events:null,resources:null};
}
export function winnerFixture(model:OperationModel):OperationFixture {
 const subject=root+'ScreeningPlanning.VotingRules.candidateOrderWinner';
 const invoke=(invocation:Invocation)=>{
  if(invocation.subject!==subject)throw new BindingError('Unsupported typed winner continuation');
  const [votes,candidates]=argumentsOf(invocation,model);
  if(!Array.isArray(votes)||!Array.isArray(candidates)||votes.some(id=>typeof id!=='string')||candidates.some(id=>typeof id!=='string'))throw new BindingError('Winner native arguments need string arrays');
  return returned(candidateOrderWinner(votes,candidates),invocation,model);
 };
 return {subject,invokeProduction:invoke,oracle:invoke,cleanup(){}};
}
/** A disk-backed database belongs to one explicit case fixture. Observations
 * read actual persisted rows; cleanup closes the resource before deleting it.
 */
export async function filmSaveFixture(model:OperationModel):Promise<OperationFixture>{
 const subject=root+'Catalog.FilmCatalogStore.save';
 const directory=await mkdtemp(join(tmpdir(),'cinema-native-'));
 const database=new Database(join(directory,'cinema.sqlite3'));
 let store:CinemaStore;
 try{store=new CinemaStore(database);}catch(error){database.close();await rm(directory,{recursive:true,force:true});throw error;}
 const events:VerificationEvent[]=[],calls:InvocationObservation[]=[];
 const observedSave=async(request:SaveFilm)=>{
  const invocation:Invocation={subject,arguments:[projectValue(request,model.operations[subject]!.parameters[0]!.ty,model.data)]};
  try{const actual=await store.ports.catalog.save(request);calls.push({invocation,observation:returned(actual,invocation,model).observation});return actual;}
  catch(error){
   if(error instanceof ApplicationFailure){const declaration=root+error.code;calls.push({invocation,observation:{kind:'returned',value:{kind:'failure',value:{declaration,payload:projectValue(error.payload,{name:declaration,symbol_id:declaration,arguments:[]},model.data)}}}});}
   else calls.push({invocation,observation:{kind:'threw',message:error instanceof Error?error.message:String(error)}});
   throw error;
  }
 };
 const service=new MovieNight({...store.ports,catalog:{...store.ports.catalog,save:observedSave}},event=>{
  const declaration=root+event.name;events.push({subject:declaration,payload:projectValue(event.payload,{name:declaration,symbol_id:declaration,arguments:[]},model.data)});
 });
 const invoke=async(invocation:Invocation,production:(request:SaveFilm)=>Promise<unknown>):Promise<InvocationEvidence>=>{
  events.length=0;calls.length=0;
  const [request]=argumentsOf(invocation,model);
  if(!request||typeof request!=='object'||Array.isArray(request))throw new BindingError('SaveFilm native arguments need a record');
  // SQL instrumentation is scoped to this explicit synchronous SQLite call.
  // Snapshot reads happen outside this scope and never count as production reads.
  const resources:ResourceObservation[]=[];
  const original=database.query;
  database.query=function(sql:string){
   const statement=Reflect.apply(original,database,[sql]);
   return new Proxy(statement,{get(target,key){
    const value=Reflect.get(target,key,target);
    if(typeof value!=='function')return value;
    if(!['get','all','run','values'].includes(String(key)))return value.bind(target);
    return (...args:unknown[])=>{
     try{return Reflect.apply(value,target,args);}finally{
      const write=/^\s*(INSERT|UPDATE|DELETE|REPLACE)\b/i.test(sql),read=/^\s*SELECT\b/i.test(sql);
      const table=sql.match(/\b(?:FROM|JOIN|INTO|UPDATE)\s+([a-z_]+)/i)?.[1]?.toLowerCase();
      const names:Record<string,string>={films:'Film',comments:'Comment',ratings:'Rating',polls:'ScreeningPoll',votes:'FilmVote'};
      if(write||read)resources.push({operation:subject,resource:table&&names[table]?root+names[table]:'unmodeled:sqlite-statement',access:write?'write':'read'});
     }
    };
   }});
  } as Database['query'];
  try{
   const evidence=returned(await production(request as SaveFilm),invocation,model);
   return {...evidence,calls:[...calls],events:[...events],resources};
  }catch(error){
   if(!(error instanceof ApplicationFailure))throw error;
   const declaration=root+error.code;
   return {observation:{kind:'returned',value:{kind:'failure',value:{declaration,payload:projectValue(error.payload,{name:declaration,symbol_id:declaration,arguments:[]},model.data)}}},calls:[...calls],events:[...events],resources};
  }finally{database.query=original;}
 };
 return {subject,
  invokeProduction(invocation){if(invocation.subject!==subject)throw new BindingError('Unsupported typed catalog production call');return invoke(invocation,request=>store.ports.catalog.save(request));},
  setup(invocation){if(invocation.subject!==root+'Catalog.FilmCatalogApi.saveFilm')throw new BindingError('Unsupported typed catalog setup continuation');return invoke(invocation,request=>service.saveFilm(request));},
  readState(entities){
   const actual=store.observe() as Record<string,unknown[]>;
   const values:Record<string,Value[]>={};
   for(const entity of entities){
    const name=entity.startsWith(root)?entity.slice(root.length):'';
    if(!Object.hasOwn(actual,name))throw new BindingError('Actual entity observer unavailable: '+entity);
    if(name==='Film'){
     const rows=database.query('SELECT id,document FROM films').all() as {id:string;document:string}[];
     for(const row of rows){
      let document:{id?:unknown};try{document=JSON.parse(row.document);}catch(error){throw new BindingError('Persisted Film document cannot be projected: '+String(error));}
      if(row.id!==document.id)throw new BindingError('Physical SQLite Film key differs from observed Film.id');
     }
     if(rows.length!==actual[name]!.length)throw new BindingError('Film observer omitted persisted rows');
    }
    values[entity]=actual[name]!.map(value=>projectValue(value,{name:entity,symbol_id:entity,arguments:[]},model.data));
   }
   return {entities:values,complete:true};
  },
  async cleanup(){try{database.close();}finally{await rm(directory,{recursive:true,force:true});}}
 };
}
