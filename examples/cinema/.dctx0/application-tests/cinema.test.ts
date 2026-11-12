import {test} from 'bun:test';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {validateRuntimeContract} from '../../src/domain/contracts.ts';
import {applicationTest} from './session.ts';
import {cinemaFixture} from './cinema-fixture.ts';
import {verifyNativeBoundaries} from './native-checks.ts';
import {constructorTest,operationTest,stateChangeTest,assertionTest,scenarioTest,lifecycleTest,identityTest,inputValue,projectInput,projectValue} from './library/index.ts';
test('CinemaSuite',()=>applicationTest(async(client,model,bundle)=>{
 const generated=await readFile(new URL('generated/facade.ts',import.meta.url),'utf8');
 const expected=createHash('sha256').update(JSON.stringify(model)).digest('hex');
 if(!generated.includes('// model-sha256: '+expected+'\n'))throw new Error('Generated facade does not match the centrally prepared model');
 const failures:unknown[]=[];
 for(const form of ['decoder','operation','state-change','assertion','scenario','lifecycle','entity-identity']){
  const registrations=bundle.plan.registrations.filter(registration=>registration.form===form);
  for(const registration of registrations){
   try{
    const factory=(context:import('./library/index.ts').CaseContext)=>cinemaFixture(model,context);
    switch(form){
     case 'decoder':await constructorTest(client,registration.test,(value,ty)=>inputValue(value,ty,model.data),(value,ty)=>{
      const name=ty.symbol_id?.split('.').at(-1);if(!name)throw new Error('Expected a named Cinema contract');
      try{return {kind:'accepted',value:validateRuntimeContract(name,value)};}catch(error){if(error instanceof TypeError||error instanceof RangeError)return {kind:'rejected',reason:error.message};throw error;}
     },(value,ty)=>projectValue(value,ty,model.data),(value,shape,ty)=>projectInput(value,shape,ty,model.data));break;
     case 'operation':await operationTest(client,registration.test,factory);break;
     case 'state-change':await stateChangeTest(client,registration.test,factory);break;
     case 'assertion':await assertionTest(client,registration.test,factory);break;
     case 'scenario':await scenarioTest(client,registration.test,factory);break;
     case 'lifecycle':await lifecycleTest(client,registration.test,factory);break;
     case 'entity-identity':await identityTest(client,registration.test);break;
    }
   }catch(error){failures.push(error);}
  }
 }
 await verifyNativeBoundaries(model);
 if(failures.length)throw new AggregateError(failures,'Cinema native forms failed');
}),180000);
