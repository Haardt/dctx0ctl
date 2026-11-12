import {test,expect} from 'bun:test';
import {createServer,type Server} from 'node:net';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {AssessmentClient,BindingError,decoderTest,type Reply,type Request} from './index.ts';
async function channel(handle:(request:Request,id:number)=>Reply|object|null,run:(client:AssessmentClient)=>Promise<void>,options={timeoutMs:1000,maxMessageBytes:65536}){
 const dir=await mkdtemp(join(tmpdir(),'dctx0-bun-'));
 const path=join(dir,'socket');const sockets=new Set<import('node:net').Socket>();
 const server:Server=createServer(socket=>{
  sockets.add(socket);socket.on('close',()=>sockets.delete(socket));let buffer='';
  socket.on('data',bytes=>{
   buffer+=bytes.toString();let at:number;
   while((at=buffer.indexOf('\n'))>=0){
    const frame=JSON.parse(buffer.slice(0,at));buffer=buffer.slice(at+1);
    const reply=handle(frame.payload,frame.id);
    if(reply!==null)socket.write(JSON.stringify('protocol_version' in reply?reply:{protocol_version:2,id:frame.id,payload:reply})+'\n');
   }
  });
 });
 await new Promise<void>(resolve=>server.listen(path,resolve));
 try{const client=await AssessmentClient.connect(path,options);try{await run(client);}finally{client.close();}}
 finally{for(const socket of sockets)socket.destroy();await new Promise<void>(resolve=>server.close(()=>resolve()));await rm(dir,{recursive:true,force:true});}
}
const pass={status:'PASS' as const};
test('explicit context transports invalid bytes unchanged and closes every case',async()=>{
 const seen:Request[]=[];let iteration=0;
 await channel(request=>{
  seen.push(request);
  switch(request.kind){
   case 'collaborator':throw new Error('Unexpected collaborator request in decoder fixture');
   case 'register':return {kind:'registered'};
   case 'next':return iteration++===0?{kind:'case',context:7,id:'central:raw',form:'decoder',directive:{kind:'decode',subject:'Raw',ty:{name:'String',symbol_id:null,arguments:[]},raw:[255,0,123]}}:{kind:'no-more-cases'};
   case 'observe':return {kind:'action',context:7,directive:{kind:'verdict',result:pass}};
   case 'close-case':return {kind:'case-closed',context:7,result:pass};
   case 'close-test':return {kind:'test-closed'};
   case 'finish':return {kind:'finished',summary:{assessment_version:'dctx0-application-assessment-2',group:'raw',finalized:true,status:'PASS',required_cases:1,completed_cases:1,missing_cases:0,missing_tests:0,issues:0,issue_code:null}};
  }
 },async client=>{
  await decoderTest(client,'raw',raw=>{expect([...raw]).toEqual([255,0,123]);raw.fill(0);return {kind:'rejected',reason:'invalid UTF8'};},()=>{throw new Error('rejected values are never projected');});
  await client.finish();
 });
 const observation=seen.find(r=>r.kind==='observe');
 expect(observation).toEqual({kind:'observe',context:7,observation:{kind:'decoded',raw:[255,0,123],outcome:{kind:'rejected',reason:'invalid UTF8'}}});
 expect(seen.map(r=>r.kind)).toEqual(['register','next','observe','close-case','next','close-test','finish']);
});
test.each(['codec','projection','exception'] as const)('%s remains distinct from production rejection',async mode=>{
 let iteration=0;let observed:Request|undefined;
 await channel(request=>{
  if(request.kind==='register')return {kind:'registered'};
  if(request.kind==='next')return iteration++===0?{kind:'case',context:1,id:'case',form:'decoder',directive:{kind:'decode',subject:'Int',ty:{name:'Int',symbol_id:null,arguments:[]},raw:[49]}}:{kind:'no-more-cases'};
  if(request.kind==='observe'){observed=request;return {kind:'action',context:1,directive:{kind:'verdict',result:{status:'UNKNOWN',issue:{code:'BINDING',message:'incomplete'}}}};}
  if(request.kind==='close-case')return {kind:'case-closed',context:1,result:{status:'UNKNOWN'}};
  return {kind:'test-closed'};
 },async client=>{
  await expect(decoderTest(client,'case',()=>{if(mode==='codec')throw new BindingError('parse');if(mode==='exception')throw new Error('crash');return {kind:'accepted',value:1};},()=>{throw new Error('projection');})).rejects.toThrow('UNKNOWN');
 });
 expect(observed?.kind==='observe'&&observed.observation.kind==='decoded'&&observed.observation.outcome.kind).toBe(mode==='exception'?'exception':'binding-error');
});
test('frames reject wrong correlation, version and unknown top-level fields',async()=>{
 for(const frame of [{protocol_version:2,id:2,payload:{kind:'registered'}},{protocol_version:1,id:1,payload:{kind:'registered'}},{protocol_version:2,id:1,payload:{kind:'registered'},extra:true}]){
  await channel(()=>frame,async client=>{await expect(client.exchange({kind:'register',test:'x'})).rejects.toThrow('Invalid assessment frame');});
 }
});
test('bounded channels fail on oversized replies and timeouts',async()=>{
 await channel(()=>({kind:'rejected',issue:{code:'X',message:'x'.repeat(1000)}}),async client=>{await expect(client.exchange({kind:'register',test:'x'})).rejects.toThrow('byte limit');},{timeoutMs:1000,maxMessageBytes:100});
 await channel(()=>null,async client=>{await expect(client.exchange({kind:'finish'})).rejects.toThrow('timed out');},{timeoutMs:20,maxMessageBytes:1024});
});
test('concurrent explicit handles serialize requests and preserve exact integer text',async()=>{
 const requests:Request[]=[];
 await channel(request=>{requests.push(request);return {kind:'registered'};},async client=>{
  await Promise.all(['a','b'].map(test=>client.exchange({kind:'register',test})));
  await client.exchange({kind:'observe',context:1,observation:{kind:'decoded',raw:[49],outcome:{kind:'accepted',value:{kind:'integer',value:'9007199254740993'}}}});
 });
 expect(requests[2]?.kind==='observe'&&requests[2].observation.kind==='decoded'&&requests[2].observation.outcome).toEqual({kind:'accepted',value:{kind:'integer',value:'9007199254740993'}});
});
test('missing cases or post-verdict issues cannot finalize PASS',async()=>{
 for(const change of [{missing_cases:1},{issues:1},{finalized:false},{status:'UNKNOWN'},{completed_cases:0}])await channel(()=>({kind:'finished',summary:{assessment_version:'dctx0-application-assessment-2',group:'x',finalized:true,status:'PASS',required_cases:1,completed_cases:1,missing_cases:0,missing_tests:0,issues:0,issue_code:null,...change}}),async client=>{await expect(client.finish()).rejects.toThrow('incomplete');});
});
test('constructor input codec cannot impersonate a production rejection',async()=>{
 const {constructorTest}=await import('./testing.ts');let iteration=0;let actual:Request|undefined;let called=false;
 await channel(request=>{
  if(request.kind==='register')return {kind:'registered'};
  if(request.kind==='next')return iteration++===0?{kind:'case',context:1,id:'constructor:case',form:'decoder',directive:{kind:'construct',subject:'Int',ty:{name:'Int',symbol_id:null,arguments:[]},value:{kind:'integer',value:'9007199254740993'}}}:{kind:'no-more-cases'};
  if(request.kind==='observe'){actual=request;return {kind:'action',context:1,directive:{kind:'verdict',result:{status:'UNKNOWN'}}};}
  if(request.kind==='close-case')return {kind:'case-closed',context:1,result:{status:'UNKNOWN'}};
  return {kind:'test-closed'};
 },async client=>{
  await expect(constructorTest(client,'constructor',()=>{throw new Error('test codec failed');},()=>{called=true;return {kind:'rejected',reason:'wrong'};},()=>({kind:'unit'}),()=>({kind:'unit'}))).rejects.toThrow('UNKNOWN');
 });
 expect(called).toBe(false);
 expect(actual).toEqual({kind:'observe',context:1,observation:{kind:'binding-error',message:'test codec failed'}});
});
test('constructor observations report actual witnessed inputs after codec edits',async()=>{
 const {constructorTest}=await import('./testing.ts');let iteration=0;let actual:Request|undefined;
 await channel(request=>{
  if(request.kind==='register')return {kind:'registered'};
  if(request.kind==='next')return iteration++===0?{kind:'case',context:4,id:'constructor:case',form:'decoder',directive:{kind:'construct',subject:'Record',ty:{name:'Record',symbol_id:null,arguments:[]},value:{kind:'record',value:{field:{kind:'string',value:'original'}}}}}:{kind:'no-more-cases'};
  if(request.kind==='observe'){actual=request;return {kind:'action',context:4,directive:{kind:'verdict',result:pass}};}
  if(request.kind==='close-case')return {kind:'case-closed',context:4,result:pass};
  return {kind:'test-closed'};
 },async client=>{
  await constructorTest(client,'constructor',value=>{if(value.kind==='record')value.value={};return 1;},()=>({kind:'accepted',value:9007199254740993n}),value=>({kind:'integer',value:String(value)}),native=>({kind:'integer',value:String(native)}));
 });
 expect(actual?.kind==='observe'&&actual.observation).toEqual({kind:'constructed',input:{kind:'integer',value:'1'},outcome:{kind:'accepted',value:{kind:'integer',value:'9007199254740993'}}});
});
test('explicit cancellation rejects outstanding work and unavailable channels fail',async()=>{
 await channel(()=>null,async client=>{
  const pending=client.exchange({kind:'finish'});
  // Let exchange install its pending request, then cancel the explicit handle.
  await new Promise(resolve=>setTimeout(resolve,0));client.close();
  await expect(pending).rejects.toThrow('closed');
 });
 await expect(AssessmentClient.connect('/no-such-dctx0-application-socket')).rejects.toThrow();
 await expect(AssessmentClient.connect('/unused',{maxMessageBytes:0})).rejects.toThrow('limits');
});
test('input witness failures prevent constructor execution and remain binding errors',async()=>{
 const {constructorTest}=await import('./testing.ts');let iteration=0,called=false;let observed:Request|undefined;
 await channel(request=>{
  if(request.kind==='register')return {kind:'registered'};
  if(request.kind==='next')return iteration++===0?{kind:'case',context:1,id:'witness:case',form:'decoder',directive:{kind:'construct',subject:'Int',ty:{name:'Int',symbol_id:null,arguments:[]},value:{kind:'integer',value:'7'}}}:{kind:'no-more-cases'};
  if(request.kind==='observe'){observed=request;return {kind:'action',context:1,directive:{kind:'verdict',result:{status:'UNKNOWN'}}};}
  if(request.kind==='close-case')return {kind:'case-closed',context:1,result:{status:'UNKNOWN'}};
  return {kind:'test-closed'};
 },async client=>{
  await expect(constructorTest(client,'witness',()=>new Date(),()=>{called=true;return {kind:'rejected',reason:'wrong'};},()=>({kind:'unit'}),()=>{throw new BindingError('class needs witness');})).rejects.toThrow('UNKNOWN');
 });
 expect(called).toBe(false);
 expect(observed).toEqual({kind:'observe',context:1,observation:{kind:'binding-error',message:'class needs witness'}});
});
