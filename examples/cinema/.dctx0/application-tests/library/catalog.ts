/** Collect actual root registration calls without executing test callbacks.
 * This is a bounded Bun module-mock collector, not a Bun --list-tests API.
 */
import {mock} from 'bun:test';
import {realpath} from 'node:fs/promises';
import {resolve,relative,isAbsolute} from 'node:path';
import {pathToFileURL} from 'node:url';
const root=await realpath(process.cwd());
const tests:{entrypoint:string;name:string;skipped:boolean;focused:boolean}[]=[];
let entrypoint='',complete=true;
const limitations=['Registers top-level test()/it() declarations through a mocked bun:test API; test callbacks are never executed.','Nested describe/each/hooks/concurrent forms are unsupported and fail collection; this does not enumerate central cases as individual Bun tests.'];
function unavailable():never{complete=false;throw new Error('Unsupported Bun collection form');}
function capture(skipped:boolean,focused:boolean){return (name:unknown,callback?:unknown)=>{
 if(typeof name!=='string'||!name||name.length>4096||typeof callback!=='function'||tests.length>=1024)unavailable();
 if(tests.some(test=>test.entrypoint===entrypoint&&test.name===name))throw new Error('Duplicate native root test registration');
 tests.push({entrypoint,name,skipped,focused});
};}
const test=Object.assign(capture(false,false),{skip:capture(true,false),todo:capture(true,false),only:capture(false,true),each:unavailable,concurrent:unavailable});
mock.module('bun:test',()=>({test,it:test,describe:unavailable,expect:unavailable,beforeAll:unavailable,afterAll:unavailable,beforeEach:unavailable,afterEach:unavailable}));
try{
 const roots=process.argv.slice(2);if(!roots.length)throw new Error('At least one native root is required');
 for(const file of roots){
  const actual=await realpath(resolve(root,file));entrypoint=relative(root,actual);
  if(!entrypoint||entrypoint.startsWith('..')||isAbsolute(entrypoint))throw new Error('Native catalog root escapes captured application');
  await import(pathToFileURL(actual).href);
 }
 if(!tests.length||tests.some(test=>test.skipped||test.focused))throw new Error('Native root catalog is empty, skipped or focused');
 console.log(JSON.stringify({schema_version:1,collector_version:'dctx0-bun-registration-1',library_version:'0.2.0',complete,tests,limitations}));
}catch(error){
 console.log(JSON.stringify({schema_version:1,collector_version:'dctx0-bun-registration-1',library_version:'0.2.0',complete:false,tests,limitations,error:error instanceof Error?error.message:String(error)}));process.exitCode=1;
}
