import {AssessmentClient} from './client.ts';
import {assertPass,BindingError,message,type Directive,type Invocation,type InvocationEvidence,type Reply,type StateEvidence} from './protocol.ts';
import {createCaseContext,updateMocks,type CaseContext} from './context.ts';
export type {CaseContext} from './context.ts';
export type OperationFixture={
 subject:string;
 subjects?:readonly string[];
 invokeProduction:(invocation:Invocation)=>Promise<InvocationEvidence>|InvocationEvidence;
 setup?:(invocation:Invocation)=>Promise<InvocationEvidence>|InvocationEvidence;
 oracle?:(invocation:Invocation)=>Promise<InvocationEvidence>|InvocationEvidence;
 readState?:(entities:string[],phase:'before'|'after',invocation:Invocation)=>Promise<StateEvidence>|StateEvidence;
 cleanup:()=>void|Promise<void>;
};
export type FixtureFactory=(context:CaseContext)=>Promise<OperationFixture>|OperationFixture;
function action(reply:Reply,context:number):Directive {
 if(reply.kind!=='action'||reply.context!==context)throw new Error('Expected central operation continuation');
 return reply.directive;
}
/** Fixtures are case-local. Production is a typed closure for one fixed subject;
 * setup and pure oracle continuations are explicit local capabilities.
 */
async function run(client:AssessmentClient,test:string,form:'operation'|'state-change'|'assertion'|'scenario'|'lifecycle'|'entity-identity',create?:FixtureFactory):Promise<void>{
 const registered=await client.exchange({kind:'register',test});
 if(registered.kind!=='registered')throw new Error('Expected operation registration');
 const failures:string[]=[];
 try{
  while(true){
   const next=await client.exchange({kind:'next',test});
   if(next.kind==='no-more-cases')break;
   if(next.kind!=='case'||next.form!==form||!Number.isSafeInteger(next.context)||next.context<1)throw new Error('Expected central operation case');
   const {context,id}=next;
   const caseContext=createCaseContext(client,next),multi=form==='scenario'||form==='lifecycle';
   let fixture:OperationFixture|undefined,cleanup_error:string|null=null;
   let production=false,after=false;
   let beforeInvocation:string|undefined,stateInvocation:string|undefined;
   try{
    let directive=next.directive;
    if(directive.kind!=='verdict'){if(!create)throw new BindingError('Identity cases must be centrally derived');fixture=await create(caseContext);}
    while(directive.kind!=='verdict'){
     if(!fixture)throw new BindingError('Missing operation fixture');
     if(directive.kind==='read-state'){
      if(form!=='state-change'&&!multi||!fixture.readState)throw new BindingError('Actual state observer unavailable');
      if(directive.phase==='before'?!multi&&production:(!multi&&after||beforeInvocation!==JSON.stringify(directive.invocation)||stateInvocation!==JSON.stringify(directive.invocation)))throw new BindingError('Invalid before/production/after state order');
      const observedInvocation=structuredClone(directive.invocation);
      const state=await fixture.readState([...directive.entities],directive.phase,observedInvocation);
      if(directive.phase==='before'){beforeInvocation=JSON.stringify(directive.invocation);stateInvocation=undefined;if(multi){production=false;after=false;}}else after=true;
      const reply=await client.exchange({kind:'observe',context,observation:{kind:'state',...state,phase:directive.phase,invocation:observedInvocation}});
      updateMocks(caseContext,reply);directive=action(reply,context);
     }else if(directive.kind==='invoke'){
      const phase=directive.phase,invocation=structuredClone(directive.invocation);
      if(form==='assertion'&&phase!=='oracle')throw new BindingError('Assertion cases require pure local oracle calls');
      if(phase==='setup'&&!multi&&production||phase==='production'&&(!multi&&production||form==='state-change'&&beforeInvocation!==JSON.stringify(directive.invocation)))throw new BindingError('Invalid setup/production/oracle order');
      if(phase==='production'&&(fixture.subjects?!fixture.subjects.includes(invocation.subject):invocation.subject!==fixture.subject))throw new BindingError('Production subject differs from typed fixture');
      const invoke=phase==='production'?fixture.invokeProduction:phase==='setup'?fixture.setup:fixture.oracle;
      if(!invoke)throw new BindingError(`Unsupported local ${phase} continuation`);
      let evidence:InvocationEvidence;
      try{evidence=await invoke(invocation);}
      catch(error){if(error instanceof BindingError)throw error;evidence={observation:{kind:'threw',message:message(error)},calls:null,events:null,resources:null};}
      if(phase!=='oracle')stateInvocation=JSON.stringify(invocation);
      if(phase==='production'){production=true;after=false;}
      else if(phase==='setup'&&!multi)beforeInvocation=undefined;
      const reply=await client.exchange({kind:'observe',context,observation:{kind:'invoked',invocation,...evidence}});
      updateMocks(caseContext,reply);directive=action(reply,context);
     }else throw new BindingError('Unsupported operation directive');
    }
    assertPass(directive.result,id);
   }catch(error){
    failures.push(`${id}: ${message(error)}`);
    if(error instanceof BindingError){
     try{await client.exchange({kind:'observe',context,observation:{kind:'binding-error',message:error.message}});}catch(fault){failures.push(message(fault));}
    }
   }finally{
    try{await fixture?.cleanup();}catch(error){cleanup_error=message(error);failures.push(`${id}: cleanup: ${cleanup_error}`);}
    const closed=await client.exchange({kind:'close-case',context,cleanup_error});
    if(closed.kind!=='case-closed'||closed.context!==context)throw new Error('Expected operation case closure');
    try{assertPass(closed.result,id);}catch(error){failures.push(message(error));}
   }
  }
 }finally{
  const closed=await client.exchange({kind:'close-test',test});
  if(closed.kind!=='test-closed')throw new Error('Expected operation test closure');
 }
 if(failures.length)throw new Error(failures.join('\n'));
}
export function operationTest(client:AssessmentClient,test:string,create:FixtureFactory):Promise<void>{return run(client,test,'operation',create);}
export function stateChangeTest(client:AssessmentClient,test:string,create:FixtureFactory):Promise<void>{return run(client,test,'state-change',create);}

export function assertionTest(client:AssessmentClient,test:string,create:FixtureFactory):Promise<void>{return run(client,test,'assertion',create);}

export function scenarioTest(client:AssessmentClient,test:string,create:FixtureFactory):Promise<void>{return run(client,test,'scenario',create);}
export function lifecycleTest(client:AssessmentClient,test:string,create:FixtureFactory):Promise<void>{return run(client,test,'lifecycle',create);}

/** Identity is derived centrally from completed source cases; no native fixture. */
export function identityTest(client:AssessmentClient,test:string):Promise<void>{return run(client,test,'entity-identity');}
