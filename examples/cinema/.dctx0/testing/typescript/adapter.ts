/** Reviewed testing adapter; real services, translated values, injected ports. */
import {ApplicationFailure,MovieNight,type MovieNightPorts} from '../../../src/application/movie-night.ts';
import {candidateOrderWinner} from '../../../src/domain/candidate-order-winner.ts';
import type {ProjectAdapter,Value,Observation,ValueCheck} from './contract.ts';
import schema from './model.json';
type Type = {name:string;symbol_id:string|null;arguments:Type[]};
type Data = {alias:Type|null;fields:{name:string;ty:Type}[];variants:{name:string;payload:Type|null}[]};
type Operation = {parameters:{name:string;ty:Type}[];result:Type;failures:Type[]};
const data = schema.data as Record<string,Data>;
const operations = schema.operations as Record<string,Operation>;
const root='HomeCinema.MovieNight.';
const configurationFile=Bun.file(new URL('./config.json',import.meta.url));
type TestingConfiguration={mode:'mock'|'real'|'adaptive';database?:string;time?:string;ids?:string[];realCases?:string[]};
const configuration:TestingConfiguration=await configurationFile.exists()?await configurationFile.json() as TestingConfiguration: {mode:'mock'};
if(!['mock','real','adaptive'].includes(configuration.mode))throw new Error('Unsupported Cinema adapter mode');
class AdapterTranslationError extends Error {}
function decode(value:Value):unknown {
 switch(value.kind){
  case 'integer':{const number=Number(value.value);if(!Number.isSafeInteger(number)||BigInt(value.value)!==BigInt(number))throw new AdapterTranslationError('Unrepresentable Int');return number;}
  case 'float':{const number=Number(value.value);if(!Number.isFinite(number))throw new AdapterTranslationError('Unrepresentable Float');return number;}
  case 'decimal':throw new AdapterTranslationError('Exact Decimal requires an application-specific representation');
  case 'unit':return undefined;
  case 'boolean':case 'string':return value.value;
  case 'list':return value.value.map(decode);
  case 'option':return value.value===null?undefined:decode(value.value);
  case 'record':return Object.fromEntries(Object.entries(value.value).map(([key,value])=>[key,decode(value)]));
  case 'variant':if(value.value.payload!==null)throw new AdapterTranslationError('Payload variants need an application-specific representation');return value.value.name;
  case 'failure':throw new ApplicationFailure(value.value.declaration.replace(root,''),value.value.payload?decode(value.value.payload) as Record<string,unknown>:{});
 }
}
function encode(value:unknown,ty:Type):Value {
 if(ty.symbol_id){
  const declaration=data[ty.symbol_id];if(!declaration)throw new AdapterTranslationError('Missing adapter type descriptor');
  if(declaration.alias)return encode(value,declaration.alias);
  if(declaration.variants.length){if(typeof value!=='string'||!declaration.variants.some(v=>v.name===value&&!v.payload))throw new AdapterTranslationError('Invalid application alternative');return {kind:'variant',value:{declaration:ty.symbol_id,name:value,payload:null}};}
  if(!value||typeof value!=='object'||Array.isArray(value))throw new AdapterTranslationError('Expected application record');
  const record=value as Record<string,unknown>;
  if(Object.keys(record).some(key=>!declaration.fields.some(field=>field.name===key)))throw new AdapterTranslationError('Application record has unexpected fields');
  return {kind:'record',value:Object.fromEntries(declaration.fields.map(field=>[field.name,encode(record[field.name],field.ty)]))};
 }
 switch(ty.name){
  case 'Option':return {kind:'option',value:value===undefined?null:encode(value,ty.arguments[0])};
  case 'List':if(!Array.isArray(value))throw new AdapterTranslationError('Expected application list');return {kind:'list',value:value.map(value=>encode(value,ty.arguments[0]))};
  case 'Int':if(typeof value!=='number'||!Number.isSafeInteger(value))throw new AdapterTranslationError('Unrepresentable application Int');return {kind:'integer',value:String(value)};
  case 'Float':if(typeof value!=='number'||!Number.isFinite(value))throw new AdapterTranslationError('Unrepresentable application Float');return {kind:'float',value:String(value)};
  case 'String':case 'UUID':case 'Timestamp':if(typeof value!=='string')throw new AdapterTranslationError('Expected application text');return {kind:'string',value};
  case 'Boolean':if(typeof value!=='boolean')throw new AdapterTranslationError('Expected application Boolean');return {kind:'boolean',value};
  case 'Unit':if(value!==undefined)throw new AdapterTranslationError('Expected application void');return {kind:'unit'};
  default:throw new AdapterTranslationError('Unsupported application representation: '+ty.name);
 }
}
const surfaces:Record<string,string>={
 'Catalog.FilmCatalogApi.searchFilms':'searchFilms','Catalog.FilmCatalogApi.getFilm':'getFilm',
 'Catalog.FilmCatalogApi.saveFilm':'saveFilm','Catalog.FilmCatalogApi.changeFilmAvailability':'changeFilmAvailability',
 'Community.FilmCommunityApi.listComments':'listComments','Community.FilmCommunityApi.addComment':'addComment',
 'Community.FilmCommunityApi.getFilmRating':'getFilmRating','Community.FilmCommunityApi.rateFilm':'rateFilm',
 'ScreeningPlanning.ScreeningPlanningApi.openVoting':'openVoting','ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting':'getCurrentVoting',
 'ScreeningPlanning.ScreeningPlanningApi.castVote':'castVote','ScreeningPlanning.ScreeningPlanningApi.closeVoting':'closeVoting',
};
const portMappings:Record<string,[string,string]>={
 'Catalog.FilmCatalogStore.search':['catalog','search'],'Catalog.FilmCatalogStore.find':['catalog','find'],'Catalog.FilmCatalogStore.save':['catalog','save'],'Catalog.FilmCatalogStore.changeAvailability':['catalog','changeAvailability'],
 'Community.FilmPresence.exists':['presence','exists'],'Community.CommentStore.listByFilm':['comments','listByFilm'],'Community.CommentStore.append':['comments','append'],'Community.RatingStore.summary':['ratings','summary'],'Community.RatingStore.upsert':['ratings','upsert'],
 'ScreeningPlanning.VotingStore.open':['voting','open'],'ScreeningPlanning.VotingStore.current':['voting','current'],'ScreeningPlanning.VotingStore.cast':['voting','cast'],'ScreeningPlanning.VotingStore.close':['voting','close'],
};
function failureObservation(error:unknown):Observation{
 if(error instanceof ApplicationFailure){const declaration=root+error.code;return {kind:'returned',value:{kind:'failure',value:{declaration,payload:encode(error.payload,{name:error.code,symbol_id:declaration,arguments:[]})}}};}
 return {kind:'threw',message:error instanceof Error?error.message:String(error)};
}
const supportsReal=configuration.mode!=='mock';
export const adapter:ProjectAdapter={
 async describe(){return {protocol_version:1,subjects:[{subject:root+'FilmId',role:'contract',artifact:'src/domain/film-id.ts',entry_point:'filmId'},{subject:root+'ReleaseYear',role:'contract',artifact:'src/domain/release-year.ts',entry_point:'releaseYear'},...(supportsReal?Object.keys(portMappings).map(subject=>({subject:root+subject,role:'port',artifact:'src/persistence/sqlite.ts',entry_point:portMappings[subject].join('.')})):[]),...Object.entries(surfaces).map(([subject,entry_point])=>({subject:root+subject,role:'operation',artifact:'src/application/movie-night.ts',entry_point:'MovieNight.'+entry_point})),{subject:root+'ScreeningPlanning.VotingRules.candidateOrderWinner',role:'operation',artifact:'src/domain/candidate-order-winner.ts',entry_point:'candidateOrderWinner'}],artifacts:[...(supportsReal?['src/app/session.ts','src/http/app.ts','src/persistence/sqlite.ts','src/domain/release-year.ts']:[]),'src/application/movie-night.ts','src/domain/candidate-order-winner.ts','src/domain/film-id.ts','src/domain/voting-rules.ts'],capabilities:['sessions','invocation','events','values','call-observation',...(configuration.mode==='adaptive'?['mock-injection']:[]),...(supportsReal?['state','real-ports',...(configuration.time&&configuration.ids?['controlled-time','controlled-ids']:[])]:['mock-injection'])],representations:{Int:'lossless safe JavaScript integer',Float:'finite IEEE-754 number',UUID:'text',Timestamp:'RFC3339 text',Option:'undefined or value',variants:'alternative name'}};},
 async begin(_caseId,context){
  const real=configuration.mode==='real'||(configuration.mode==='adaptive'&&configuration.realCases?.includes(_caseId)===true);
  async function port(subject:string,args:unknown[]):Promise<unknown>{
   const operation=operations[root+subject];if(!operation)throw new AdapterTranslationError('Unknown port mapping');
   const invocation={subject:root+subject,arguments:args.map((value,index)=>encode(value,operation.parameters[index].ty))};
   const reply=await context.collaborator(invocation);context.recordCall(invocation,reply);
   if(reply.kind==='unavailable')throw new AdapterTranslationError(reply.reason);
   if(reply.kind==='threw')throw new AdapterTranslationError(reply.message);
   return decode(reply.value);
  }
  const ports:MovieNightPorts={
   catalog:{search:async value=>await port('Catalog.FilmCatalogStore.search',[value]) as never,find:async value=>await port('Catalog.FilmCatalogStore.find',[value]) as never,save:async value=>await port('Catalog.FilmCatalogStore.save',[value]) as never,changeAvailability:async value=>await port('Catalog.FilmCatalogStore.changeAvailability',[value]) as never},
   presence:{exists:async value=>await port('Community.FilmPresence.exists',[value]) as never},
   comments:{listByFilm:async value=>await port('Community.CommentStore.listByFilm',[value]) as never,append:async value=>await port('Community.CommentStore.append',[value]) as never},
   ratings:{summary:async (film,member)=>await port('Community.RatingStore.summary',[film,member]) as never,upsert:async value=>await port('Community.RatingStore.upsert',[value]) as never},
   voting:{open:async value=>await port('ScreeningPlanning.VotingStore.open',[value]) as never,current:async()=>await port('ScreeningPlanning.VotingStore.current',[]) as never,cast:async value=>await port('ScreeningPlanning.VotingStore.cast',[value]) as never,close:async value=>await port('ScreeningPlanning.VotingStore.close',[value]) as never},
  };
  const publish=(event:import('../../../src/application/movie-night.ts').ApplicationEvent)=>context.emit(root+event.name,encode(event.payload,{name:event.name,symbol_id:root+event.name,arguments:[]}));
  let session:import('../../../src/app/session.ts').CinemaSession|undefined;
  let service:MovieNight;
  let actualPorts:MovieNightPorts|undefined;
  let productionPorts:MovieNightPorts|undefined;
  if(real){
   if(!configuration.database)throw new AdapterTranslationError('Real testing needs an explicit database');
   const {CinemaSession}=await import('../../../src/app/session.ts');
   let nextId=0;
   const sources=configuration.time&&configuration.ids?{now:()=>configuration.time!,id:()=>{const id=configuration.ids![nextId++];if(!id)throw new AdapterTranslationError('Controlled ID corpus exhausted');return id;}}:undefined;
   session=new CinemaSession(configuration.database,publish,sources,actual=>{
   actualPorts=actual;productionPorts=actual;
   // Register all real port observers before resolving the scoped consumer.
   const traced:Record<string,Record<string,Function>>={};
   for(const [subject,[group,method]] of Object.entries(portMappings)){
    traced[group]??={};const receiver=(actualPorts as unknown as Record<string,Record<string,Function>>)[group];
    traced[group][method]=async(...args:unknown[])=>{
     const operation=operations[root+subject];const invocation={subject:root+subject,arguments:args.map((value,index)=>encode(value,operation.parameters[index].ty))};
     let value:unknown;
     try{value=await receiver[method].apply(receiver,args);}catch(error){context.recordCall(invocation,failureObservation(error));throw error;}
     context.recordCall(invocation,{kind:'returned',value:encode(value,operation.result)});return value;
    };
   }
   actualPorts=traced as unknown as MovieNightPorts;return actualPorts;
   });
   service=session.service;
  }else service=new MovieNight(ports,publish);
  return {
   async validate(subject:string,value:Value):Promise<ValueCheck>{
    if(subject!==root+'FilmId'&&subject!==root+'ReleaseYear')return {kind:'unavailable',reason:'No production factory for this declaration'};
    const input=decode(value);
    const factory=subject===root+'FilmId'?(await import('../../../src/domain/film-id.ts')).filmId:(await import('../../../src/domain/release-year.ts')).releaseYear;
    let result:unknown;
    try{result=factory(input);
    }catch(error){return {kind:'rejected',reason:error instanceof Error?error.message:String(error)};}
    return {kind:'accepted',value:encode(result,{name:subject.slice(root.length),symbol_id:subject,arguments:[]})};
   },
   async invoke(invocation):Promise<Observation>{
    const operation=operations[invocation.subject];if(!operation)throw new AdapterTranslationError('No adapter operation mapping');
    const args=invocation.arguments.map(decode),name=surfaces[invocation.subject.replace(root,'')];
    const isWinner=invocation.subject===root+'ScreeningPlanning.VotingRules.candidateOrderWinner';
    const realPort=real?portMappings[invocation.subject.replace(root,'')]:undefined;
    if(!isWinner&&!name&&!realPort)return {kind:'unavailable',reason:'No application entry point for selected subject'};
    let result:unknown;
    try{
     if(isWinner)result=candidateOrderWinner(args[0] as string[],args[1] as string[]);
     else if(realPort){const receiver=(productionPorts as unknown as Record<string,Record<string,Function>>)[realPort[0]];result=await receiver[realPort[1]].apply(receiver,args);}
     else {if(!name)throw new AdapterTranslationError('No application entry point');const callable=(service as unknown as Record<string,(...args:unknown[])=>unknown>)[name];result=await callable.apply(service,args);}
    }catch(error){
     if(error instanceof AdapterTranslationError)throw error;
     return failureObservation(error);
    }
    return {kind:'returned',value:encode(result,operation.result)};
   },
   async observe(entities){
    if(!session)throw new AdapterTranslationError('No real state observer in mock mode');
    const state=session.store.observe() as Record<string,unknown[]>;
    return Object.fromEntries(entities.map(entity=>{const name=entity.replace(root,'');if(!(name in state))throw new AdapterTranslationError('No actual observer for entity '+entity);return [entity,state[name].map(row=>encode(row,{name,symbol_id:entity,arguments:[]}))];}));
   },
   async close(){if(session)await session.close();},
  };
 },
};
