import {strict as assert} from 'node:assert';
import {Board} from '../tutorial-step-16-http-replay/src/entities.ts';
import {boardHandler} from '../tutorial-step-16-http-replay/src/http.ts';

// Development acceptance: requires a Chromium debugger, never imported by production.
const endpoint = process.env.BROWSER_DEBUG_URL ?? 'http://127.0.0.1:9228';
const browser = await (await fetch(endpoint + '/json/version')).json() as {webSocketDebuggerUrl:string};
const socket = new WebSocket(browser.webSocketDebuggerUrl);
await new Promise<void>((resolve,reject)=>{socket.onopen=()=>resolve();socket.onerror=()=>reject(new Error('Browser connection failed'));});
let sequence=0;
const pending=new Map<number,{resolve:(value:any)=>void,reject:(error:Error)=>void,timer:ReturnType<typeof setTimeout>}>();
socket.onmessage=event=>{
  const packet=JSON.parse(String(event.data)),request=pending.get(packet.id);
  if(!request)return;
  clearTimeout(request.timer);pending.delete(packet.id);
  if(packet.error)request.reject(new Error(packet.error.message));else request.resolve(packet.result);
};
function send(method:string,params:Record<string,unknown>={},sessionId?:string):Promise<any>{
  const id=++sequence;
  return new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{pending.delete(id);reject(new Error('Browser timeout: '+method));},10000);
    pending.set(id,{resolve,reject,timer});socket.send(JSON.stringify({id,method,params,sessionId}));
  });
}
const id='00000000-0000-0000-0000-000000000001';
const second='00000000-0000-0000-0000-000000000002';
const board=new Board([{id,title:'Spec lesen',status:'Open'},{id:second,title:'Code prüfen',status:'Done'}]);
const handler=boardHandler(board);
let mode='normal';
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch:async request=>{
  const path=new URL(request.url).pathname;
  if(path==='/board/tasks'&&mode==='empty')return Response.json([]);
  if(path==='/board/tasks'&&mode==='list-failure')return new Response('Unavailable',{status:503});
  if(request.method!=='GET'&&mode==='write-failure')return new Response('Unavailable',{status:503});
  if(path.startsWith('/tasks/')&&mode==='detail-failure')return new Response('Missing',{status:404});
  if(path==='/tasks/'+id&&mode==='delayed')await Bun.sleep(200);
  return handler(request);
}});
let targetId:string|undefined;
const checks:string[]=[];
try {
  targetId=(await send('Target.createTarget',{url:'about:blank'})).targetId;
  const sessionId=(await send('Target.attachToTarget',{targetId,flatten:true})).sessionId;
  await send('Page.enable',{},sessionId);
  await send('Debugger.enable',{},sessionId);
  await send('Profiler.enable',{},sessionId);
  await send('Profiler.startPreciseCoverage',{callCount:true,detailed:true},sessionId);
  await send('Emulation.setDeviceMetricsOverride',{width:1100,height:780,deviceScaleFactor:1,mobile:false},sessionId);
  async function evaluate(expression:string){
    const result=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true},sessionId);
    if(result.exceptionDetails)throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  }
  async function until(expression:string){
    await evaluate(`(async()=>{const start=Date.now();while(!(${expression})){if(Date.now()-start>5000)throw new Error('UI condition timed out');await new Promise(resolve=>setTimeout(resolve,20));}})()`);
  }
  await send('Page.navigate',{url:server.url.href},sessionId);
  await until("document.querySelectorAll('.task').length===2");
  assert.equal(await evaluate("document.getElementById('count').textContent"),'2');checks.push('real HTTP list renders two tasks');
  await evaluate("document.querySelector('.task').click()");
  await until("document.getElementById('detail-title').textContent==='Spec lesen'");
  assert.equal(await evaluate("document.getElementById('detail-id').textContent"),id);checks.push('selection fetches actual task details');
  assert.equal(await evaluate("document.querySelectorAll('.task-row input')[1].checked"),true);checks.push('stored Done status is visible');
  await evaluate("document.getElementById('new-title').value='Neue Aufgabe';document.getElementById('new-task').requestSubmit()");
  await until("document.querySelectorAll('.task').length===3");
  assert.equal(board.listTasks()[2]!.title,'Neue Aufgabe');checks.push('new task is inserted into the actual board through HTTP');
  await evaluate("document.querySelector('.task-row input').click()");
  await until("document.querySelector('.task-row').classList.contains('done')");
  assert.equal(board.listTasks()[0]!.status,'Done');checks.push('checkbox changes actual stored status to Done');
  await evaluate("document.querySelector('.task-row input').click()");
  await until("!document.querySelector('.task-row').classList.contains('done')");
  assert.equal(board.listTasks()[0]!.status,'Open');checks.push('checkbox can reopen an actual task');
  mode='write-failure';await evaluate("document.querySelector('.task-row input').click()");
  await until("document.getElementById('status').textContent.includes('Änderung konnte nicht gespeichert')");
  assert.equal(board.listTasks()[0]!.status,'Open');
  assert.equal(await evaluate("document.querySelector('.task-row input').checked"),false);checks.push('failed write preserves stored state and restores checkbox');
  await evaluate("document.getElementById('new-title').value='Fehler';document.getElementById('new-task').requestSubmit()");
  await until("!document.getElementById('create').disabled");
  assert.equal(board.listTasks().length,3);assert.equal(await evaluate("document.getElementById('new-title').value"),'Fehler');checks.push('failed creation retains input for retry');
  await evaluate("document.getElementById('new-title').value='   ';document.getElementById('new-task').requestSubmit()");
  await until("document.getElementById('status').textContent==='Bitte einen Titel eingeben.'");checks.push('blank title does not create a task');
  mode='detail-failure';await evaluate("document.querySelector('.task').click()");
  await until("document.getElementById('hint').textContent==='Die Aufgabe konnte nicht geladen werden.'");checks.push('missing detail shows an error');
  mode='delayed';await evaluate("document.querySelectorAll('.task')[0].click();document.querySelectorAll('.task')[1].click()");
  await until("document.getElementById('detail-title').textContent==='Code prüfen'");
  await Bun.sleep(300);
  assert.equal(await evaluate("document.getElementById('detail-title').textContent"),'Code prüfen');checks.push('late response cannot overwrite a newer selection');
  mode='empty';await evaluate("document.getElementById('refresh').click()");
  await until("document.getElementById('status').textContent==='Noch keine Aufgaben vorhanden.'");
  assert.equal(await evaluate("document.querySelectorAll('.task').length"),0);checks.push('empty board renders without stale tasks');
  mode='list-failure';await evaluate("document.getElementById('refresh').click()");
  await until("document.getElementById('status').textContent.includes('Bitte erneut versuchen.')");
  assert.equal(await evaluate("document.getElementById('refresh').disabled"),false);checks.push('HTTP failure allows retry');
  mode='normal';await evaluate("document.getElementById('refresh').click()");
  await until("document.querySelectorAll('.task').length===3");checks.push('retry restores actual tasks');
  await evaluate("document.getElementById('new-title').value=''");
  await send('Emulation.setDeviceMetricsOverride',{width:375,height:812,deviceScaleFactor:1,mobile:true},sessionId);
  assert.equal(await evaluate('document.documentElement.scrollWidth<=window.innerWidth'),true);checks.push('mobile layout fits the viewport');
  await send('Emulation.setDeviceMetricsOverride',{width:1100,height:780,deviceScaleFactor:1,mobile:false},sessionId);
  const coverage=await send('Profiler.takePreciseCoverage',{},sessionId);
  const scripts=[];
  for(const script of coverage.result.filter((entry:any)=>entry.url===server.url.href)){
    const {scriptSource}=await send('Debugger.getScriptSource',{scriptId:script.scriptId},sessionId);
    const functions=script.functions.filter((entry:any)=>entry.ranges[0].count>0);
    const ranges=script.functions.flatMap((entry:any)=>entry.ranges);
    let offset=0;const executable:number[]=[],uncovered:number[]=[];
    for(const [index,line] of scriptSource.split('\n').entries()){
      const start=offset+line.search(/\S/);offset+=line.length+1;
      if(!line.trim()||/^[{}]+;?$/.test(line.trim()))continue;
      const matching=ranges.filter((range:any)=>range.startOffset<=start&&start<range.endOffset).sort((a:any,b:any)=>(a.endOffset-a.startOffset)-(b.endOffset-b.startOffset));
      if(!matching.length)continue;
      executable.push(index+1);if(!matching[0].count)uncovered.push(index+1);
    }
    scripts.push({source:scriptSource,functions:script.functions.length,executed_functions:functions.length,executable_lines:executable,uncovered_lines:uncovered,raw_ranges:script.functions});
  }
  assert.ok(scripts.length,'No actual page JavaScript coverage was returned');
  for(const script of scripts){assert.equal(script.executed_functions,script.functions);assert.deepEqual(script.uncovered_lines,[],'Uncovered browser source lines');}
  await send('Profiler.stopPreciseCoverage',{},sessionId);
  const screenshot=await send('Page.captureScreenshot',{format:'png'},sessionId);
  const path=process.env.UI_SCREENSHOT ?? '/tmp/dctx0-miniboard.png';
  await Bun.write(path,Buffer.from(screenshot.data,'base64'));
  console.log(JSON.stringify({status:'PASS',browser:await send('Browser.getVersion'),checks,browser_coverage:{measurement:'V8 precise function/block ranges; executable line starts mapped to the innermost range, raw ranges retained; not a complete branch proof',scripts},screenshot:path},null,2));
}finally{
  if(targetId)await send('Target.closeTarget',{targetId});
  socket.close();await server.stop(true);
}
