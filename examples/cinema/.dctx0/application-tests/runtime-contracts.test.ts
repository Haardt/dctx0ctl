import {test,expect} from 'bun:test';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {validateRuntimeContract} from '../../src/domain/contracts.ts';
import {AssessmentClient,constructorTest,inputValue,projectInput,projectValue,type ValueModel} from './library/index.ts';

// This is a real Bun test calling production constructors directly. It is
// deliberately separate from the retained legacy worker/adapter integration.
test('CinemaRuntimeContracts',async()=>{
 const socket=process.env.DCTX0_APPLICATION_SOCKET??process.env.DCTX0_SOCKET;
 const plan=process.env.DCTX0_APPLICATION_PLAN??process.env.DCTX0_PLAN;
 if(!socket||!plan)throw new Error('Run with the central application assessment service and DCTX0_APPLICATION_SOCKET / DCTX0_APPLICATION_PLAN');
 const pin=JSON.parse(await readFile(new URL('library-pin.json',import.meta.url),'utf8')) as {version:string;application_protocol:number;files:Record<string,string>};
 expect(pin.version).toBe('0.2.0');expect(pin.application_protocol).toBe(2);
 for(const [file,hash] of Object.entries(pin.files))expect(createHash('sha256').update(await readFile(new URL('library/'+file,import.meta.url))).digest('hex')).toBe(hash);
 const bundle=JSON.parse(await readFile(plan,'utf8')) as {schema_version:number;boundary:string;model:{data:ValueModel}};
 expect(bundle.schema_version).toBe(1);expect(bundle.boundary).toBe('value-constructor');
 const client=await AssessmentClient.connect(socket);
 let failure:unknown,failed=false;
 try{
  await constructorTest(client,'CinemaRuntimeContracts',(value,ty)=>inputValue(value,ty,bundle.model.data),(input,ty)=>{
   const name=ty.symbol_id?.split('.').at(-1);
   if(!name)throw new Error('Expected a named Cinema runtime contract');
   try{return {kind:'accepted',value:validateRuntimeContract(name,input)};}
   catch(error){if(error instanceof TypeError||error instanceof RangeError)return {kind:'rejected',reason:error.message};throw error;}
  },(actual,ty)=>projectValue(actual,ty,bundle.model.data),(native,expected,ty)=>projectInput(native,expected,ty,bundle.model.data));
 }catch(error){failed=true;failure=error;}
 try{await client.finish();}finally{client.close();}
 if(failed)throw failure;
},120000);
