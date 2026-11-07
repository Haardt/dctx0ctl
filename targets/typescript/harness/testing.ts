import {fileURLToPath} from 'node:url';
/** Child-only environments. Resource ownership and provisioning remain authored. */
export type Fixture = {environment:Record<string,string|undefined>;close:()=>void|Promise<void>};
export function configuredFixture(key:string):string {
 const value=process.env[key];if(!value)throw new Error(`Fixture prerequisite ${key} is not configured`);return value;
}
export function planCommand(cli:string,project:string,args:string[],environment:Fixture['environment']):ReturnType<typeof Bun.spawn> {
 const env={...process.env};
 for(const [key,value] of Object.entries(environment)){
  if(!key||/[=\0]/.test(key))throw new Error('Invalid fixture environment key');
  if(value===undefined)delete env[key];else {if(value.includes('\0'))throw new Error(`Invalid fixture environment value for ${key}`);env[key]=value;}
 }
 return Bun.spawn([cli,...args],{cwd:project,env,stdout:'pipe',stderr:'ignore'});
}
export async function runPlan(args:string[],fixture:Fixture):Promise<void> {
 const child=planCommand(process.env.DCTX0_TEST_CLI??'dctx0',fileURLToPath(new URL('../../../',import.meta.url)),args,fixture.environment);
 const output=await new Response(child.stdout).text();const status=await child.exited;
 let report:{published:boolean;run:{result:{status:string}}};
 try{report=JSON.parse(output);}catch{throw new Error('Central verification did not return a report; inspect bindings test directly');}
 if(status!==0||report.published!==false||report.run.result.status!=='PASS')throw new Error('Central verification failed or is incomplete; inspect bindings test directly');
}
