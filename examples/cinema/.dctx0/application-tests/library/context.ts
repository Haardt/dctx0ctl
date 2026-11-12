import {AssessmentClient} from './client.ts';
import {BindingError,type Invocation,type Observation,type Reply} from './protocol.ts';
export type CaseContext={
 id:string;context:number;initialSubject:string|null;mocks:string[];
 collaborate:(invocation:Invocation)=>Promise<Observation>;
};
export function updateMocks(context:CaseContext,reply:Reply):void {
 if(reply.kind!=='case'&&reply.kind!=='action')return;
 const mocks=reply.mocks??[];
 if(!Array.isArray(mocks)||mocks.some(subject=>typeof subject!=='string')||new Set(mocks).size!==mocks.length)throw new BindingError('Invalid central mock capabilities');
 context.mocks.splice(0,context.mocks.length,...mocks);
}
export function createCaseContext(client:AssessmentClient,reply:Extract<Reply,{kind:'case'}>):CaseContext{
 const directive=reply.directive;
 const context:CaseContext={id:reply.id,context:reply.context,initialSubject:directive.kind==='invoke'||directive.kind==='read-state'?directive.invocation.subject:null,mocks:[],
  async collaborate(invocation){
   const response=await client.exchange({kind:'collaborator',context:reply.context,invocation});
   if(response.kind!=='collaborator-result'||response.context!==reply.context)throw new BindingError('Expected central collaborator observation');
   return response.observation;
  },
 };
 updateMocks(context,reply);return context;
}
