import {expect,test} from 'bun:test';
import {mkdtemp,readFile,readdir,rm,stat} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const root=resolve(import.meta.dir,'../..');
const script=join(import.meta.dir,'agent-step.sh');
async function exercise(step:string, extra:string[]=[]){
  const base=await mkdtemp(join(tmpdir(),'dctx0-agent-script-test-'));
  const process=Bun.spawn(['bash',script,step,...extra],{cwd:root,env:{...Bun.env,TMPDIR:base},stdout:'pipe',stderr:'pipe'});
  const output=await new Response(process.stdout).text(),error=await new Response(process.stderr).text(),code=await process.exited;
  return {base,output,error,code,path:output.match(/^Exercise: (.+)$/m)?.[1]};
}
test('first exercise supplies requirements and tests without solution or inherited approval',async()=>{
  const run=await exercise('1');
  try {
    expect(run.code).toBe(0);const directory=run.path!;
    expect(await readdir(join(directory,'src'))).toEqual(['board.test.ts']);
    expect(await readFile(join(directory,'dctx0/workspace.dctx0'),'utf8')).toBe(await readFile(join(root,'examples/tutorial-step-1-values/dctx0/workspace.dctx0'),'utf8'));
    expect(await stat(join(directory,'.dctx0ctl')).catch(()=>null)).toBeNull();
    expect(await stat(join(directory,'.dctx0/testing/typescript/inspection-notes.json')).catch(()=>null)).toBeNull();
    expect(await readFile(join(directory,'AGENTS.md'),'utf8')).toContain('Do not copy answers');
  } finally {await rm(run.base,{recursive:true,force:true});}
});
test('later exercise starts from previous production and current independent tests',async()=>{
  const reference=join(root,'examples/tutorial-step-4-expressions/src/expressions.ts');
  const before=await readFile(reference,'utf8');const run=await exercise('4');
  try {
    expect(run.code).toBe(0);const directory=run.path!;
    expect(await stat(join(directory,'src/expressions.ts')).catch(()=>null)).toBeNull();
    expect(await readFile(join(directory,'src/expressions.test.ts'),'utf8')).toBe(await readFile(join(root,'examples/tutorial-step-4-expressions/src/expressions.test.ts'),'utf8'));
    expect(await readFile(join(directory,'src/data.ts'),'utf8')).toBe(await readFile(join(root,'examples/tutorial-step-3-data/src/data.ts'),'utf8'));
    expect(await readFile(reference,'utf8')).toBe(before);
  } finally {await rm(run.base,{recursive:true,force:true});}
});
test('optional agent command runs in the exercise and receives the task as an argument',async()=>{
  const run=await exercise('1',['--','bash','-c','printf "Agent directory: %s\\nAgent task: %s\\n" "$PWD" "$0"']);
  try {
    expect(run.code).toBe(0);expect(run.output).toContain('Agent directory: '+run.path);
    expect(run.output).toContain('Agent task: Implement tutorial step 1');
  } finally {await rm(run.base,{recursive:true,force:true});}
});
test('invalid step is rejected without creating an exercise',async()=>{
  const run=await exercise('0');
  try {expect(run.code).toBe(2);expect(run.path).toBeUndefined();expect(await readdir(run.base)).toEqual([]);}
  finally {await rm(run.base,{recursive:true,force:true});}
});
test('fragment exercise copies every current spec file while retaining previous production only',async()=>{
  const run=await exercise('14');
  try {
    expect(run.code).toBe(0);const directory=run.path!;
    expect((await readdir(join(directory,'dctx0'))).sort()).toEqual(['copy.dctx0','workspace.dctx0']);
    expect(await readFile(join(directory,'dctx0/copy.dctx0'),'utf8')).toBe(await readFile(join(root,'examples/tutorial-step-14-fragments/dctx0/copy.dctx0'),'utf8'));
    expect(await stat(join(directory,'src/copy.ts')).catch(()=>null)).toBeNull();
    expect(await stat(join(directory,'src/copy.test.ts')).then(info=>info.isFile())).toBe(true);
    expect(await stat(join(directory,'.dctx0/testing/typescript/inspection-notes.json')).catch(()=>null)).toBeNull();
  }finally{await rm(run.base,{recursive:true,force:true});}
});
test('HTTP exercise supplies current startup tests without supplying the new server or UI solution',async()=>{
  const run=await exercise('16');
  try {
    expect(run.code).toBe(0);const directory=run.path!;
    expect(await readFile(join(directory,'server.test.ts'),'utf8')).toBe(await readFile(join(root,'examples/tutorial-step-16-http-replay/server.test.ts'),'utf8'));
    expect(await stat(join(directory,'server.ts')).catch(()=>null)).toBeNull();
    expect(await stat(join(directory,'src/ui.ts')).catch(()=>null)).toBeNull();
    expect(await readFile(join(directory,'AGENTS.md'),'utf8')).toContain('new tasks, Done/Open status');
  }finally{await rm(run.base,{recursive:true,force:true});}
});
