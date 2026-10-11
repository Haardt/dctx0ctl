import {candidateOrderWinner} from './src/domain/candidate-order-winner.ts';
import {filmId} from './src/domain/film-id.ts';
import {Counter,decodeIncrement} from './counter.ts';
import type {ProjectAdapter,Value} from './contract.ts';
import {invokeApplication} from './contract.ts';
const number=(v:Value)=>{if(v.kind!=='integer'||!Number.isSafeInteger(Number(v.value)))throw new Error('Unrepresentable integer');return Number(v.value);};
const integer=(value:number):Value=>({kind:'integer',value:String(value)});
export const adapter:ProjectAdapter={
 async describe(){return {protocol_version:1,subjects:[{subject:'operation',role:'operation',artifact:'counter.ts',entry_point:'Counter.increment'},{subject:'HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner',role:'operation',artifact:'src/domain/candidate-order-winner.ts',entry_point:'candidateOrderWinner'}],artifacts:['counter.ts','src/domain/candidate-order-winner.ts','src/domain/film-id.ts','src/domain/voting-rules.ts'],capabilities:['sessions','invocation','state','events','mock-injection','resource-observation'],representations:{Int:'safe JavaScript integers'}};},
 async begin(caseId,context){
  if(caseId==='init-error')throw new Error('Initialization unavailable');
  const counter=new Counter(async()=>{const response=await context.collaborator({subject:'port',arguments:[integer(counter.value)]});if(response.kind!=='returned')throw new Error(response.kind==='threw'?response.message:response.reason);return number(response.value);},value=>context.emit('CounterChanged',integer(value)));
  return {
   async invoke(invocation){
    console.log('actual application output');
    const command=invocation.arguments[0];
    if(command?.kind==='string'&&command.value==='exception')return invokeApplication(()=>counter.crash(),integer);
    if(command?.kind==='string'&&command.value==='adapter-error')throw new Error('Invalid adapter translation');
    if(command?.kind==='string'&&command.value==='caught-observation-limit'){
     try{for(let index=0;index<10001;index++)context.recordResource({resource:'Counter',access:'read'});}catch{}
     return {kind:'returned',value:integer(counter.value)};
    }
    if(command?.kind==='string'&&command.value==='observation-limit'){
     for(let index=0;index<10001;index++)context.recordResource({resource:'Counter',access:'read'});
     return {kind:'returned',value:integer(counter.value)};
    }
    if(command?.kind==='string'&&command.value==='mutable-observation'){
     const value=counter.increment(1);
     const observation={resource:'Counter',access:'write' as const};
     context.recordResource(observation);
     observation.resource='mutated-after-recording';
     const payload=integer(value);context.emit('Snapshot',payload);
     if(payload.kind==='integer')payload.value='999';
     return {kind:'returned',value:integer(value)};
    }

    if(invocation.subject==='HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner'){
     const films=(value:Value)=>{if(value.kind!=='list')throw new Error('Expected list');return value.value.map(v=>filmId(v.kind==='string'?v.value:v));};
     const votes=films(invocation.arguments[0]),candidates=films(invocation.arguments[1]);
     return invokeApplication(()=>candidateOrderWinner(votes,candidates),winner=>({kind:'option',value:winner?{kind:'string',value:winner}:null}));
    }
    if(command?.kind==='string'&&command.value==='mock'){
     return invokeApplication(()=>counter.refresh(),integer);
    }else {const amount=number(command);return invokeApplication(()=>counter.increment(amount),integer);}
   },
   async validate(_subject,value){try{return {kind:'accepted',value:integer(decodeIncrement(value.kind==='integer'?Number(value.value):value))};}catch(error){return {kind:'rejected',reason:String(error)};}},
   async observe(entities){return Object.fromEntries(entities.map(e=>[e,[{kind:'record',value:{count:integer(counter.value)}}]]));},
   async close(){if(caseId==='cleanup-error')throw new Error('Cleanup unavailable');},
  };
 },
};
