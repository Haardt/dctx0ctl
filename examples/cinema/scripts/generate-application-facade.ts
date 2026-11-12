/** Generate local typed calls from the one published operation inventory.
 * Authored connections bind owners, not a manually maintained operation registry.
 */
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
type Operation={id:string;parameters:{name:string}[]};
type Bundle={model:{operations:Record<string,Operation>}};
const file=process.argv[2];
if(!file)throw new Error('Usage: bun generate-application-facade.ts PREPARED_BUNDLE');
const bytes=await readFile(file),bundle=JSON.parse(bytes.toString()) as Bundle;
const root='HomeCinema.MovieNight.';
const owners={
 'Catalog.FilmCatalogApi':{expression:'bindings.service',type:'MovieNight'},
 'Catalog.FilmCatalogStore':{expression:'bindings.ports.catalog',type:"MovieNightPorts['catalog']",port:'catalog'},
 'Community.FilmCommunityApi':{expression:'bindings.service',type:'MovieNight'},
 'Community.FilmPresence':{expression:'bindings.ports.presence',type:"MovieNightPorts['presence']",port:'presence'},
 'Community.CommentStore':{expression:'bindings.ports.comments',type:"MovieNightPorts['comments']",port:'comments'},
 'Community.RatingStore':{expression:'bindings.ports.ratings',type:"MovieNightPorts['ratings']",port:'ratings'},
 'ScreeningPlanning.ScreeningPlanningApi':{expression:'bindings.service',type:'MovieNight'},
 'ScreeningPlanning.VotingStore':{expression:'bindings.ports.voting',type:"MovieNightPorts['voting']",port:'voting'},
 'ScreeningPlanning.VotingRules':{expression:'bindings.rules',type:'VotingRules'},
} satisfies Record<string,{expression:string;type:string;port?:string}>;
const methods=new Map<string,string[]>();const cases:string[]=[];
for(const operation of Object.values(bundle.model.operations).sort((a,b)=>a.id<b.id?-1:a.id>b.id?1:0)){
 if(!operation.id.startsWith(root))throw new Error('Unbound operation '+operation.id);
 const local=operation.id.slice(root.length),at=local.lastIndexOf('.'),owner=local.slice(0,at),method=local.slice(at+1);
 if(!/^[A-Za-z_][A-Za-z_0-9]*$/.test(method))throw new Error('Invalid native method '+method);
 const connection=owners[owner as keyof typeof owners];
 if(!connection)throw new Error('Unbound owner '+owner);
 const callable=connection.type+"['"+method+"']";
 const args=operation.parameters.map((_,i)=>`input.argument<Parameters<${callable}>[${i}]>(invocation,${i})`).join(',');
 cases.push(`  case ${JSON.stringify(operation.id)}: return ${connection.expression}.${method}(${args});`);
 if('port' in connection){
  const members=methods.get(connection.port)??[];
  members.push(`   ${method}:(...args:Parameters<${callable}>)=>observe(${JSON.stringify(operation.id)},args,()=>actual.${connection.port}.${method}(...args)),`);
  methods.set(connection.port,members);
 }
}
const source=`// Generated from the published model, not a second case generator.
// model-sha256: ${createHash('sha256').update(JSON.stringify(bundle.model)).digest('hex')}
import type {MovieNight,MovieNightPorts} from '../../../src/application/movie-night.ts';
import type {VotingRules} from '../../../src/domain/voting-rules.ts';
import {BindingError,type Invocation} from '../library/index.ts';
export type SurfaceBindings=Pick<MovieNight,${JSON.stringify(Object.values(bundle.model.operations).filter(operation=>operation.id.includes('Api.')).map(operation=>operation.id.split('.').at(-1))).slice(1,-1).split(',').join('|')}>;
export type NativeBindings={service:SurfaceBindings;ports:MovieNightPorts;rules:VotingRules};
export type ArgumentReader={argument<T>(invocation:Invocation,index:number):T};
export function createCinemaFacade(bindings:NativeBindings){
 return async(invocation:Invocation,input:ArgumentReader):Promise<unknown>=>{
  switch(invocation.subject){
${cases.join('\n')}
   default:throw new BindingError('No generated native connection for '+invocation.subject);
  }
 };
}
export function observedPorts(actual:MovieNightPorts,observe:<T>(subject:string,args:readonly unknown[],invoke:()=>Promise<T>)=>Promise<T>):MovieNightPorts {
 return {
${[...methods].map(([port,members])=>`  ${port}:{\n${members.join('\n')}\n  },`).join('\n')}
 };
}
`;
const destination=new URL('../.dctx0/application-tests/generated/',import.meta.url);
await mkdir(destination,{recursive:true});await writeFile(new URL('facade.ts',destination),source);
