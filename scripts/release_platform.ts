/** Real, bounded distribution checks on the runner OS; this is not a managed-executor port. */
import {spawn, type ChildProcess} from 'node:child_process';
import {mkdtempSync, mkdirSync, readFileSync, writeFileSync, copyFileSync, cpSync, rmSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {tmpdir} from 'node:os';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';

const binary = resolve(process.argv[2] ?? 'target/release/dctx0');
const root = mkdtempSync(join(tmpdir(), 'dctx0-platform-'));
const checks: {name: string; elapsed_ms: number; status: string}[] = [];
let failure: string|undefined;
const owned = new Set<ChildProcess>();
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
async function timed<T>(name: string, action: () => Promise<T>): Promise<T> {
  const started = performance.now();
  let status = 'FAIL';
  try {const result = await action(); status = 'PASS'; return result;}
  finally {checks.push({name, elapsed_ms: performance.now() - started, status});}
}
function start(program: string, args: string[], cwd = root) {
  const child = spawn(program, args, {cwd, detached: process.platform !== 'win32', stdio: ['pipe', 'pipe', 'pipe']});
  owned.add(child);
  let stderr = '';
  child.stderr!.on('data', value => { stderr += value; if (stderr.length > 1_048_576) void stop(child); });
  const closed = new Promise<number|null>((resolve, reject) => {
    child.once('error', reject);
    child.once('close', code => {owned.delete(child); resolve(code);});
  });
  // Prevent a rejected spawn promise becoming unhandled before callers attach a handler.
  void closed.catch(() => {});
  return {child, closed, logs: () => stderr};
}
async function bounded<T>(promise: Promise<T>, ms = 15000): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  try { return await Promise.race([promise, new Promise<T>((_, reject) => {timer = setTimeout(() => reject(new Error('Platform check timed out')), ms);})]); }
  finally { clearTimeout(timer!); }
}
async function stop(child: ChildProcess) {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return;
  if (process.platform === 'win32') {
    const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {stdio: 'ignore'});
    await bounded(new Promise<void>((resolve, reject) => {killer.once('error', reject); killer.once('close', () => resolve());}), 10000);
  } else {
    try { process.kill(-child.pid, 'SIGKILL'); } catch (error: any) { if (error.code !== 'ESRCH') throw error; }
  }
}
async function command(program: string, args: string[], cwd = root) {
  const p = start(program, args, cwd);
  let stdout = '';
  p.child.stdout!.on('data', value => {stdout += value; if (stdout.length > 1_048_576) void stop(p.child);});
  p.child.stdin!.end();
  try {return {code: await bounded(p.closed, 60000), stdout, stderr: p.logs()};}
  finally {await stop(p.child); await bounded(p.closed);}
}
function messages(child: ChildProcess, framed = false) {
  let buffer = Buffer.alloc(0);
  const queue: any[] = [];
  let waiting: {resolve: (value: any) => void; reject: (error: any) => void}|undefined;
  let error: any;
  child.stdout!.on('data', data => {
    try {
    buffer = Buffer.concat([buffer, data]);
    assert(buffer.length <= 1_048_576, 'Protocol buffer limit');
    while (true) {
      let body: Buffer;
      if (framed) {
        const boundary = buffer.indexOf('\r\n\r\n');
        if (boundary < 0) return;
        const length = Number(buffer.subarray(0, boundary).toString().match(/Content-Length:\s*(\d+)/i)?.[1]);
        assert(Number.isSafeInteger(length) && length >= 0 && length <= 1_048_576);
        if (buffer.length < boundary + 4 + length) return;
        body = buffer.subarray(boundary + 4, boundary + 4 + length);
        buffer = buffer.subarray(boundary + 4 + length);
      } else {
        const newline = buffer.indexOf('\n');
        if (newline < 0) return;
        body = buffer.subarray(0, newline); buffer = buffer.subarray(newline + 1);
      }
      const value = JSON.parse(body.toString());
      if (waiting) {const take = waiting; waiting = undefined; take.resolve(value);} else queue.push(value);
    }
    } catch (caught) {error = caught; waiting?.reject(caught); waiting = undefined;}
  });
  child.once('close', () => {error ??= new Error('Protocol process closed'); waiting?.reject(error); waiting = undefined;});
  return {
    send(value: any) {const body = JSON.stringify(value); child.stdin!.write(framed ? `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}` : body + '\n');},
    async next() {if (queue.length) return queue.shift(); if(error)throw error; return bounded(new Promise((resolve,reject) => {waiting = {resolve,reject};}));}
  };
}

try {
  await timed('standalone CLI version, skills and compilation', async () => {
    const version = await command(binary, ['--version']);
    assert.equal(version.code, 0, version.stderr); assert.match(version.stdout, /^dctx0 \d+\.\d+\.\d+/);
    const skills = await command(binary, ['skills', 'list', '--json']);
    assert.equal(skills.code, 0, skills.stderr); assert.match(skills.stdout, /dctx0-language/);
    const specs = join(root, 'spec'); mkdirSync(specs);
    writeFileSync(join(specs, 'a.dctx0'), 'workspace Platform { domain D { value Year = Int { range 1888..2100 } } }');
    const check = await command(binary, ['check', specs, '--json']);
    assert.equal(check.code, 0, check.stderr); assert.equal(JSON.parse(check.stdout).ok, true);
  });
  await timed('LSP initialize, diagnostics, symbols and clean shutdown', async () => {
    const specs = join(root, 'lsp'); mkdirSync(specs);
    const file = join(specs, 'a.dctx0');
    writeFileSync(file, 'workspace Platform { type Year = Int }');
    const p = start(binary, ['lsp', '--stdio', '--project', specs]);
    const client = messages(p.child, true);
    client.send({jsonrpc:'2.0',id:1,method:'initialize',params:{capabilities:{}}});
    assert.equal((await client.next()).result.capabilities.positionEncoding, 'utf-16');
    client.send({jsonrpc:'2.0',method:'initialized',params:{}});
    const uri = pathToFileURL(file).href;
    client.send({jsonrpc:'2.0',method:'textDocument/didOpen',params:{textDocument:{uri,languageId:'dctx0',version:1,text:'workspace Platform { type Year = Missing }'}}});
    let diagnostic: any;
    do {diagnostic = await client.next();} while (diagnostic.method !== 'textDocument/publishDiagnostics');
    assert(diagnostic.params.diagnostics.length > 0);
    client.send({jsonrpc:'2.0',id:2,method:'textDocument/documentSymbol',params:{textDocument:{uri}}});
    let response: any; do {response = await client.next();} while (response.id !== 2);
    assert(response.result.length > 0);
    client.send({jsonrpc:'2.0',id:3,method:'shutdown',params:null});
    do {response = await client.next();} while (response.id !== 3);
    client.send({jsonrpc:'2.0',method:'exit'});
    assert.equal(await bounded(p.closed), 0, p.logs());
  });
  await timed('actual Bun SDK harness call, state, event and mock transport', async () => {
    const directory = join(root, 'harness'); mkdirSync(directory);
    const source = resolve('targets/typescript/harness');
    for (const file of ['contract.ts','module-observer.ts']) copyFileSync(join(source,file),join(directory,file));
    writeFileSync(join(directory,'worker.ts'),readFileSync(join(source,'worker.ts'),'utf8').replace('__ADAPTER_IMPORT__',JSON.stringify('./adapter.ts')).replace('__SUBJECTS__',JSON.stringify(['operation','value','Entity'])));
    copyFileSync(resolve('targets/typescript/tests/fixtures/counter.ts'),join(directory,'counter.ts'));
    writeFileSync(join(directory,'adapter.ts'), `
import {Counter} from './counter.ts';
export const adapter={describe(){return {protocol_version:1,subjects:[],artifacts:['counter.ts'],capabilities:['sessions','invocation','state','events','mock-injection'],representations:{}}},begin(id,context){
const integer=v=>({kind:'integer',value:String(v)});
const counter=new Counter(async()=>{const r=await context.collaborator({subject:'port',arguments:[integer(counter.value)]});return Number(r.value.value)},v=>context.emit('Changed',integer(v)));
return {async invoke(call){const v=call.arguments[0];return {kind:'returned',value:integer(v.kind==='string'?await counter.refresh():counter.increment(Number(v.value)))}},observe(){return {Entity:[{kind:'record',value:{count:integer(counter.value)}}]}},close(){},validate(subject,value){return {kind:'accepted',value}}}}};`);
    const p = start(process.execPath, ['run','worker.ts'], directory);
    const client = messages(p.child); let id = 0;
    const ask = async (request: any) => {const expected=++id; client.send({protocol_version:1,id:expected,request}); const packet=await client.next(); assert.equal(packet.message.id,expected); return packet.message.reply;};
    assert.equal((await ask({kind:'describe'})).description.protocol_version,1);
    assert.equal((await ask({kind:'begin',case_id:'case'})).kind,'begun');
    const reply=await ask({kind:'invoke',invocation:{subject:'operation',arguments:[{kind:'integer',value:'2'}]}});
    assert.equal(reply.observation.value.value,'2'); assert.equal(reply.events[0].subject,'Changed');
    const observed=await ask({kind:'observe',entities:['Entity']}); assert.equal(observed.states.Entity[0].value.count.value,'2');
    const invocationId=++id; client.send({protocol_version:1,id:invocationId,request:{kind:'invoke',invocation:{subject:'operation',arguments:[{kind:'string',value:'mock'}]}}});
    const mock=(await client.next()).message; assert.equal(mock.kind,'mock-call');
    client.send({protocol_version:1,id:++id,request:{kind:'mock-reply',request_id:mock.request_id,observation:{kind:'returned',value:{kind:'integer',value:'7'}}}});
    const response=(await client.next()).message; assert.equal(response.id,invocationId); assert.equal(response.reply.observation.value.value,'7');
    assert.equal((await ask({kind:'shutdown'})).kind,'bye'); assert.equal(await bounded(p.closed),0,p.logs());
  });
  await timed('forced cancellation cleans up the owned process and descendant', async () => {
    const pidFile=join(root,'descendant.pid');
    const descendant=join(root,'descendant.ts');
    writeFileSync(descendant,`import {writeFileSync} from 'node:fs';writeFileSync(${JSON.stringify(pidFile)},String(process.pid));setInterval(()=>{},1000);`);
    const parent=join(root,'parent.ts');
    writeFileSync(parent,`import {spawn} from 'node:child_process';spawn(process.execPath,[${JSON.stringify(descendant)}],{stdio:'ignore'});setInterval(()=>{},1000);`);
    const p=start(process.execPath,[parent]);
    let pid: number|undefined;
    for(let i=0;i<100;i++){try{pid=Number(readFileSync(pidFile,'utf8'));if(pid)break;}catch{}await sleep(50);}
    assert(pid,'Descendant started');
    await stop(p.child); await bounded(p.closed);
    let live=true;
    for(let i=0;i<100;i++){try{process.kill(pid!,0);}catch{live=false;break;}await sleep(50);}
    if(live){try{process.kill(pid!,'SIGKILL');}catch{}}
    assert(!live,'Descendant survived cancellation');
  });
  if (process.platform !== 'linux') await timed('managed executor rejects unsupported platform without PASS or adoption', async () => {
    const app=join(root,'managed-boundary');
    cpSync(resolve('fixtures/conformance/adaptive-typescript/direct'),app,{recursive:true});
    const specs=join(app,'spec');mkdirSync(specs);
    copyFileSync(resolve('fixtures/conformance/adaptive-typescript/spec/workspace.dctx0'),join(specs,'workspace.dctx0'));
    const checked=await command(binary,['check',specs,'--json']);assert.equal(checked.code,0,checked.stderr);
    const initialized=await command(binary,['verify','init',specs,'--project',app,'--target','typescript','--subject','Adaptive.Counter.next','--json']);
    assert.equal(initialized.code,0,initialized.stderr);
    const attempted=await command(binary,['verify','run',specs,'--project',app,'--no-adopt','--json']);
    assert.notEqual(attempted.code,0,'Unsupported execution must fail acceptance');
    const report=JSON.parse(attempted.stdout);
    assert.notEqual(report.evidence.run.result.status,'PASS');
    assert.equal(report.adopted_tasks.length,0);
    assert.match(attempted.stdout,/Linux|Unsupported|unsupported/);
  });
} catch(error) {
  failure=String(error); throw error;
} finally {
  for(const child of owned){await stop(child);}
  for(let i=0;i<100 && owned.size;i++)await sleep(50);
  const report={schema_version:1,status:failure ? 'FAIL' : 'PASS',failure,platform:process.platform,architecture:process.arch,
    binary_sha256:createHash('sha256').update(readFileSync(binary)).digest('hex'),bun:Bun.version,checks,
    scope:'Standalone CLI/LSP and testing-only Bun SDK transport with owned test-process cancellation; no managed verification/adoption claim on macOS or Windows',
    memory:{peak_rss_bytes:process.platform==='linux' ? (process.resourceUsage().maxRSS*1024 || null) : null,
      scope:'Linux Bun runner self peak RSS; other OS self metrics unavailable here; the Python distribution runner measures child peak memory separately'}};
  mkdirSync(resolve('target'),{recursive:true});
  const reportPath=resolve('target/release-platform.json');writeFileSync(reportPath,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
  assert.equal(owned.size,0,'Owned processes must be reaped');
  rmSync(root,{recursive:true,force:true});
}
