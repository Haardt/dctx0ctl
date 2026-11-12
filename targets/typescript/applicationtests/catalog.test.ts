import {test,expect} from 'bun:test';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const collector=new URL('./catalog.ts',import.meta.url).pathname;
async function collect(source:string,argument='root.test.ts'){
 const root=await mkdtemp(join(tmpdir(),'bun-catalog-'));
 try{
  await writeFile(join(root,'root.test.ts'),"import {test,describe,beforeAll} from 'bun:test';\n"+source);
  const process=Bun.spawn(['bun',collector,argument],{cwd:root,stdout:'pipe',stderr:'pipe'});
  const text=await new Response(process.stdout).text(),error=await new Response(process.stderr).text(),code=await process.exited;
  if(!text.trim())throw new Error('Collector omitted its structured report: '+error);
  return {code,report:JSON.parse(text)};
 }finally{await rm(root,{recursive:true,force:true});}
}
test('standalone catalog collects actual native names without executing test callbacks',async()=>{
 const {code,report}=await collect("test('CinemaSuite',()=>{throw new Error('must never execute');});");
 expect(code).toBe(0);expect(report.complete).toBe(true);
 expect(report.tests).toEqual([{entrypoint:'root.test.ts',name:'CinemaSuite',skipped:false,focused:false}]);
});
test.each([
 "test.skip('Skipped',()=>{});",
 "test.only('Focused',()=>{});",
 "test('Duplicate',()=>{});test('Duplicate',()=>{});",
 "describe('Nested',()=>test('Hidden',()=>{}));",
 "beforeAll(()=>{});test('Hooked',()=>{});",
 "test.each([1])('Parameterized',()=>{});",
 "test.concurrent('Concurrent',()=>{});",
 "",
])('incomplete or unsupported catalog never becomes collection success: %s',async source=>{
 const {code,report}=await collect(source);expect(code).not.toBe(0);expect(report.complete).toBe(false);expect(typeof report.error).toBe('string');
});
test('catalog refuses roots outside its captured application',async()=>{
 const {code,report}=await collect("test('Local',()=>{});",collector);
 expect(code).not.toBe(0);expect(report.complete).toBe(false);expect(report.error).toContain('escapes');
});
