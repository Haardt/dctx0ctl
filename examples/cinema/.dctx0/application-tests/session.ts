import {expect} from 'bun:test';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {AssessmentClient} from './library/index.ts';
import type {OperationModel} from './operation-fixtures.ts';
export type ApplicationBundle={schema_version:number;model:OperationModel;plan:{registrations:{form:string;test:string;group:string}[]}};
export async function applicationTest(run:(client:AssessmentClient,model:OperationModel,bundle:ApplicationBundle)=>Promise<void>):Promise<void>{
 const socket=process.env.DCTX0_APPLICATION_SOCKET??process.env.DCTX0_SOCKET,plan=process.env.DCTX0_APPLICATION_PLAN??process.env.DCTX0_PLAN;
 if(!socket||!plan)throw new Error('Run with central application service and DCTX0_APPLICATION_SOCKET / DCTX0_APPLICATION_PLAN');
 const pin=JSON.parse(await readFile(new URL('library-pin.json',import.meta.url),'utf8')) as {version:string;application_protocol:number;files:Record<string,string>};
 expect(pin.version).toBe('0.2.0');expect(pin.application_protocol).toBe(2);
 for(const [file,hash] of Object.entries(pin.files))expect(createHash('sha256').update(await readFile(new URL('library/'+file,import.meta.url))).digest('hex')).toBe(hash);
 const bundle=JSON.parse(await readFile(plan,'utf8')) as ApplicationBundle;
 expect(bundle.schema_version).toBe(1);
 const client=await AssessmentClient.connect(socket);
 let failure:unknown,failed=false;
 try{await run(client,bundle.model,bundle);}catch(error){failed=true;failure=error;}
 try{await client.finish();}finally{client.close();}
 if(failed)throw failure;
}
