import {test,expect} from 'bun:test';
import {operationTest,stateChangeTest,assertionTest,BindingError,type Directive,type Invocation,type InvocationEvidence,type Reply,type Request,type OperationFixture} from './index.ts';
const invocation:Invocation={subject:'Store.save',arguments:[{kind:'integer',value:'1'}]};
const evidence:InvocationEvidence={observation:{kind:'returned',value:{kind:'unit'}},calls:[],events:[],resources:[]};
function fixtureClient(directives:Directive[],form='state-change'){
 const requests:Request[]=[];let began=false,action=0;
 const client={async exchange(request:Request):Promise<Reply>{
  requests.push(request);
  if(request.kind==='register')return {kind:'registered'};
  if(request.kind==='next'){if(began)return {kind:'no-more-cases'};began=true;return {kind:'case',context:1,id:'case',form,directive:directives[action++]!};}
  if(request.kind==='observe')return {kind:'action',context:1,directive:request.observation.kind==='binding-error'?{kind:'verdict',result:{status:'UNKNOWN'}}:directives[action++]!};
  if(request.kind==='close-case')return {kind:'case-closed',context:1,result:request.cleanup_error?{status:'UNKNOWN'}:{status:'PASS'}};
  return {kind:'test-closed'};
 }};
 return {client:client as import('./client.ts').AssessmentClient,requests};
}
const before:Directive={kind:'read-state',entities:['Film'],phase:'before',invocation};
const production:Directive={kind:'invoke',phase:'production',invocation};
const after:Directive={kind:'read-state',entities:['Film'],phase:'after',invocation};
const verdict:Directive={kind:'verdict',result:{status:'PASS'}};
test('state helper owns before/production/after/cleanup with exact observations',async()=>{
 const {client,requests}=fixtureClient([before,production,after,verdict]);const order:string[]=[];
 await stateChangeTest(client,'save',()=>({subject:invocation.subject,invokeProduction(){order.push('production');return evidence;},readState(entities,phase){order.push(phase);return {entities:{Film:[]},complete:true};},cleanup(){order.push('cleanup');}}));
 expect(order).toEqual(['before','production','after','cleanup']);
 expect(requests.filter(r=>r.kind==='observe').map(r=>r.observation.kind)).toEqual(['state','invoked','state']);
});
test('setup can require its own before snapshot before the production snapshot',async()=>{
 const setup={subject:'Store.save',arguments:[{kind:'integer',value:'0'}]} as Invocation;
 const {client}=fixtureClient([{...before,invocation:setup},{kind:'invoke',phase:'setup',invocation:setup},before,production,after,verdict]);const order:string[]=[];
 await stateChangeTest(client,'save',()=>({subject:invocation.subject,setup(){order.push('setup');return evidence;},invokeProduction(){order.push('production');return evidence;},readState(entities,phase){order.push(phase);return {entities:{Film:[]},complete:true};},cleanup(){}}));
 expect(order).toEqual(['before','setup','before','production','after']);
});
test('oracle invocation stays separate from production call evidence',async()=>{
 const {client,requests}=fixtureClient([production,{...production,phase:'oracle'},verdict],'operation');
 await operationTest(client,'pure',()=>({subject:invocation.subject,invokeProduction:()=>evidence,oracle:()=>({...evidence,calls:null}),cleanup(){}}));
 const observations=requests.filter(r=>r.kind==='observe').map(r=>r.observation);
 expect(observations).toHaveLength(2);
 expect(observations[1]).toEqual({kind:'invoked',invocation,...evidence,calls:null});
});
test.each(['missing-state','wrong-subject','missing-continuation','cleanup','technical'] as const)('%s cannot produce silent success',async mode=>{
 const {client,requests}=fixtureClient(mode==='missing-continuation'?[{...production,phase:'setup'},verdict]:[before,production,after,verdict]);
 const fixture:OperationFixture={subject:mode==='wrong-subject'?'Other':invocation.subject,invokeProduction(){if(mode==='technical')throw new Error('crash');return evidence;},readState:mode==='missing-state'?undefined:()=>({entities:{Film:[]},complete:true}),cleanup(){if(mode==='cleanup')throw new Error('close failed');}};
 if(mode==='technical')await stateChangeTest(client,'save',()=>fixture);else await expect(stateChangeTest(client,'save',()=>fixture)).rejects.toThrow();
 const observations=requests.filter(r=>r.kind==='observe').map(r=>r.observation);
 if(mode==='technical')expect(observations.some(o=>o.kind==='invoked'&&o.observation.kind==='threw')).toBe(true);
 if(mode==='cleanup')expect(requests.find(r=>r.kind==='close-case')).toEqual({kind:'close-case',context:1,cleanup_error:'close failed'});
});
test('callback argument edits remain visible in actual invocation observations',async()=>{
 const {client,requests}=fixtureClient([production,verdict],'operation');
 await operationTest(client,'mutated',()=>({subject:invocation.subject,invokeProduction(actual){actual.arguments=[{kind:'integer',value:'2'}];return evidence;},cleanup(){}}));
 const observation=requests.find(r=>r.kind==='observe');
 expect(observation?.kind==='observe'&&observation.observation.kind==='invoked'&&observation.observation.invocation.arguments).toEqual([{kind:'integer',value:'2'}]);
});
test('setup oracle continuation can run before the production call',async()=>{
 const {client}=fixtureClient([{...production,phase:'setup'},{...production,phase:'oracle'},production,verdict],'operation');const order:string[]=[];
 await operationTest(client,'continuation',()=>({subject:invocation.subject,setup(){order.push('setup');return evidence;},oracle(){order.push('oracle');return evidence;},invokeProduction(){order.push('production');return evidence;},cleanup(){}}));
 expect(order).toEqual(['setup','oracle','production']);
});

test('assertion cases use only pure local oracle calls, with no production trace',async()=>{
 const {client,requests}=fixtureClient([{...production,phase:'oracle'},{...production,phase:'oracle'},verdict],'assertion');
 await assertionTest(client,'property',()=>({subject:invocation.subject,invokeProduction(){throw new Error('Must never execute');},oracle:()=>evidence,cleanup(){}}));
 expect(requests.filter(r=>r.kind==='observe')).toHaveLength(2);
});
test.each(['scenario','lifecycle'] as const)('%s keeps one fixture through multiple ordered stateful steps',async form=>{
 const helper=form==='scenario'?(await import('./operation.ts')).scenarioTest:(await import('./operation.ts')).lifecycleTest;
 const {client}=fixtureClient([before,production,after,before,production,after,verdict],form);let fixtures=0;const order:string[]=[];
 await helper(client,'connected',()=>{fixtures++;return {subject:invocation.subject,invokeProduction(){order.push('production');return evidence;},readState(entities,phase){order.push(phase);return {entities:{Film:[]},complete:true};},cleanup(){order.push('cleanup');}};});
 expect(fixtures).toBe(1);expect(order).toEqual(['before','production','after','before','production','after','cleanup']);
});
test('central action updates mock capabilities on the same explicit case context',async()=>{
 const {scenarioTest}=await import('./operation.ts');let began=false,step=0;const actual:string[]=[];let retained:import('./context.ts').CaseContext|undefined;
 const client={async exchange(request:Request):Promise<Reply>{
  if(request.kind==='register')return {kind:'registered'};
  if(request.kind==='next'){if(began)return {kind:'no-more-cases'};began=true;return {kind:'case',context:1,id:'connected',form:'scenario',directive:production,mocks:['First']};}
  if(request.kind==='collaborator'){actual.push(request.invocation.subject);return {kind:'collaborator-result',context:1,observation:{kind:'returned',value:{kind:'unit'}}};}
  if(request.kind==='observe')return ++step===1?{kind:'action',context:1,directive:production,mocks:['Second']}:{kind:'action',context:1,directive:verdict,mocks:[]};
  if(request.kind==='close-case')return {kind:'case-closed',context:1,result:{status:'PASS'}};
  return {kind:'test-closed'};
 }} as import('./client.ts').AssessmentClient;
 await scenarioTest(client,'connected',context=>{retained=context;return {subject:invocation.subject,async invokeProduction(){await context.collaborate({subject:context.mocks[0]!,arguments:[]});return evidence;},cleanup(){}};});
 expect(actual).toEqual(['First','Second']);expect(retained!.mocks).toEqual([]);
});
test('identity helper accepts only central derivation without invoking a native fixture',async()=>{
 const {identityTest}=await import('./operation.ts');const {client,requests}=fixtureClient([verdict],'entity-identity');
 await identityTest(client,'identity');expect(requests.filter(request=>request.kind==='observe')).toHaveLength(0);
});
test('lifecycle allows setup after-snapshots and nested transition snapshots for the same actual call',async()=>{
 const {lifecycleTest}=await import('./operation.ts');
 const {client}=fixtureClient([before,{...production,phase:'setup'},after,before,before,production,after,after,verdict],'lifecycle');const order:string[]=[];
 await lifecycleTest(client,'nested',()=>({subject:invocation.subject,setup(){order.push('setup');return evidence;},invokeProduction(){order.push('production');return evidence;},readState(entities,phase){order.push(phase);return {entities:{Film:[]},complete:true};},cleanup(){order.push('cleanup');}}));
 expect(order).toEqual(['before','setup','after','before','before','production','after','after','cleanup']);
});
