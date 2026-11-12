import {AssessmentClient} from './client.ts';
import {assertPass,BindingError,message,type DecodeResult,type DecoderOutcome,type TypeRef,type Value} from './protocol.ts';
export type Decoder<T>=(raw:Uint8Array,ty:TypeRef)=>DecodeResult<T>|Promise<DecodeResult<T>>;
export type Projector<T>=(actual:T,ty:TypeRef)=>Value|Promise<Value>;
/** The production call receives the centrally supplied bytes unchanged.
 * Parsing in a test binding must throw BindingError, never return rejection.
 */
export async function decoderTest<T>(client:AssessmentClient,test:string,decode:Decoder<T>,project:Projector<T>):Promise<void>{
 const registered=await client.exchange({kind:'register',test});
 if(registered.kind!=='registered')throw new Error('Expected decoder registration');
 const failures:string[]=[];
 try{
  while(true){
   const next=await client.exchange({kind:'next',test});
   if(next.kind==='no-more-cases')break;
   if(next.kind!=='case'||next.form!=='decoder'||!Number.isSafeInteger(next.context)||next.context<1)throw new Error('Expected central decoder case');
   const {context,id,directive}=next;
   try{
    if(directive.kind==='verdict'){assertPass(directive.result,id);continue;}
    if(directive.kind!=='decode'||!Array.isArray(directive.raw)||directive.raw.some(v=>!Number.isInteger(v)||v<0||v>255))throw new BindingError('Invalid decoder directive');
    // Retain a separate copy: production is allowed to mutate its byte buffer.
    const raw=[...directive.raw];
    let outcome:DecoderOutcome;
    try{
     const actual=await decode(Uint8Array.from(raw),directive.ty);
     if(actual.kind==='rejected')outcome={kind:'rejected',reason:actual.reason};
     else if(actual.kind==='accepted'){
      try{outcome={kind:'accepted',value:await project(actual.value,directive.ty)};}
      catch(error){outcome={kind:'binding-error',message:message(error)};}
     }else outcome={kind:'binding-error',message:'Production decoder binding returned invalid outcome'};
    }catch(error){outcome={kind:error instanceof BindingError?'binding-error':'exception',message:message(error)};}
    const reply=await client.exchange({kind:'observe',context,observation:{kind:'decoded',raw,outcome}});
    if(reply.kind!=='action'||reply.context!==context||reply.directive.kind!=='verdict')throw new Error('Expected central decoder verdict');
    assertPass(reply.directive.result,id);
   }catch(error){failures.push(`${id}: ${message(error)}`);}
   finally{
    const closed=await client.exchange({kind:'close-case',context,cleanup_error:null});
    if(closed.kind!=='case-closed'||closed.context!==context)throw new Error('Expected decoder case closure');
    try{assertPass(closed.result,id);}catch(error){failures.push(message(error));}
   }
  }
 }finally{
  const closed=await client.exchange({kind:'close-test',test});
  if(closed.kind!=='test-closed')throw new Error('Expected decoder test closure');
 }
 if(failures.length)throw new Error(failures.join('\n'));
}

export type InputProjector<T>=(actual:T,expected:Value,ty:TypeRef)=>Value|Promise<Value>;
export type InputCodec<T>=(input:Value,ty:TypeRef)=>T|Promise<T>;
/** Constructors consume native values. Codec failures happen before production
 * and cannot count as production rejection of a contract-invalid value.
 */
export async function constructorTest<T,U>(client:AssessmentClient,test:string,inputCodec:InputCodec<T>,construct:(input:T,ty:TypeRef)=>DecodeResult<U>|Promise<DecodeResult<U>>,project:Projector<U>,witness:InputProjector<T>):Promise<void>{
 const registered=await client.exchange({kind:'register',test});
 if(registered.kind!=='registered')throw new Error('Expected constructor registration');
 const failures:string[]=[];
 try{
  while(true){
   const next=await client.exchange({kind:'next',test});
   if(next.kind==='no-more-cases')break;
   if(next.kind!=='case'||next.form!=='decoder'||!Number.isSafeInteger(next.context)||next.context<1)throw new Error('Expected central constructor case');
   const {context,id,directive}=next;
   try{
    if(directive.kind==='verdict'){assertPass(directive.result,id);continue;}
    if(directive.kind!=='construct')throw new BindingError('Expected value-constructor directive');
    // The original is a shape hint; only the actual native witness is evidence.
    const expected=structuredClone(directive.value);
    let input:Value;
    let outcome:DecoderOutcome;
    let native:T;
    try{
     native=await inputCodec(structuredClone(expected),structuredClone(directive.ty));
     input=await witness(native,structuredClone(expected),structuredClone(directive.ty));
    }
    catch(error){
     outcome={kind:'binding-error',message:message(error)};
     const reply=await client.exchange({kind:'observe',context,observation:{kind:'binding-error',message:outcome.message}});
     if(reply.kind!=='action'||reply.context!==context||reply.directive.kind!=='verdict')throw new Error('Expected constructor binding verdict');
     assertPass(reply.directive.result,id);continue;
    }
    try{
     const actual=await construct(native,directive.ty);
     if(actual.kind==='rejected')outcome={kind:'rejected',reason:actual.reason};
     else if(actual.kind==='accepted'){
      try{outcome={kind:'accepted',value:await project(actual.value,directive.ty)};}
      catch(error){outcome={kind:'binding-error',message:message(error)};}
     }else outcome={kind:'binding-error',message:'Production constructor binding returned invalid outcome'};
    }catch(error){outcome={kind:'exception',message:message(error)};}
    const reply=await client.exchange({kind:'observe',context,observation:{kind:'constructed',input,outcome}});
    if(reply.kind!=='action'||reply.context!==context||reply.directive.kind!=='verdict')throw new Error('Expected central constructor verdict');
    assertPass(reply.directive.result,id);
   }catch(error){failures.push(`${id}: ${message(error)}`);}
   finally{
    const closed=await client.exchange({kind:'close-case',context,cleanup_error:null});
    if(closed.kind!=='case-closed'||closed.context!==context)throw new Error('Expected constructor case closure');
    try{assertPass(closed.result,id);}catch(error){failures.push(message(error));}
   }
  }
 }finally{
  const closed=await client.exchange({kind:'close-test',test});
  if(closed.kind!=='test-closed')throw new Error('Expected constructor test closure');
 }
 if(failures.length)throw new Error(failures.join('\n'));
}
